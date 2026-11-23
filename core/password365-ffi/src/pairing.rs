//! Joining an account from another device (scan its code, compare six
//! digits, type the master password) and syncing with paired devices over
//! the local network. iOS lets the app use the network only while it is
//! open, so the phone starts every sync; network waits never hold the vault.

use std::cell::OnceCell;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use password365_app::devices::{DeviceDto, net_error};
use password365_app::reach::reach;
use password365_net::pairing::{JoinListener, Joining};
use zeroize::Zeroizing;

use crate::{Failure, Password365, Result};

/// Finds Password365 devices on the local network. On iPhone this is the
/// system's Bonjour (Swift): only it may browse without Apple's multicast
/// entitlement. Asked only when a device is not at its known address.
#[uniffi::export(with_foreign)]
pub trait Discovery: Send + Sync {
    /// `ip:port` of every device advertising Password365 sync right now.
    fn browse(&self) -> Vec<String>;
}

/// A join in progress (between the steps the user takes).
#[derive(Default)]
pub(crate) struct JoinState {
    joining: Option<Joining>,
    bundle: Option<Zeroizing<Vec<u8>>>,
    /// This (new) device is showing a code and waiting for the device with the vault to scan it.
    listener: Option<JoinListener>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub(crate) type JoinSlot = Mutex<JoinState>;

fn step_missing() -> Failure {
    Failure::Failed {
        code: "not_found".into(),
        reason: "no device to join is waiting — scan the code again".into(),
    }
}

/// A paired device.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Device {
    /// Sync id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Last successful sync (ms since epoch).
    pub last_sync: Option<i64>,
    /// Changes made here that it has not received yet.
    pub pending_changes: u64,
    /// Missing changes for long (show a reminder).
    pub out_of_sync: bool,
}

impl From<DeviceDto> for Device {
    fn from(d: DeviceDto) -> Self {
        Self {
            id: d.id,
            name: d.name,
            last_sync: d.last_sync,
            pending_changes: d.pending_changes as u64,
            out_of_sync: d.out_of_sync,
        }
    }
}

/// What syncing did here.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncSummary {
    /// Items created, changed or deleted here.
    pub changed: u64,
    /// Items with two different new passwords (to review).
    pub conflicts: u64,
    /// Devices that could not be reached (they sync next time).
    pub unreachable: u64,
}

impl Password365 {
    fn join_slot(&self) -> std::sync::MutexGuard<'_, JoinState> {
        self.join
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Syncs with one paired device: at its known address, else wherever
    /// `discover` finds it (proven by its pinned key); a new address is
    /// remembered.
    pub(crate) fn sync_device(
        &self,
        id: &str,
        discover: impl FnOnce() -> Vec<SocketAddr>,
    ) -> Result<SyncSummary> {
        let (known, key, expected) = {
            let mut app = self.app();
            (
                app.device_address(id).ok(),
                app.network_key()?,
                app.device_key(id)?,
            )
        };
        let mut reached = reach(known, &key, &expected, discover)?;
        let result = self
            .app()
            .sync_over(&mut reached.stream, reached.moved_to)?;
        Ok(SyncSummary {
            changed: result.changed as u64,
            conflicts: result.conflicts.len() as u64,
            unreachable: 0,
        })
    }
}

#[uniffi::export]
impl Password365 {
    /// New device: connects to the device showing `code` (scanned or typed);
    /// returns the six digits to compare with its screen.
    pub fn join_connect(&self, code: String) -> Result<String> {
        self.join_cancel();
        let joining = Joining::connect(code.trim()).map_err(net_error)?;
        let shown = joining.verification_code().to_owned();
        self.join_slot().joining = Some(joining);
        Ok(shown)
    }

    /// New device: waits until the other device approves (blocks; call it
    /// off the main thread). Harmless to call again once approved.
    pub fn join_wait(&self) -> Result<()> {
        if self.join_slot().bundle.is_some() {
            return Ok(());
        }
        let mut joining = self.join_slot().joining.take().ok_or_else(step_missing)?;
        let received = joining.receive_bundle().map_err(net_error);
        let mut slot = self.join_slot();
        slot.joining = Some(joining);
        slot.bundle = Some(received?);
        Ok(())
    }

    /// New device: opens the account with its master password (retry on a
    /// wrong one) and tells the other device this one's name.
    ///
    /// `replace_key`: this device still holds the Secret Key of an earlier account (the call
    /// failed with `foreign_secret_key` and the person agreed to replace it).
    pub fn join_finish(
        &self,
        password: String,
        device_name: String,
        replace_key: bool,
    ) -> Result<()> {
        let (joining, bundle) = {
            let mut slot = self.join_slot();
            let bundle = slot.bundle.clone().ok_or_else(step_missing)?;
            (slot.joining.take().ok_or_else(step_missing)?, bundle)
        };
        let joined = if replace_key {
            self.app()
                .join_account_replacing_key(&bundle, password.into())
        } else {
            self.app().join_account(&bundle, password.into())
        };
        if let Err(e) = joined {
            self.join_slot().joining = Some(joining);
            return Err(e.into());
        }
        self.app().complete_join(joining, &device_name)?;
        *self.join_slot() = JoinState::default();
        Ok(())
    }

    /// This NEW device shows a code for the device that has the vault to scan (the other way to
    /// pair): a QR link and its plain text. Then `join_accept`, `join_wait` and `join_finish`.
    pub fn join_show_code(&self) -> Result<crate::hosting::PairingInvite> {
        self.join_cancel();
        let ip = password365_net::pairing::local_ip().map_err(net_error)?;
        let listener = JoinListener::open(ip).map_err(net_error)?;
        let invite = crate::hosting::PairingInvite {
            link: listener.invite_link(),
            code: listener.invite_code(),
        };
        let mut slot = self.join_slot();
        slot.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        slot.listener = Some(listener);
        Ok(invite)
    }

    /// Waits until the device with the vault scans the code and connects (blocks: call it off the
    /// main thread); returns the six digits to show here, which its person types there.
    pub fn join_accept(&self) -> Result<String> {
        let (listener, cancel) = {
            let mut slot = self.join_slot();
            (
                slot.listener.take().ok_or_else(step_missing)?,
                std::sync::Arc::clone(&slot.cancel),
            )
        };
        let joining = listener.accept_cancellable(&cancel).map_err(net_error)?;
        let shown = joining.verification_code().to_owned();
        self.join_slot().joining = Some(joining);
        Ok(shown)
    }

    /// Gives up a join (the codes did not match, or the user went back).
    pub fn join_cancel(&self) {
        self.join_slot()
            .cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let state = std::mem::take(&mut *self.join_slot());
        if let Some(joining) = state.joining {
            let _ = joining.cancel();
        }
    }

    /// This device's name, as the others show it.
    pub fn device_name(&self) -> Result<String> {
        Ok(self.app().this_device()?.name)
    }

    /// Paired devices.
    pub fn devices(&self) -> Result<Vec<Device>> {
        Ok(self
            .app()
            .devices()?
            .into_iter()
            .map(Device::from)
            .collect())
    }

    /// Syncs with every paired device it can reach on this network. Devices
    /// that moved are looked for with `discovery` (once per call).
    pub fn sync_all(&self, discovery: Option<Arc<dyn Discovery>>) -> Result<SyncSummary> {
        let ids: Vec<String> = self.app().devices()?.into_iter().map(|d| d.id).collect();
        let found: OnceCell<Vec<SocketAddr>> = OnceCell::new();
        let discover = || {
            found
                .get_or_init(|| {
                    discovery
                        .as_ref()
                        .map(|d| d.browse().iter().filter_map(|a| a.parse().ok()).collect())
                        .unwrap_or_default()
                })
                .clone()
        };
        let mut total = SyncSummary {
            changed: 0,
            conflicts: 0,
            unreachable: 0,
        };
        for id in ids {
            match self.sync_device(&id, discover) {
                Ok(done) => {
                    total.changed += done.changed;
                    total.conflicts += done.conflicts;
                }
                _ => total.unreachable += 1,
            }
        }
        Ok(total)
    }
}

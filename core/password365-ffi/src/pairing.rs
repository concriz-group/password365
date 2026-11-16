//! Joining an account from another device (scan its code, compare six
//! digits, type the master password) and syncing with paired devices over
//! the local network. iOS lets the app use the network only while it is
//! open, so the phone starts every sync; network waits never hold the vault.

use std::cell::OnceCell;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use password365_app::devices::{DeviceDto, net_error};
use password365_app::reach::reach;
use password365_net::pairing::Joining;
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
}

pub(crate) type JoinSlot = Mutex<JoinState>;

fn step_missing() -> Failure {
    Failure::Failed {
        code: "not_found".into(),
        message: "no device to join is waiting — scan the code again".into(),
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
    pub fn join_finish(&self, password: String, device_name: String) -> Result<()> {
        let (joining, bundle) = {
            let mut slot = self.join_slot();
            let bundle = slot.bundle.clone().ok_or_else(step_missing)?;
            (slot.joining.take().ok_or_else(step_missing)?, bundle)
        };
        if let Err(e) = self.app().join_account(&bundle, password.into()) {
            self.join_slot().joining = Some(joining);
            return Err(e.into());
        }
        self.app().complete_join(joining, &device_name)?;
        *self.join_slot() = JoinState::default();
        Ok(())
    }

    /// Gives up a join (the codes did not match, or the user went back).
    pub fn join_cancel(&self) {
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

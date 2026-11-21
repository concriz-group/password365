//! Inviting a device from this one (the other direction of pairing): show a QR code, wait for the new
//! device, compare six digits, send the account. The same steps as the desktop's "Add a device",
//! for the phone apps. Network waits never hold the vault, and a cancel stops the wait.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use password365_app::devices::net_error;
use password365_net::pairing::{HostPairing, PairingHost, local_ip};
use password365_net::tcp::SYNC_PORT;

use crate::pairing::Device;
use crate::{Failure, Password365, Result};

/// What the "Add a device" screen shows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairingInvite {
    /// The link the QR code carries: the new device's own camera opens Password365 with it.
    pub link: String,
    /// The same invite as plain text (to paste on a device with no camera).
    pub code: String,
}

/// An invite in progress (between the steps the user takes).
#[derive(Default)]
pub(crate) struct HostState {
    host: Option<PairingHost>,
    pairing: Option<HostPairing>,
    cancel: Arc<AtomicBool>,
}

pub(crate) type HostSlot = Mutex<HostState>;

fn missing(step: &str) -> Failure {
    Failure::Failed {
        code: "not_found".into(),
        reason: format!("no pairing in progress ({step}) — start again"),
    }
}

#[uniffi::export]
impl Password365 {
    /// Opens an invite on this device's network address (the vault must be unlocked).
    pub fn pairing_start(&self) -> Result<PairingInvite> {
        self.pairing_cancel();
        let ip = local_ip().map_err(net_error)?;
        let host = self.app().start_pairing(ip)?;
        let invite = PairingInvite {
            link: host.invite_link(),
            code: host.invite_code(),
        };
        let mut slot = self.host_slot();
        slot.cancel = Arc::new(AtomicBool::new(false));
        slot.host = Some(host);
        Ok(invite)
    }

    /// Waits until a new device connects (blocks: call it off the main thread). Ends with a
    /// "cancelled" error when `pairing_cancel` is called.
    pub fn pairing_wait(&self) -> Result<()> {
        let (host, cancel) = {
            let mut slot = self.host_slot();
            (
                slot.host.take().ok_or_else(|| missing("invite"))?,
                Arc::clone(&slot.cancel),
            )
        };
        let pairing = host.accept_cancellable(&cancel).map_err(net_error)?;
        self.host_slot().pairing = Some(pairing);
        Ok(())
    }

    /// Stops an invite or a wait.
    pub fn pairing_cancel(&self) {
        let mut slot = self.host_slot();
        slot.cancel.store(true, Ordering::SeqCst);
        slot.host = None;
        slot.pairing = None;
    }

    /// The person typed the six digits the new device shows. If they are this connection's, sends
    /// the account (all vaults) and pins the new device; if not, the pairing ends. Blocks while
    /// the new device's person types the master password: call it off the main thread.
    pub fn pairing_approve(&self, typed_code: String) -> Result<Device> {
        let mut pairing = self
            .host_slot()
            .pairing
            .take()
            .ok_or_else(|| missing("approval"))?;
        let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
        let typed = digits(&typed_code);
        if typed.is_empty() || typed != digits(pairing.verification_code()) {
            let _ = pairing.reject();
            return Err(Failure::Failed {
                code: "validation".into(),
                reason: "wrong_pairing_code".into(),
            });
        }
        let ids: Vec<String> = self.app().vaults()?.into_iter().map(|v| v.id).collect();
        self.app().send_pairing_bundle(&mut pairing, &ids)?;
        let peer = pairing.wait_joined(SYNC_PORT).map_err(net_error)?;
        Ok(self.app().pin_device(peer)?.into())
    }
}

impl Password365 {
    fn host_slot(&self) -> std::sync::MutexGuard<'_, HostState> {
        self.host
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use password365_app::item_input::ItemInput;

    use crate::tests::{MemoryKeeper, PASSWORD, vault};

    #[test]
    fn a_device_with_a_vault_invites_another_and_the_account_arrives() {
        let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let host = vault(&dir_a, Arc::new(MemoryKeeper::default()));
        host.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
        host.app()
            .create_item(&ItemInput {
                title: "Atlas Travel".into(),
                ..ItemInput::default()
            })
            .unwrap();
        let guest = vault(&dir_b, Arc::new(MemoryKeeper::default()));

        let invite = host.pairing_start().unwrap();
        assert!(invite.link.starts_with("https://password365.app/pair#PASSWORD365-PAIR:1:"));

        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| host.pairing_wait());
            // The new device scans the link (or pastes the code) and shows six digits.
            let digits = guest.join_connect(invite.link.clone()).unwrap();
            waiting.join().unwrap().unwrap();

            let approving = scope.spawn(|| host.pairing_approve(digits.clone()));
            guest.join_wait().unwrap();
            guest.join_finish(PASSWORD.into(), "Laptop".into(), false).unwrap();
            let paired = approving.join().unwrap().unwrap();
            assert_eq!(paired.name, "Laptop");
        });
        assert_eq!(guest.items(String::new()).unwrap().len(), 1, "the item arrived");
        assert_eq!(host.devices().unwrap().len(), 1);
    }

    #[test]
    fn a_wrong_code_ends_the_pairing_and_sends_nothing() {
        let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let host = vault(&dir_a, Arc::new(MemoryKeeper::default()));
        host.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
        let guest = vault(&dir_b, Arc::new(MemoryKeeper::default()));
        let invite = host.pairing_start().unwrap();
        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| host.pairing_wait());
            guest.join_connect(invite.code.clone()).unwrap();
            waiting.join().unwrap().unwrap();
        });
        let error = host.pairing_approve("000000".into()).unwrap_err();
        assert!(matches!(error, crate::Failure::Failed { ref code, .. } if code == "validation"));
        assert!(host.devices().unwrap().is_empty());
        // Nothing is left to approve afterwards.
        assert!(host.pairing_approve("000000".into()).is_err());
    }

    #[test]
    fn cancelling_stops_the_wait() {
        let dir = tempfile::tempdir().unwrap();
        let host = vault(&dir, Arc::new(MemoryKeeper::default()));
        host.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
        host.pairing_start().unwrap();
        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| host.pairing_wait());
            std::thread::sleep(std::time::Duration::from_millis(200));
            host.pairing_cancel();
            assert!(waiting.join().unwrap().is_err());
        });
    }
}

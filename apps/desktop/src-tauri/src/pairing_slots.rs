//! Pairing steps in progress, kept between IPC calls (one pairing at a time).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use password365_app::{AppError, Result};
use password365_net::pairing::{HostPairing, Joining, PairingHost};
use zeroize::Zeroizing;

/// In-progress pairing state, registered with Tauri next to the app state.
#[derive(Default)]
pub struct PairingSlots {
    host: Mutex<Option<PairingHost>>,
    pairing: Mutex<Option<HostPairing>>,
    joining: Mutex<Option<Joining>>,
    bundle: Mutex<Option<Zeroizing<Vec<u8>>>>,
    cancel: Mutex<Arc<AtomicBool>>,
}

/// Shared handle registered with Tauri.
pub type SharedSlots = Arc<PairingSlots>;

fn slot<T>(m: &Mutex<Option<T>>) -> Result<std::sync::MutexGuard<'_, Option<T>>> {
    m.lock()
        .map_err(|_| AppError::Internal("pairing state poisoned".into()))
}

fn missing(step: &str) -> AppError {
    AppError::Validation(format!("No pairing in progress ({step}); start again."))
}

impl PairingSlots {
    /// Starts a new invite, cancelling any previous one.
    pub fn put_host(&self, host: PairingHost) -> Result<Arc<AtomicBool>> {
        self.cancel_all()?;
        let flag = Arc::new(AtomicBool::new(false));
        *self
            .cancel
            .lock()
            .map_err(|_| AppError::Internal("pairing state poisoned".into()))? = Arc::clone(&flag);
        *slot(&self.host)? = Some(host);
        Ok(flag)
    }

    /// The flag that stops the current invite's wait.
    pub fn cancel_flag(&self) -> Result<Arc<AtomicBool>> {
        Ok(Arc::clone(&*self.cancel.lock().map_err(|_| {
            AppError::Internal("pairing state poisoned".into())
        })?))
    }

    /// The invite waiting for a device.
    pub fn take_host(&self) -> Result<PairingHost> {
        slot(&self.host)?.take().ok_or_else(|| missing("invite"))
    }

    /// A connected new device waiting for approval.
    pub fn put_pairing(&self, pairing: HostPairing) -> Result<()> {
        *slot(&self.pairing)? = Some(pairing);
        Ok(())
    }

    /// Takes the connected device for approval or rejection.
    pub fn take_pairing(&self) -> Result<HostPairing> {
        slot(&self.pairing)?
            .take()
            .ok_or_else(|| missing("approval"))
    }

    /// New device: the connection to the existing device.
    pub fn put_joining(&self, joining: Joining) -> Result<()> {
        *slot(&self.joining)? = Some(joining);
        Ok(())
    }

    /// New device: takes the connection.
    pub fn take_joining(&self) -> Result<Joining> {
        slot(&self.joining)?.take().ok_or_else(|| missing("join"))
    }

    /// New device: keeps the received bundle until the master password is right.
    pub fn put_bundle(&self, bundle: Zeroizing<Vec<u8>>) -> Result<()> {
        *slot(&self.bundle)? = Some(bundle);
        Ok(())
    }

    /// New device: the received bundle (kept for a retry).
    pub fn bundle(&self) -> Result<Zeroizing<Vec<u8>>> {
        slot(&self.bundle)?.clone().ok_or_else(|| missing("bundle"))
    }

    /// Stops any wait and forgets every step (secrets are wiped on drop).
    pub fn cancel_all(&self) -> Result<()> {
        self.cancel
            .lock()
            .map_err(|_| AppError::Internal("pairing state poisoned".into()))?
            .store(true, Ordering::Relaxed);
        if let Some(pairing) = slot(&self.pairing)?.take() {
            let _ = pairing.reject();
        }
        if let Some(joining) = slot(&self.joining)?.take() {
            let _ = joining.cancel();
        }
        slot(&self.host)?.take();
        slot(&self.bundle)?.take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_slots_say_what_is_missing() {
        let slots = PairingSlots::default();
        assert!(
            slots
                .take_host()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
                .contains("invite")
        );
        assert!(
            slots
                .take_pairing()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
                .contains("approval")
        );
        assert!(
            slots
                .take_joining()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
                .contains("join")
        );
        assert!(
            slots
                .bundle()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
                .contains("bundle")
        );
        slots.cancel_all().unwrap();
    }

    #[test]
    fn the_bundle_stays_for_a_retry_until_cancelled() {
        let slots = PairingSlots::default();
        slots.put_bundle(Zeroizing::new(vec![1, 2])).unwrap();
        assert_eq!(slots.bundle().unwrap().as_slice(), &[1, 2]);
        assert_eq!(slots.bundle().unwrap().as_slice(), &[1, 2]);
        slots.cancel_all().unwrap();
        assert!(slots.bundle().is_err());
    }
}

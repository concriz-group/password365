//! Paired devices, pairing and sync (1Password-style Devices screen).
//!
//! Networking blocks (waiting for the other device, the user comparing
//! codes), so the frontend runs the network steps itself, *outside* any lock
//! on the service, and calls these methods only for the parts that touch the
//! vault. See `apps/desktop/src-tauri/src/sync_commands.rs`.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr};

use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;
use password365_model::{Timestamp, VaultId};
use password365_net::NetError;
use password365_net::pairing::{HostPairing, Joining, PairingHost};
use password365_net::secure::SecureStream;
use password365_net::tcp::SYNC_PORT;
use password365_sync::DeviceId;
use password365_sync::pairing::StaticKey;
use password365_vault::{Peer, Session, Store};
use serde::Serialize;

use crate::service::parse_id;
use crate::{AppError, AppService, Result};

/// This device, as other devices see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThisDeviceDto {
    /// Sync id.
    pub id: String,
    /// Name shown on other devices.
    pub name: String,
}

/// A paired device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceDto {
    /// Sync id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// When it was paired (ms since epoch).
    pub paired_at: i64,
    /// Last successful sync (ms since epoch).
    pub last_sync: Option<i64>,
    /// Local changes it has not received yet.
    pub pending_changes: usize,
    /// Where it was last reachable.
    pub addr: Option<String>,
    /// It is missing local changes and has not synced for a long time (or
    /// is missing very many): show a reminder.
    pub out_of_sync: bool,
}

/// What a sync did on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncResultDto {
    /// The device synced with.
    pub device: DeviceDto,
    /// Items created, changed, deleted or restored here.
    pub changed: usize,
    /// Item ids where two different passwords were set at once (review them).
    pub conflicts: Vec<String>,
}

impl AppService {
    /// This device's identity.
    pub fn this_device(&mut self) -> Result<ThisDeviceDto> {
        let session = self.session()?;
        Ok(ThisDeviceDto {
            id: session.device_id().to_string(),
            name: session.device_name().to_owned(),
        })
    }

    /// Renames this device.
    pub fn rename_this_device(&mut self, name: &str) -> Result<ThisDeviceDto> {
        self.session()?.set_device_name(name)?;
        self.this_device()
    }

    /// Paired devices, by name.
    pub fn devices(&mut self) -> Result<Vec<DeviceDto>> {
        let session = self.session()?;
        session
            .peers()
            .into_iter()
            .map(|peer| {
                Ok(device_dto(
                    peer,
                    session.changes_pending_for(peer.device_id)?,
                ))
            })
            .collect()
    }

    /// Removes a device from the account: it can no longer sync with this
    /// one, and every other paired device refuses it after its next sync.
    pub fn remove_device(&mut self, id: &str) -> Result<()> {
        let id = parse_id::<DeviceId>(id)?;
        let session = self.session()?;
        session.peer(id)?;
        Ok(session.revoke_device(id)?)
    }

    /// `true` while the vault is unlocked and paired with at least one device:
    /// only then does this device listen for syncs. Does not count as activity
    /// (polling it must not keep the vault from auto-locking).
    #[must_use]
    pub fn sync_listener_wanted(&self) -> bool {
        self.unlocked_session()
            .is_some_and(|s| !s.peers().is_empty())
    }

    /// While syncing is possible (see [`Self::sync_listener_wanted`]), the
    /// number of the latest change made here; it grows whenever something
    /// changed, so the background service can push it right away.
    #[must_use]
    pub fn sync_watch(&self) -> Option<u64> {
        self.unlocked_session()
            .filter(|s| !s.peers().is_empty())
            .map(Session::sync_seq)
    }

    /// The key that identifies this device on the network (unlocked only).
    pub fn network_key(&mut self) -> Result<StaticKey> {
        Ok(self.session()?.static_key())
    }

    /// Where to reach a paired device.
    pub fn device_address(&mut self, id: &str) -> Result<SocketAddr> {
        let id = parse_id::<DeviceId>(id)?;
        let addr = self.session()?.peer(id)?.addr.clone();
        addr.and_then(|a| a.parse().ok()).ok_or_else(|| {
            AppError::Validation("This device's network address is unknown; pair it again.".into())
        })
    }

    /// The pinned key of a paired device (to recognise it at a new address).
    pub fn device_key(&mut self, id: &str) -> Result<[u8; 32]> {
        let id = parse_id::<DeviceId>(id)?;
        Ok(self.session()?.peer(id)?.static_public)
    }

    /// Sets where to reach a paired device (`ip:port`), e.g. after its
    /// network address changed.
    pub fn set_device_address(&mut self, id: &str, addr: &str) -> Result<()> {
        let id = parse_id::<DeviceId>(id)?;
        let addr: SocketAddr = addr.trim().parse().map_err(|_| {
            AppError::Validation(format!(
                "'{addr}' is not an address like 192.168.1.20:47480"
            ))
        })?;
        Ok(self.session()?.set_peer_addr(id, Some(addr.to_string()))?)
    }

    /// Runs a sync conversation over an already-established connection.
    /// `remote_ip` (when known) is remembered as the device's address.
    pub fn sync_over<S: Read + Write>(
        &mut self,
        stream: &mut SecureStream<S>,
        remote_ip: Option<IpAddr>,
    ) -> Result<SyncResultDto> {
        let session = self.session()?;
        let outcome = password365_net::sync::sync(stream, session).map_err(net_error)?;
        if let Some(ip) = remote_ip {
            session.set_peer_addr(
                outcome.peer,
                Some(SocketAddr::new(ip, SYNC_PORT).to_string()),
            )?;
        }
        let pending = session.changes_pending_for(outcome.peer)?;
        Ok(SyncResultDto {
            device: device_dto(session.peer(outcome.peer)?, pending),
            changed: outcome.report.changed.len(),
            conflicts: outcome
                .report
                .conflicts
                .iter()
                .map(ToString::to_string)
                .collect(),
        })
    }

    /// Writes a `.p365sync` file with the changes a paired device is missing;
    /// only that device can read it. Returns how many items it carries.
    pub fn export_sync_file(&mut self, id: &str, path: &std::path::Path) -> Result<usize> {
        let id = parse_id::<DeviceId>(id)?;
        let session = self.session()?;
        let bytes = password365_net::sync_file::export(session, id).map_err(net_error)?;
        let peer = session.peer(id)?;
        let carried = session.changes_for(peer, peer.acked_seq)?.len();
        write_private(path, &bytes)?;
        Ok(carried)
    }

    /// Reads a `.p365sync` file from a paired device and merges it.
    pub fn import_sync_file(&mut self, path: &std::path::Path) -> Result<SyncResultDto> {
        let bytes = std::fs::read(path)
            .map_err(|e| AppError::Io(format!("cannot read {}: {e}", path.display())))?;
        let session = self.session()?;
        let outcome = password365_net::sync_file::import(session, &bytes).map_err(net_error)?;
        let pending = session.changes_pending_for(outcome.peer)?;
        Ok(SyncResultDto {
            device: device_dto(session.peer(outcome.peer)?, pending),
            changed: outcome.report.changed.len(),
            conflicts: outcome
                .report
                .conflicts
                .iter()
                .map(ToString::to_string)
                .collect(),
        })
    }

    /// Starts inviting a new device: listen on `ip` and return the host
    /// (its `invite_code()` is the QR code).
    pub fn start_pairing(&mut self, ip: IpAddr) -> Result<PairingHost> {
        PairingHost::open(self.session()?, ip).map_err(net_error)
    }

    /// The user confirmed the codes match: sends the account and `vault_ids`
    /// to the new device. Then wait (outside any lock) with
    /// `HostPairing::wait_joined(SYNC_PORT)` and pin the result with [`Self::pin_device`].
    pub fn send_pairing_bundle(
        &mut self,
        pairing: &mut HostPairing,
        vault_ids: &[String],
    ) -> Result<()> {
        let vaults = vault_ids
            .iter()
            .map(|id| parse_id::<VaultId>(id))
            .collect::<Result<Vec<_>>>()?;
        let secret_key = self.keys.load()?.ok_or(AppError::NotInitialized)?;
        pairing
            .send_bundle(self.session()?, &secret_key, &vaults)
            .map_err(net_error)
    }

    /// Pins a newly paired device so it can sync with this one.
    pub fn pin_device(&mut self, peer: Peer) -> Result<DeviceDto> {
        let session = self.session()?;
        let id = peer.device_id;
        session.pair_peer(peer)?;
        Ok(device_dto(
            session.peer(id)?,
            session.changes_pending_for(id)?,
        ))
    }

    /// New device: creates this device's store from the received bundle,
    /// keeps the Secret Key in the keychain and unlocks. A wrong master
    /// password fails without writing anything (the user can retry).
    pub fn join_account(&mut self, bundle: &[u8], password: SecretString) -> Result<()> {
        self.guarded(|s| s.join_account_unguarded(bundle, password))
    }

    fn join_account_unguarded(&mut self, bundle: &[u8], password: SecretString) -> Result<()> {
        if self.status().initialized {
            return Err(AppError::AlreadyInitialized);
        }
        let (session, secret_key) =
            Store::join(&self.config.store_path, bundle, password, self.config.kdf)?;
        if let Err(e) = self.remember_joined_key(&secret_key) {
            // Never leave a store this device cannot unlock.
            drop(session);
            let _ = std::fs::remove_file(&self.config.store_path);
            return Err(e);
        }
        self.start_session(session);
        Ok(())
    }

    /// Stores the account's Secret Key. A key left by an earlier, interrupted
    /// attempt to join the same account is kept; a different account's key is
    /// never overwritten.
    fn remember_joined_key(&self, key: &AccountSecretKey) -> Result<()> {
        match self.keys.load()? {
            None => self.keys.save(key),
            Some(stored) if stored.expose() == key.expose() => Ok(()),
            Some(_) => Err(AppError::Validation(
                "This computer already holds the Secret Key of a different account.".into(),
            )),
        }
    }

    /// New device: tells the other device who we are (after `join_account`).
    pub fn complete_join(&mut self, joining: Joining, device_name: &str) -> Result<()> {
        let session = self.session()?;
        session.set_device_name(device_name)?;
        joining.complete(session, SYNC_PORT).map_err(net_error)
    }
}

/// A device missing changes is "out of sync" after this long without syncing...
pub const OUT_OF_SYNC_AFTER_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// ...or as soon as it misses this many changes.
pub const OUT_OF_SYNC_CHANGES: usize = 50;

/// Whether a device needs a reminder (pure, for tests).
#[must_use]
pub fn is_out_of_sync(pending_changes: usize, last_contact: Timestamp, now: Timestamp) -> bool {
    pending_changes >= OUT_OF_SYNC_CHANGES
        || (pending_changes > 0
            && now.as_millis() - last_contact.as_millis() > OUT_OF_SYNC_AFTER_MS)
}

/// Writes `bytes` to `path` owner-only, replacing any older file atomically.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let io = |e: std::io::Error| AppError::Io(format!("cannot write {}: {e}", path.display()));
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let _ = std::fs::remove_file(&tmp);
    let mut file = password365_vault::private_file::create_private_file(&tmp).map_err(io)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.map_err(io)
}

fn device_dto(peer: &Peer, pending_changes: usize) -> DeviceDto {
    let last_contact = peer.last_sync.unwrap_or(peer.paired_at);
    DeviceDto {
        out_of_sync: is_out_of_sync(pending_changes, last_contact, Timestamp::now()),
        id: peer.device_id.to_string(),
        name: peer.name.clone(),
        paired_at: peer.paired_at.as_millis(),
        last_sync: peer.last_sync.map(Timestamp::as_millis),
        pending_changes,
        addr: peer.addr.clone(),
    }
}

/// Maps transport errors to what the user should read.
#[must_use]
pub fn net_error(error: NetError) -> AppError {
    match error {
        NetError::Vault(e) => e.into(),
        NetError::Io(e) => AppError::Io(format!("Could not reach the other device: {e}")),
        other => AppError::Validation(other.to_string()),
    }
}

#[cfg(test)]
#[path = "devices_tests.rs"]
mod tests;

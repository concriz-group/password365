//! IPC commands for the Devices screen, pairing and joining.
//!
//! Every step that waits on the network or on the other person runs on a
//! blocking thread *without* the app-state lock; only the parts that touch
//! the vault take it.

use password365_app::devices::{DeviceDto, SyncResultDto, ThisDeviceDto, net_error};
use password365_app::{AppError, AppService, Result};
use password365_crypto::secret::SecretString;
use password365_net::pairing::{Joining, local_ip};
use password365_net::tcp::SYNC_PORT;
use serde::Serialize;
use tauri::State;

use crate::pairing_slots::{PairingSlots, SharedSlots};
use crate::state::{AppState, SharedState};
use crate::sync_service;

/// What the "Add a device" screen shows.
#[derive(Debug, Serialize)]
pub struct InviteDto {
    /// The pairing code (also typed by hand if there is no camera).
    pub code: String,
    /// The same code as an SVG QR code.
    pub qr_svg: String,
}

// ------------------------------------------------------------ plain logic

/// Existing device: opens an invite on the LAN address.
pub fn invite(state: &AppState, slots: &PairingSlots) -> Result<InviteDto> {
    invite_on(state, slots, local_ip().map_err(net_error)?)
}

/// Opens an invite listening on `ip`.
pub fn invite_on(
    state: &AppState,
    slots: &PairingSlots,
    ip: std::net::IpAddr,
) -> Result<InviteDto> {
    let host = state.with(|s| s.start_pairing(ip))?;
    let code = host.invite_code();
    slots.put_host(host)?;
    Ok(InviteDto {
        qr_svg: qr_svg(&code)?,
        code,
    })
}

/// Existing device: waits for the new device; returns the code to compare.
pub fn wait_for_device(slots: &PairingSlots) -> Result<String> {
    let host = slots.take_host()?;
    let cancel = slots.cancel_flag()?;
    let pairing = host.accept_cancellable(&cancel).map_err(net_error)?;
    let code = pairing.verification_code().to_owned();
    slots.put_pairing(pairing)?;
    Ok(code)
}

/// Existing device: codes matched; send the account and pin the new device.
pub fn approve_device(
    state: &AppState,
    slots: &PairingSlots,
    vault_ids: &[String],
) -> Result<DeviceDto> {
    let mut pairing = slots.take_pairing()?;
    state.with(|s| s.send_pairing_bundle(&mut pairing, vault_ids))?;
    let peer = pairing.wait_joined(SYNC_PORT).map_err(net_error)?;
    state.with(|s| s.pin_device(peer))
}

/// New device: connects with the scanned/typed code; returns the code to compare.
pub fn join_connect(slots: &PairingSlots, code: &str) -> Result<String> {
    slots.cancel_all()?;
    let joining = Joining::connect(code).map_err(net_error)?;
    let shown = joining.verification_code().to_owned();
    slots.put_joining(joining)?;
    Ok(shown)
}

/// New device: waits for the other device to approve. Calling it again
/// after the account data arrived is harmless.
pub fn join_wait_approval(slots: &PairingSlots) -> Result<()> {
    if slots.bundle().is_ok() {
        return Ok(());
    }
    let mut joining = slots.take_joining()?;
    let bundle = joining.receive_bundle().map_err(net_error)?;
    slots.put_bundle(bundle)?;
    slots.put_joining(joining)
}

/// New device: creates the account from the bundle (retry on a wrong
/// password) and reports back. The first sync is left to the background
/// service, so the window never waits on the network here.
pub fn join_finish(
    state: &AppState,
    slots: &PairingSlots,
    password: SecretString,
    device_name: &str,
) -> Result<()> {
    let bundle = slots.bundle()?;
    // Hold the connection first: never create the account without being able
    // to report back to the other device.
    let joining = slots.take_joining()?;
    if let Err(e) = state.with(|s| s.join_account(&bundle, password)) {
        slots.put_joining(joining)?;
        return Err(e);
    }
    state.with(|s| s.complete_join(joining, device_name))?;
    slots.cancel_all()
}

pub(crate) fn qr_svg(code: &str) -> Result<String> {
    let qr = qrcode::QrCode::with_error_correction_level(code, qrcode::EcLevel::M)
        .map_err(|e| AppError::Internal(format!("QR code: {e}")))?;
    Ok(qr
        .render::<qrcode::render::svg::Color<'_>>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build())
}

pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(format!("background task failed: {e}")))?
}

// ------------------------------------------------------------ IPC commands

#[tauri::command]
pub fn this_device(state: State<'_, SharedState>) -> Result<ThisDeviceDto> {
    state.with(AppService::this_device)
}

#[tauri::command]
pub fn rename_this_device(state: State<'_, SharedState>, name: String) -> Result<ThisDeviceDto> {
    state.with(|s| s.rename_this_device(&name))
}

#[tauri::command]
pub fn devices(state: State<'_, SharedState>) -> Result<Vec<DeviceDto>> {
    state.with(AppService::devices)
}

#[tauri::command]
pub fn remove_device(state: State<'_, SharedState>, id: String) -> Result<()> {
    state.with(|s| s.remove_device(&id))
}

#[tauri::command]
pub async fn sync_device(state: State<'_, SharedState>, id: String) -> Result<SyncResultDto> {
    let state = SharedState::clone(&state);
    blocking(move || sync_service::sync_device(&state, &id)).await
}

#[tauri::command]
pub fn export_sync_file(
    state: State<'_, SharedState>,
    id: String,
    path: std::path::PathBuf,
) -> Result<usize> {
    state.with(|s| s.export_sync_file(&id, &path))
}

#[tauri::command]
pub fn import_sync_file(
    state: State<'_, SharedState>,
    path: std::path::PathBuf,
) -> Result<SyncResultDto> {
    state.with(|s| s.import_sync_file(&path))
}

#[tauri::command]
pub fn invite_device(
    state: State<'_, SharedState>,
    slots: State<'_, SharedSlots>,
) -> Result<InviteDto> {
    invite(&state, &slots)
}

#[tauri::command]
pub async fn wait_for_new_device(slots: State<'_, SharedSlots>) -> Result<String> {
    let slots = SharedSlots::clone(&slots);
    blocking(move || wait_for_device(&slots)).await
}

#[tauri::command]
pub async fn approve_new_device(
    state: State<'_, SharedState>,
    slots: State<'_, SharedSlots>,
    vault_ids: Vec<String>,
) -> Result<DeviceDto> {
    let (state, slots) = (SharedState::clone(&state), SharedSlots::clone(&slots));
    blocking(move || approve_device(&state, &slots, &vault_ids)).await
}

#[tauri::command]
pub fn cancel_pairing(slots: State<'_, SharedSlots>) -> Result<()> {
    slots.cancel_all()
}

#[tauri::command]
pub async fn join_with_code(slots: State<'_, SharedSlots>, code: String) -> Result<String> {
    let slots = SharedSlots::clone(&slots);
    blocking(move || join_connect(&slots, &code)).await
}

#[tauri::command]
pub async fn join_wait(slots: State<'_, SharedSlots>) -> Result<()> {
    let slots = SharedSlots::clone(&slots);
    blocking(move || join_wait_approval(&slots)).await
}

#[tauri::command]
pub async fn join_complete(
    state: State<'_, SharedState>,
    slots: State<'_, SharedSlots>,
    password: SecretString,
    device_name: String,
) -> Result<()> {
    let (state, slots) = (SharedState::clone(&state), SharedSlots::clone(&slots));
    blocking(move || join_finish(&state, &slots, password, &device_name)).await
}

#[cfg(test)]
#[path = "sync_commands_tests.rs"]
mod tests;

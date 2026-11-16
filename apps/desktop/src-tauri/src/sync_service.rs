//! Background sync with paired devices (opt-in: only after pairing, and only
//! while the vault is unlocked).
//!
//! - Listens on [`SYNC_PORT`] for paired devices; anything else is refused
//!   by the handshake and the pinned-key check.
//! - Shortly after a change made here ([`PUSH_DELAY`]), and every
//!   [`AUTO_SYNC_INTERVAL`], syncs with every paired device it can reach.
//!
//! Network waits happen outside the app-state lock; only the short sync
//! conversation itself holds it.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use password365_app::Result;
use password365_app::devices::{SyncResultDto, net_error};
use password365_net::discovery::{self, Advertiser};
use password365_net::pairing::local_ip;
use password365_net::tcp::{self, SYNC_PORT};

use crate::state::AppState;

/// Event sent to the window after a sync changed something.
pub const SYNC_EVENT: &str = "sync-completed";
/// How often paired devices are synced in the background.
pub const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(60);
/// How long a local change settles before it is pushed (typing several
/// edits in a row sends them together).
pub const PUSH_DELAY: Duration = Duration::from_millis(1500);
const POLL: Duration = Duration::from_millis(500);

/// Syncs with one paired device now (the "Sync now" button). Tries its last
/// known address, then looks for it on the local network (its address may
/// have changed); a found address is used only if the handshake proves it is
/// that device.
pub fn sync_device(state: &AppState, id: &str) -> Result<SyncResultDto> {
    sync_device_with(state, id, || {
        discovery::browse(discovery::BROWSE_TIME).unwrap_or_default()
    })
}

/// [`sync_device`] with the discovery step supplied (tests).
pub fn sync_device_with(
    state: &AppState,
    id: &str,
    discover: impl FnOnce() -> Vec<SocketAddr>,
) -> Result<SyncResultDto> {
    let (known, key, expected) = state.with(|s| {
        Ok((
            s.device_address(id).ok(),
            s.network_key()?,
            s.device_key(id)?,
        ))
    })?;
    let mut last_error = None;
    if let Some(addr) = known {
        match tcp::connect(addr, &key) {
            Ok(mut stream) => return state.with(|s| s.sync_over(&mut stream, None)),
            Err(e) => last_error = Some(net_error(e)),
        }
    }
    for addr in discover() {
        if let Ok(mut stream) = tcp::connect(addr, &key)
            && stream.remote_static() == expected
        {
            return state.with(|s| s.sync_over(&mut stream, Some(addr.ip())));
        }
    }
    Err(last_error.unwrap_or_else(|| {
        password365_app::AppError::Io("The device was not found on this network.".into())
    }))
}

/// Syncs with every paired device; returns how many items changed here.
/// Unreachable devices are skipped (they sync next time).
pub fn sync_all(state: &AppState) -> usize {
    let ids: Vec<String> = state
        .with(|s| Ok(s.devices()?.into_iter().map(|d| d.id).collect()))
        .unwrap_or_default();
    ids.iter()
        .filter_map(|id| sync_device(state, id).ok())
        .map(|r| r.changed)
        .sum()
}

/// Handles one incoming connection; returns what the sync did.
pub fn serve(state: &AppState, stream: TcpStream, from: SocketAddr) -> Result<SyncResultDto> {
    stream
        .set_nonblocking(false)
        .map_err(|e| password365_app::AppError::Io(e.to_string()))?;
    let key = state.with(password365_app::AppService::network_key)?;
    let mut secure = tcp::secure_incoming(stream, &key).map_err(net_error)?;
    state.with(|s| s.sync_over(&mut secure, Some(from.ip())))
}

/// Starts the background thread. `notify` runs after a sync changed items.
pub fn start(state: Arc<AppState>, notify: impl Fn() + Send + 'static) {
    thread::spawn(move || run(&state, SYNC_PORT, &notify));
}

/// Outgoing syncs run on their own thread, one round at a time, so the
/// listener keeps answering: two devices syncing with each other at the same
/// moment must never wait on each other's busy loop.
#[derive(Default)]
struct Rounds {
    busy: Arc<AtomicBool>,
    changed: Arc<AtomicBool>,
}

impl Rounds {
    fn start(&self, state: &Arc<AppState>) {
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let (state, busy, changed) = (
            Arc::clone(state),
            Arc::clone(&self.busy),
            Arc::clone(&self.changed),
        );
        thread::spawn(move || {
            if sync_all(&state) > 0 {
                changed.store(true, Ordering::Release);
            }
            busy.store(false, Ordering::Release);
        });
    }

    fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::AcqRel)
    }
}

fn run(state: &Arc<AppState>, port: u16, notify: &dyn Fn()) {
    let mut listener: Option<TcpListener> = None;
    let mut _advertiser: Option<Advertiser> = None;
    let rounds = Rounds::default();
    let mut last_auto = Instant::now();
    let mut pushes = PushTracker::default();
    loop {
        if rounds.take_changed() {
            notify();
        }
        let Some(seq) = state.with(|s| Ok(s.sync_watch())).ok().flatten() else {
            listener = None;
            _advertiser = None;
            pushes = PushTracker::default();
            thread::sleep(POLL);
            continue;
        };
        if listener.is_none() {
            listener = bind(port);
            // Lets paired devices find this one if its address changed.
            _advertiser = local_ip()
                .ok()
                .and_then(|ip| Advertiser::start(ip, port).ok());
            // Catch up as soon as syncing becomes possible (unlock, new pairing).
            rounds.start(state);
            last_auto = Instant::now();
            pushes.synced(seq);
        }
        if pushes.due(seq, Instant::now()) {
            rounds.start(state);
            pushes.synced(seq);
        }
        if let Some(l) = &listener
            && let Ok((stream, from)) = l.accept()
            && serve(state, stream, from).is_ok_and(|r| r.changed > 0)
        {
            notify();
        }
        if last_auto.elapsed() >= AUTO_SYNC_INTERVAL {
            rounds.start(state);
            last_auto = Instant::now();
        }
        thread::sleep(POLL);
    }
}

/// Decides when a local change should be pushed: once the change number
/// stopped moving for [`PUSH_DELAY`].
#[derive(Default)]
struct PushTracker {
    synced: Option<u64>,
    seen: Option<(u64, Instant)>,
}

impl PushTracker {
    fn synced(&mut self, seq: u64) {
        self.synced = Some(seq);
        self.seen = None;
    }

    fn due(&mut self, seq: u64, now: Instant) -> bool {
        if self.synced.is_none_or(|s| seq <= s) {
            return false;
        }
        match self.seen {
            Some((last, since)) if last == seq => now.duration_since(since) >= PUSH_DELAY,
            _ => {
                self.seen = Some((seq, now));
                false
            }
        }
    }
}

fn bind(port: u16) -> Option<TcpListener> {
    let listener = TcpListener::bind(("0.0.0.0", port)).ok()?;
    listener.set_nonblocking(true).ok()?;
    Some(listener)
}

#[cfg(test)]
#[path = "sync_service_tests.rs"]
mod tests;

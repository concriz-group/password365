//! The SSH agent (Settings › Developer › Use the SSH agent): ssh and git ask
//! it to sign with SSH Key items. It listens on `~/.password365/agent.sock`
//! (a folder only this user can open) or, on Windows, the named pipe
//! `\\.\pipe\password365-ssh-agent`. Each key needs the user's OK once per
//! unlock; while the vault is locked the app comes forward and the request
//! waits a little for the unlock.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use interprocess::local_socket::{ListenerOptions, prelude::*};
use password365_ssh::{AgentKey, KeySource};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::state::SharedState;

/// Asks the main window to approve a key.
pub const APPROVE_EVENT: &str = "ssh-approve";
/// How long a request waits for the user (to unlock, or to answer).
const WAIT: Duration = Duration::from_secs(60);

/// Where ssh should connect (`IdentityAgent` in `~/.ssh/config`).
#[must_use]
pub fn address() -> String {
    if cfg!(windows) {
        r"\\.\pipe\password365-ssh-agent".to_owned()
    } else {
        socket_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }
}

fn socket_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".password365").join("agent.sock"))
}

/// What the approval prompt shows.
#[derive(Debug, Clone, Serialize)]
pub struct Approval {
    /// Answer with [`answer`] using this id.
    pub id: u64,
    /// Key (item) title.
    pub title: String,
    /// Key fingerprint.
    pub fingerprint: String,
}

/// Approvals: asked once per key per unlock.
#[derive(Default)]
pub struct Approvals {
    allowed: Mutex<HashSet<String>>,
    waiting: Mutex<HashMap<u64, Sender<bool>>>,
    next: AtomicU64,
    started: AtomicBool,
}

/// Shared with the commands that answer prompts.
pub type SharedApprovals = Arc<Approvals>;

/// The user answered prompt `id`.
#[tauri::command]
pub fn ssh_answer(approvals: tauri::State<'_, SharedApprovals>, id: u64, allow: bool) {
    if let Some(waiting) = approvals
        .waiting
        .lock()
        .ok()
        .and_then(|mut w| w.remove(&id))
    {
        let _ = waiting.send(allow);
    }
}

/// The keys and the prompt, from the app.
struct AppKeys<R: Runtime> {
    app: AppHandle<R>,
    state: SharedState,
    approvals: SharedApprovals,
}

impl<R: Runtime> AppKeys<R> {
    fn unlocked(&self) -> bool {
        self.state
            .with(|s| Ok(s.status().unlocked))
            .unwrap_or(false)
    }

    /// Waits for the unlock (bringing the app forward); `false` on time-out.
    fn wait_for_unlock(&self) -> bool {
        if self.unlocked() {
            return true;
        }
        if let Ok(mut allowed) = self.approvals.allowed.lock() {
            allowed.clear(); // a new unlock asks again
        }
        crate::auto_typer::bring_forward(&self.app);
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(250));
            if self.unlocked() {
                return true;
            }
        }
        false
    }
}

impl<R: Runtime> KeySource for AppKeys<R> {
    fn keys(&self) -> Vec<AgentKey> {
        let on = self
            .state
            .with(|s| Ok(s.settings().ssh_agent))
            .unwrap_or(false);
        if !on || !self.wait_for_unlock() {
            return Vec::new();
        }
        self.state
            .with(password365_app::AppService::ssh_agent_keys)
            .unwrap_or_default()
    }

    fn approve(&self, key: &AgentKey) -> bool {
        if self
            .approvals
            .allowed
            .lock()
            .is_ok_and(|a| a.contains(&key.item_id))
        {
            return true;
        }
        let id = self.approvals.next.fetch_add(1, Ordering::Relaxed);
        let (send, receive) = channel();
        if let Ok(mut waiting) = self.approvals.waiting.lock() {
            waiting.insert(id, send);
        }
        crate::auto_typer::bring_forward(&self.app);
        let prompt = Approval {
            id,
            title: key.title.clone(),
            fingerprint: key.fingerprint(),
        };
        let _ = self.app.emit(APPROVE_EVENT, prompt);
        let allowed = receive.recv_timeout(WAIT).unwrap_or(false);
        if let Ok(mut waiting) = self.approvals.waiting.lock() {
            waiting.remove(&id);
        }
        if allowed && let Ok(mut set) = self.approvals.allowed.lock() {
            set.insert(key.item_id.clone());
        }
        allowed
    }
}

fn listener() -> std::io::Result<interprocess::local_socket::Listener> {
    if cfg!(windows) {
        let name = "password365-ssh-agent"
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
        ListenerOptions::new().name(name).create_sync()
    } else {
        let path = socket_path().ok_or_else(|| std::io::Error::other("no home folder"))?;
        let folder = path
            .parent()
            .ok_or_else(|| std::io::Error::other("no folder"))?;
        std::fs::create_dir_all(folder)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))?;
        }
        let name = path
            .as_path()
            .to_fs_name::<interprocess::local_socket::GenericFilePath>()?;
        ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_sync()
    }
}

/// Starts listening (once); each connection gets its own thread.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let approvals = app.state::<SharedApprovals>().inner().clone();
    if approvals.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let listener = match listener() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("SSH agent unavailable: {e}");
            approvals.started.store(false, Ordering::SeqCst);
            return;
        }
    };
    let source = Arc::new(AppKeys {
        app: app.clone(),
        state: app.state::<SharedState>().inner().clone(),
        approvals,
    });
    thread::spawn(move || {
        for connection in listener.incoming().flatten() {
            let source = Arc::clone(&source);
            thread::spawn(move || {
                let mut stream = connection;
                password365_ssh::serve(&mut stream, source.as_ref());
            });
        }
    });
}

/// Starts the agent at launch when the setting is on.
pub fn start_if_enabled<R: Runtime>(app: &AppHandle<R>) {
    let on = app
        .state::<SharedState>()
        .with(|s| Ok(s.settings().ssh_agent))
        .unwrap_or(false);
    if on {
        start(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_in_a_private_folder() {
        if cfg!(windows) {
            assert!(address().starts_with(r"\\.\pipe\"));
        } else {
            assert!(address().ends_with("/.password365/agent.sock"));
        }
    }
}

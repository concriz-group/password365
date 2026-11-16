//! The SSH agent (Settings › Developer › Use the SSH agent): ssh and git ask
//! it to sign with SSH Key items. It listens on `~/.password365/agent.sock`
//! (a folder only this user can open) or, on Windows, the named pipe
//! `\\.\pipe\password365-ssh-agent`. Each key needs the user's OK once per
//! unlock *and per program* (the prompt names the program that asked, found
//! from the connection's process); approvals end when the vault locks. While
//! the vault is locked the app comes forward and the request waits a little
//! for the unlock.

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
    /// The program asking (its file name), when the system tells.
    pub program: Option<String>,
}

/// Approvals: asked once per key, per program, per unlock.
#[derive(Default)]
pub struct Approvals {
    /// (unlock it belongs to, (item id, program)).
    allowed: Mutex<(u64, HashSet<(String, String)>)>,
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

/// The program at the other end of an agent connection: its path when the
/// system tells (macOS, Linux), else its process id; `None` if unknown.
fn client_program(stream: &interprocess::local_socket::Stream) -> Option<String> {
    let pid = stream.peer_creds().ok()?.pid()?;
    program_path(i64::from(pid)).or_else(|| Some(format!("process {pid}")))
}

#[cfg(target_os = "macos")]
fn program_path(pid: i64) -> Option<String> {
    let out = std::process::Command::new("/bin/ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let path = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!path.is_empty()).then_some(path)
}

#[cfg(target_os = "linux")]
fn program_path(pid: i64) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.display().to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn program_path(_pid: i64) -> Option<String> {
    None
}

impl<R: Runtime> AppKeys<R> {
    /// Which unlock this is (approvals from an earlier one no longer count).
    fn unlock_number(&self) -> u64 {
        self.state.with(|s| Ok(s.unlock_number())).unwrap_or(0)
    }

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

/// One connection's view of the keys: approvals are for its program.
struct ClientKeys<R: Runtime> {
    keys: Arc<AppKeys<R>>,
    program: Option<String>,
}

impl<R: Runtime> KeySource for ClientKeys<R> {
    fn keys(&self) -> Vec<AgentKey> {
        self.keys.list()
    }

    fn approve(&self, key: &AgentKey) -> bool {
        self.keys.approve(key, self.program.as_deref())
    }
}

impl<R: Runtime> AppKeys<R> {
    fn list(&self) -> Vec<AgentKey> {
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

    fn approve(&self, key: &AgentKey, program: Option<&str>) -> bool {
        let unlock = self.unlock_number();
        let wanted = (key.item_id.clone(), program.unwrap_or("?").to_owned());
        if let Ok(mut allowed) = self.approvals.allowed.lock() {
            if allowed.0 != unlock {
                *allowed = (unlock, HashSet::new()); // locked since: ask again
            }
            if allowed.1.contains(&wanted) {
                return true;
            }
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
            program: program.map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .map_or_else(|| p.to_owned(), |n| n.to_string_lossy().into_owned())
            }),
        };
        let _ = self.app.emit(APPROVE_EVENT, prompt);
        let allowed = receive.recv_timeout(WAIT).unwrap_or(false);
        if let Ok(mut waiting) = self.approvals.waiting.lock() {
            waiting.remove(&id);
        }
        // Still the same unlock (the vault did not lock while the prompt was up).
        if allowed
            && self.unlock_number() == unlock
            && let Ok(mut set) = self.approvals.allowed.lock()
            && set.0 == unlock
        {
            set.1.insert(wanted);
        }
        allowed && self.unlocked()
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
            let keys = Arc::clone(&source);
            thread::spawn(move || {
                let mut stream = connection;
                let client = ClientKeys {
                    keys,
                    program: client_program(&stream),
                };
                password365_ssh::serve(&mut stream, &client);
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

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn the_program_behind_a_process_is_found() {
        let me = program_path(i64::from(std::process::id())).unwrap();
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_string_lossy();
        assert!(me.ends_with(name.as_ref()), "{me} vs {name}");
    }

    #[test]
    fn the_address_is_in_a_private_folder() {
        if cfg!(windows) {
            assert!(address().starts_with(r"\\.\pipe\"));
        } else {
            assert!(address().ends_with("/.password365/agent.sock"));
        }
    }
}

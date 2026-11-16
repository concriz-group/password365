//! Talks to the agent with the real OpenSSH tools (skipped when they are missing).
#![cfg(unix)]
#![allow(clippy::unwrap_used)] // a test: stop loudly

use std::os::unix::net::UnixListener;
use std::process::Command;
use std::thread;

use password365_ssh::{AgentKey, KeySource, load_key, serve};

struct One(String);

impl KeySource for One {
    fn keys(&self) -> Vec<AgentKey> {
        vec![load_key("i1", "GitHub", &self.0, None).unwrap()]
    }
    fn approve(&self, _key: &AgentKey) -> bool {
        true
    }
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-?").output().is_ok()
}

#[test]
fn ssh_add_lists_and_ssh_keygen_signs_through_the_agent() {
    if !have("ssh-add") || !have("ssh-keygen") {
        eprintln!("OpenSSH tools not installed: skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let key_file = dir.path().join("id");
    let made = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"])
        .arg(&key_file)
        .status()
        .unwrap();
    assert!(made.success());
    let pem = std::fs::read_to_string(&key_file).unwrap();
    let socket = dir.path().join("agent.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let source = One(pem);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            serve(&mut stream, &source);
        }
    });

    let listed = Command::new("ssh-add")
        .arg("-L")
        .env("SSH_AUTH_SOCK", &socket)
        .output()
        .unwrap();
    let listed = String::from_utf8(listed.stdout).unwrap();
    let public = std::fs::read_to_string(key_file.with_extension("pub")).unwrap();
    assert!(listed.starts_with("ssh-ed25519 "), "{listed}");
    assert_eq!(
        listed.split_whitespace().nth(1),
        public.split_whitespace().nth(1)
    );

    // Sign with only the public key on disk: ssh-keygen must ask the agent.
    std::fs::remove_file(&key_file).unwrap();
    let message = dir.path().join("message");
    std::fs::write(&message, "hello").unwrap();
    let signed = Command::new("ssh-keygen")
        .args(["-Y", "sign", "-n", "file", "-f"])
        .arg(key_file.with_extension("pub"))
        .arg(&message)
        .env("SSH_AUTH_SOCK", &socket)
        .output()
        .unwrap();
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    let allowed = dir.path().join("allowed");
    std::fs::write(&allowed, format!("me {}", public.trim())).unwrap();
    let verified = Command::new("ssh-keygen")
        .args(["-Y", "verify", "-n", "file", "-I", "me", "-f"])
        .arg(&allowed)
        .arg("-s")
        .arg(message.with_extension("sig"))
        .stdin(std::fs::File::open(&message).unwrap())
        .output()
        .unwrap();
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
}

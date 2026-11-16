//! End-to-end tests running the real `password365` binary (production KDF settings).
#![allow(clippy::unwrap_used)] // test helpers

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const PASSWORD: &str = "end to end master password";

fn password365(dir: &Path, stdin: &str, args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_password365"))
        .arg("--store")
        .arg(dir.join("store.qlvault"))
        .arg("--secret-key-file")
        .arg(dir.join("secret-key"))
        .arg("--password-stdin")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn password365");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn init_add_show_and_reject_wrong_password() {
    let dir = tempfile::tempdir().unwrap();
    let pw = format!("{PASSWORD}\n");

    let init = password365(dir.path(), &format!("{pw}{pw}"), &["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(stdout(&init).contains("P365-"));

    let add = password365(
        dir.path(),
        &format!("{pw}e2e-secret\ne2e-secret\n"),
        &["item", "add", "--title", "E2E"],
    );
    assert!(add.status.success());

    let show = password365(dir.path(), &pw, &["item", "show", "E2E", "--reveal"]);
    assert!(stdout(&show).contains("e2e-secret"));

    let wrong = password365(dir.path(), "not the password\n", &["item", "list"]);
    assert!(!wrong.status.success());
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("wrong master password"));
}

#[test]
fn generate_works_without_a_store() {
    let dir = tempfile::tempdir().unwrap();
    // Some list words contain "-" ("yo-yo"): use a separator no word has.
    let out = password365(
        dir.path(),
        "",
        &["generate", "--passphrase", "--separator", "+", "-q"],
    );
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim().split('+').count(), 5);
}

#[test]
fn delete_confirmation_reads_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let pw = format!("{PASSWORD}\n");
    password365(dir.path(), &format!("{pw}{pw}"), &["init"]);
    password365(
        dir.path(),
        &pw,
        &["item", "add", "--title", "Tmp", "--generate"],
    );
    let declined = password365(dir.path(), &format!("{pw}n\n"), &["item", "rm", "Tmp"]);
    assert!(!declined.status.success());
    let accepted = password365(dir.path(), &format!("{pw}y\n"), &["item", "rm", "Tmp"]);
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
}

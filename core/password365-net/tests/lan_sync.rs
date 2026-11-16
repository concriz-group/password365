//! Two paired stores sync over a real TCP connection on localhost.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread;

use password365_net::NetError;
use password365_net::tcp::{accept_sync, sync_to};
use password365_vault::{Session, VaultError};

mod common;
use common::{Pair, add, paired, titles};

/// B listens, A connects; returns both sessions and both results.
fn run(
    mut a: Session,
    mut b: Session,
) -> (
    Session,
    Session,
    password365_net::Result<password365_net::sync::SyncOutcome>,
    password365_net::Result<password365_net::sync::SyncOutcome>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = thread::spawn(move || {
        let result = accept_sync(&listener, &mut b);
        (b, result)
    });
    let result_a = sync_to(addr, &mut a);
    let (b, result_b) = server.join().expect("server thread");
    (a, b, result_a, result_b)
}

#[test]
fn paired_devices_sync_over_tcp() {
    let Pair { _dir, mut a, mut b } = paired();
    add(&mut a, "From A");
    add(&mut b, "From B");
    let (a, b, ra, rb) = run(a, b);
    let (ra, rb) = (ra.expect("a"), rb.expect("b"));
    assert_eq!((ra.report.changed.len(), rb.report.changed.len()), (1, 1));
    assert_eq!((ra.peer, rb.peer), (b.device_id(), a.device_id()));
    assert_eq!(titles(&a), vec!["From A", "From B"]);
    assert_eq!(titles(&b), titles(&a));
    assert_eq!(a.changes_pending_for(b.device_id()).expect("peer"), 0);
}

#[test]
fn a_device_that_is_not_paired_is_refused() {
    let Pair { _dir, a, .. } = paired();
    let stranger = paired().a;
    let (_, _, ra, rb) = run(a, stranger);
    assert!(matches!(
        ra,
        Err(NetError::Vault(VaultError::UnknownDevice))
    ));
    assert!(rb.is_err());
}

#[test]
fn garbage_on_the_socket_fails_cleanly() {
    let Pair { _dir, mut a, .. } = paired();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let client = thread::spawn(move || {
        let mut s = TcpStream::connect(addr).expect("connect");
        s.write_all(&[0, 3, 1, 2, 3]).expect("write");
    });
    let result = accept_sync(&listener, &mut a);
    client.join().expect("client");
    assert!(result.is_err());
    assert!(titles(&a).is_empty(), "nothing changed");
}

#[test]
fn attachments_cross_both_ways_over_tcp() {
    let Pair { _dir, mut a, mut b } = paired();
    let on_a = add(&mut a, "Passport");
    let on_b = add(&mut b, "Contract");
    let big: Vec<u8> = (0..300_000u32).map(|i| (i % 249) as u8).collect();
    let file_a = a
        .attach(on_a, "passport.pdf", "application/pdf", &big)
        .expect("attach a");
    let file_b = b
        .attach(on_b, "contract.txt", "text/plain", b"signed")
        .expect("attach b");
    let (a, b, ra, rb) = run(a, b);
    let (ra, rb) = (ra.expect("a"), rb.expect("b"));
    assert_eq!((ra.report.files, rb.report.files), (1, 1));
    assert_eq!(
        b.attachment_bytes(on_a, file_a.id)
            .expect("on b")
            .as_slice(),
        big.as_slice()
    );
    assert_eq!(
        a.attachment_bytes(on_b, file_b.id)
            .expect("on a")
            .as_slice(),
        b"signed"
    );
}

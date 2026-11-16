//! Background sync: serving an incoming connection and "Sync now".

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::thread;

use password365_app::item_input::ItemInput;

use super::*;
use crate::pairing_slots::PairingSlots;
use crate::state::test_support::state;
use crate::sync_commands::{
    approve_device, invite_on, join_connect, join_finish, join_wait_approval, wait_for_device,
};

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const PW: &str = "correct horse battery";

/// A with one item, B joined from it.
fn paired() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Arc<AppState>,
    Arc<AppState>,
) {
    let (da, a) = state();
    a.with(|s| s.create_account(PW.into(), &PW.into())).unwrap();
    let input = ItemInput {
        title: "Mail".into(),
        password: Some("pw".into()),
        ..ItemInput::default()
    };
    a.with(|s| s.create_item(&input)).unwrap();
    let a = Arc::new(a);
    let slots = Arc::new(PairingSlots::default());
    let invite = invite_on(&a, &slots, LOCALHOST).unwrap();
    let vault = a.with(|s| Ok(s.vaults()?[0].id.clone())).unwrap();
    let (a2, s2) = (Arc::clone(&a), Arc::clone(&slots));
    let (typed, read_typed) = std::sync::mpsc::channel::<String>();
    let host = thread::spawn(move || {
        wait_for_device(&s2).unwrap();
        approve_device(&a2, &s2, &read_typed.recv().unwrap(), &[vault]).unwrap();
    });
    let (db, b) = state();
    let joining = PairingSlots::default();
    typed
        .send(join_connect(&joining, &invite.code).unwrap())
        .unwrap();
    join_wait_approval(&joining).unwrap();
    join_finish(&b, &joining, PW.into(), "Phone").unwrap();
    host.join().unwrap();
    (da, db, a, Arc::new(b))
}

#[test]
fn an_incoming_sync_is_served_and_sync_now_reaches_the_device() {
    let (_da, _db, a, b) = paired();
    assert!(a.with(|s| Ok(s.sync_listener_wanted())).unwrap());

    // A listens on a free port; point B's record of A there.
    let listener = TcpListener::bind((LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let a_id = b.with(|s| Ok(s.devices()?[0].id.clone())).unwrap();
    b.with(|s| s.set_device_address(&a_id, &format!("127.0.0.1:{port}")))
        .unwrap();

    let a2 = Arc::clone(&a);
    let server = thread::spawn(move || {
        let (stream, from) = listener.accept().unwrap();
        serve(&a2, stream, from).unwrap()
    });
    let on_b = sync_device(&b, &a_id).unwrap();
    let on_a = server.join().unwrap();
    assert_eq!((on_b.changed, on_a.changed), (1, 0));
    let titles = b.with(|s| s.items("", None)).unwrap();
    assert_eq!(titles[0].title, "Mail");
}

#[test]
fn unreachable_devices_are_skipped() {
    let (_da, _db, _a, b) = paired();
    // Nothing listens on the remembered address in tests.
    assert_eq!(sync_all(&b), 0);
}

#[test]
fn a_locked_vault_serves_nobody() {
    let (_da, _db, a, _b) = paired();
    a.with(|s| {
        s.lock();
        Ok(())
    })
    .unwrap();
    let listener = TcpListener::bind((LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let client = thread::spawn(move || std::net::TcpStream::connect(addr).unwrap());
    let (stream, from) = listener.accept().unwrap();
    assert_eq!(serve(&a, stream, from).unwrap_err().code(), "locked");
    client.join().unwrap();
    assert!(!a.with(|s| Ok(s.sync_listener_wanted())).unwrap());
}

#[test]
fn a_device_whose_address_changed_is_found_again() {
    let (_da, _db, a, b) = paired();
    let a_id = b.with(|s| Ok(s.devices()?[0].id.clone())).unwrap();
    // B remembers a dead address; A now listens elsewhere.
    let dead = TcpListener::bind((LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap();
    b.with(|s| s.set_device_address(&a_id, &dead.to_string()))
        .unwrap();
    let listener = TcpListener::bind((LOCALHOST, 0)).unwrap();
    let a_addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, from) = listener.accept().unwrap();
        serve(&a, stream, from).unwrap()
    });
    let result = sync_device_with(&b, &a_id, || vec![dead, a_addr]).unwrap();
    server.join().unwrap();
    assert_eq!(result.changed, 1);
    let remembered = b.with(|s| s.device_address(&a_id)).unwrap();
    assert_eq!(
        remembered.ip(),
        a_addr.ip(),
        "the new address is remembered"
    );
}

#[test]
fn a_different_device_at_a_discovered_address_is_not_synced() {
    let (_da, _db, _a, b) = paired();
    let (_dc, _dd, c, _d) = paired();
    let a_id = b.with(|s| Ok(s.devices()?[0].id.clone())).unwrap();
    let dead = TcpListener::bind((LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap();
    b.with(|s| s.set_device_address(&a_id, &dead.to_string()))
        .unwrap();
    let listener = TcpListener::bind((LOCALHOST, 0)).unwrap();
    let c_addr = listener.local_addr().unwrap();
    let stranger = thread::spawn(move || {
        let (stream, from) = listener.accept().unwrap();
        serve(&c, stream, from)
    });
    let err = sync_device_with(&b, &a_id, || vec![c_addr]).unwrap_err();
    assert_eq!(err.code(), "io");
    assert!(
        stranger.join().unwrap().is_err(),
        "the stranger learns nothing and syncs nothing"
    );
}

#[test]
fn nothing_found_is_reported() {
    let (_da, _db, _a, b) = paired();
    let a_id = b.with(|s| Ok(s.devices()?[0].id.clone())).unwrap();
    // A dead address: the real sync port may be taken by a running Password365.
    let dead = TcpListener::bind((LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap();
    b.with(|s| s.set_device_address(&a_id, &dead.to_string()))
        .unwrap();
    let err = sync_device_with(&b, &a_id, Vec::new).unwrap_err();
    assert_eq!(err.code(), "io");
}

#[test]
fn a_sync_round_runs_on_its_own_thread_one_at_a_time() {
    let (_d, s) = state();
    let s = Arc::new(s);
    let rounds = Rounds::default();
    rounds.busy.store(true, Ordering::Release);
    rounds.start(&s);
    assert!(
        rounds.busy.load(Ordering::Acquire),
        "no second round while one runs"
    );
    rounds.busy.store(false, Ordering::Release);
    rounds.start(&s);
    while rounds.busy.load(Ordering::Acquire) {
        thread::yield_now();
    }
    assert!(!rounds.take_changed(), "nothing paired, nothing changed");
}

#[test]
fn a_local_change_is_pushed_once_it_settles() {
    let t0 = Instant::now();
    let mut p = PushTracker::default();
    assert!(
        !p.due(5, t0),
        "nothing synced yet: the first round covers it"
    );
    p.synced(5);
    assert!(!p.due(5, t0), "nothing new");
    assert!(!p.due(6, t0), "just changed");
    assert!(!p.due(7, t0 + PUSH_DELAY), "still changing");
    assert!(!p.due(7, t0 + PUSH_DELAY + Duration::from_millis(100)));
    assert!(p.due(7, t0 + PUSH_DELAY * 2 + Duration::from_millis(100)));
    p.synced(7);
    assert!(!p.due(7, t0 + PUSH_DELAY * 3));
}

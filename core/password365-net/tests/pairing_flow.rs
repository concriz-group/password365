//! A new device joins an existing one over localhost TCP, then they sync.

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::atomic::AtomicBool;
use std::thread;

use password365_crypto::kdf::KdfParams;
use password365_crypto::secret_key::AccountSecretKey;
use password365_model::{ItemDetails, ItemKind, ItemOverview};
use password365_net::NetError;
use password365_net::pairing::{HostPairing, JoinListener, Joining, PairingHost, local_ip};
use password365_net::tcp::{accept_sync, sync_to};
use password365_vault::{Credentials, Session, Store, VaultError};

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

struct Existing {
    dir: tempfile::TempDir,
    session: Session,
    secret_key: AccountSecretKey,
}

fn existing() -> Existing {
    let dir = tempfile::tempdir().expect("temp dir");
    let secret_key = AccountSecretKey::generate().expect("random");
    let creds = Credentials {
        password: "master".into(),
        secret_key: secret_key.clone(),
    };
    let mut session = Store::create(
        &dir.path().join("a.qlvault"),
        &creds,
        KdfParams::insecure_for_tests(),
    )
    .expect("create");
    session.set_device_name("Laptop").expect("name");
    let vault = session.vault_by_name("Personal").expect("vault").id;
    session
        .add_item(
            vault,
            ItemKind::Login,
            ItemOverview::new("Mail").expect("title"),
            ItemDetails::default(),
        )
        .expect("add");
    Existing {
        dir,
        session,
        secret_key,
    }
}

#[test]
fn a_new_device_joins_and_syncs() {
    let Existing {
        dir,
        mut session,
        secret_key,
    } = existing();
    let host = PairingHost::open(&session, LOCALHOST).expect("open");
    let code = host.invite_code();
    let vault = session.vault_by_name("Personal").expect("vault").id;

    // The existing device's sync listener; any free port stands in for SYNC_PORT.
    let a_listener = TcpListener::bind((LOCALHOST, 0)).expect("bind");
    let a_port = a_listener.local_addr().expect("addr").port();
    let inviter = thread::spawn(move || {
        let pairing = host.accept().expect("accept");
        let shown = pairing.verification_code().to_owned();
        let peer = pairing
            .approve(&mut session, &secret_key, &[vault], 9)
            .expect("approve");
        assert_eq!(peer.vaults, Some(vec![vault]), "only the vault it was sent");
        (session, shown, peer)
    });

    let mut joining = Joining::connect(&code).expect("connect");
    let code_on_new = joining.verification_code().to_owned();
    let bundle = joining.receive_bundle().expect("bundle");
    let path = dir.path().join("b.qlvault");
    let wrong = Store::join(
        &path,
        &bundle,
        "nope".into(),
        KdfParams::insecure_for_tests(),
    );
    assert!(
        matches!(wrong, Err(VaultError::WrongCredentials)),
        "a typo can be retried"
    );
    let (mut joined, _) = Store::join(
        &path,
        &bundle,
        "master".into(),
        KdfParams::insecure_for_tests(),
    )
    .expect("join");
    joined.set_device_name("Phone").expect("name");
    joining.complete(&mut joined, a_port).expect("complete");

    let (mut session, code_on_existing, peer) = inviter.join().expect("inviter");
    assert_eq!(code_on_new, code_on_existing);
    assert_eq!(
        (peer.device_id, peer.name.as_str()),
        (joined.device_id(), "Phone")
    );
    let inviter_peer = joined.peers()[0].clone();
    assert_eq!(inviter_peer.name, "Laptop");
    let addr = inviter_peer.addr.expect("address remembered");
    assert_eq!(addr, format!("127.0.0.1:{a_port}"));
    assert_eq!(
        session
            .peer(peer.device_id)
            .expect("pinned")
            .addr
            .as_deref(),
        Some("127.0.0.1:9")
    );

    // First sync, initiated by the new device to the address it remembered.
    let server = thread::spawn(move || {
        let outcome = accept_sync(&a_listener, &mut session).expect("serve");
        assert_eq!(outcome.report.changed.len(), 0);
        session
    });
    sync_to(addr.parse().expect("addr"), &mut joined).expect("sync");
    server.join().expect("server");
    let titles: Vec<&str> = joined
        .items()
        .iter()
        .map(|i| i.overview.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Mail"]);
}

#[test]
fn rejecting_cancels_on_the_new_device() {
    let Existing { session, .. } = existing();
    let host = PairingHost::open(&session, LOCALHOST).expect("open");
    let code = host.invite_code();
    let inviter = thread::spawn(move || host.accept().expect("accept").reject().expect("reject"));
    let mut joining = Joining::connect(&code).expect("connect");
    assert!(matches!(joining.receive_bundle(), Err(NetError::Cancelled)));
    inviter.join().expect("inviter");
}

#[test]
fn cancelling_on_the_new_device_stops_the_existing_one() {
    let Existing {
        mut session,
        secret_key,
        ..
    } = existing();
    let host = PairingHost::open(&session, LOCALHOST).expect("open");
    let code = host.invite_code();
    let inviter = thread::spawn(move || {
        let result = host
            .accept()
            .expect("accept")
            .approve(&mut session, &secret_key, &[], 9);
        (session, result)
    });
    let mut joining = Joining::connect(&code).expect("connect");
    let _ = joining.receive_bundle().expect("bundle");
    joining.cancel().expect("cancel");
    let (session, result) = inviter.join().expect("inviter");
    assert!(matches!(result, Err(NetError::Cancelled)));
    assert!(session.peers().is_empty(), "nobody was paired");
}

#[test]
fn a_cancelled_invite_stops_waiting_and_closes() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let Existing { session, .. } = existing();
    let host = PairingHost::open(&session, LOCALHOST).expect("open");
    let code = host.invite_code();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let waiting = thread::spawn(move || host.accept_cancellable(&flag).map(|_| ()));
    thread::sleep(std::time::Duration::from_millis(120));
    cancel.store(true, Ordering::Relaxed);
    assert!(matches!(
        waiting.join().expect("thread"),
        Err(NetError::Cancelled)
    ));
    assert!(Joining::connect(&code).is_err(), "nobody listens any more");
}

#[test]
fn a_cancellable_invite_still_pairs() {
    use std::sync::atomic::AtomicBool;
    let Existing { session, .. } = existing();
    let host = PairingHost::open(&session, LOCALHOST).expect("open");
    let code = host.invite_code();
    let waiting = thread::spawn(move || {
        let never = AtomicBool::new(false);
        host.accept_cancellable(&never)
            .map(|p| p.verification_code().to_owned())
    });
    let joining = Joining::connect(&code).expect("connect");
    assert_eq!(
        waiting.join().expect("thread").expect("paired"),
        joining.verification_code()
    );
}

#[test]
fn bad_codes_fail_before_connecting() {
    assert!(matches!(
        Joining::connect("hello"),
        Err(NetError::Invite(_))
    ));
    let Existing { session, .. } = existing();
    let mut invite = password365_sync::pairing::Invite::parse(
        &PairingHost::open(&session, LOCALHOST)
            .expect("open")
            .invite_code(),
    )
    .expect("own code parses");
    invite.addr = None;
    assert!(matches!(
        Joining::connect(&invite.to_code()),
        Err(NetError::NoAddress)
    ));
    invite.addr = Some("not an address".into());
    assert!(matches!(
        Joining::connect(&invite.to_code()),
        Err(NetError::Invite(_))
    ));
}

#[test]
fn the_local_ip_is_a_real_interface() {
    // A machine with no network at all has no route; that is reported, not faked.
    if let Ok(ip) = local_ip() {
        assert!(!ip.is_unspecified());
    }
}

#[test]
fn a_new_device_that_shows_the_code_is_joined_by_the_device_with_the_vault() {
    let Existing {
        dir,
        session,
        secret_key,
    } = existing();
    let vault = session.vault_by_name("Personal").expect("vault").id;
    // The NEW device (no vault yet) listens and shows the code.
    let listener = JoinListener::open(LOCALHOST).expect("open");
    let code = listener.invite_code();
    assert!(
        listener
            .invite_link()
            .starts_with("https://password365.app/join#PASSWORD365-PAIR:1:")
    );
    let cancel = AtomicBool::new(false);
    let new_device = thread::spawn(move || {
        let mut joining = listener.accept_cancellable(&cancel).expect("accept");
        let shown = joining.verification_code().to_owned();
        let bundle = joining.receive_bundle().expect("bundle");
        (joining, shown, bundle)
    });

    // The device WITH the vault scans the code and connects.
    let mut pairing = HostPairing::connect(&code).expect("connect");
    let on_vault_device = pairing.verification_code().to_owned();
    pairing
        .send_bundle(&session, &secret_key, &[vault])
        .expect("send");
    let waiting = thread::spawn(move || (pairing.wait_joined(9), session));

    let (joining, shown_on_new, bundle) = new_device.join().expect("new device");
    assert_eq!(
        shown_on_new, on_vault_device,
        "both screens show the same six digits"
    );
    let path = dir.path().join("b.qlvault");
    let (mut joined, _) = Store::join(
        &path,
        &bundle,
        "master".into(),
        KdfParams::insecure_for_tests(),
    )
    .expect("join");
    joined.set_device_name("Desktop").expect("name");
    joining.complete(&mut joined, 9).expect("complete");

    let (peer, session) = waiting.join().expect("host");
    let peer = peer.expect("pinned");
    assert_eq!(
        (peer.device_id, peer.name.as_str()),
        (joined.device_id(), "Desktop")
    );
    assert_eq!(peer.vaults, Some(vec![vault]));
    assert!(session.vault_by_name("Personal").is_some());
}

#[test]
fn a_connection_without_the_one_time_secret_is_refused_by_the_listening_new_device() {
    use password365_sync::pairing::Invite;
    let listener = JoinListener::open(LOCALHOST).expect("open");
    let mut forged = Invite::parse(&listener.invite_code()).expect("parse");
    forged.psk[0] ^= 0xff;
    let cancel = AtomicBool::new(false);
    let waiting = thread::spawn(move || listener.accept_cancellable(&cancel).is_err());
    // The secret is checked in the handshake's last message, so the connecting side may get that far;
    // what matters is that the new device never accepts it and no account can be sent.
    let _ = HostPairing::connect(&forged.to_code());
    assert!(
        waiting.join().expect("listener"),
        "and the new device does not accept it either"
    );
}

#[test]
fn a_new_device_that_nobody_scans_gives_up_when_cancelled() {
    let listener = JoinListener::open(LOCALHOST).expect("open");
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&cancel);
    let waiting = thread::spawn(move || listener.accept_cancellable(&flag).err());
    thread::sleep(std::time::Duration::from_millis(150));
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(matches!(
        waiting.join().expect("thread"),
        Some(NetError::Cancelled)
    ));
}

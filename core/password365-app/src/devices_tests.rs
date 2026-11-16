//! Pairing and syncing two `AppService`s over localhost TCP.

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::thread;

use password365_net::pairing::Joining;
use password365_net::tcp;

use super::*;
use crate::secret_key_store::SecretKeyStore;
use crate::service::tests::{PW, fresh, login, with_account};

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

/// A (existing account with one item) pairs B (fresh install).
fn paired() -> (
    crate::service::tests::Fixture,
    crate::service::tests::Fixture,
) {
    let mut a = with_account();
    a.app.create_item(&login("Mail", "pw")).unwrap();
    a.app.rename_this_device("Laptop").unwrap();
    let personal = a.app.vaults().unwrap()[0].id.clone();
    let host = a.app.start_pairing(LOCALHOST).unwrap();
    let code = host.invite_code();
    let inviter = thread::spawn(move || {
        let mut pairing = host.accept().unwrap();
        a.app
            .send_pairing_bundle(&mut pairing, &[personal])
            .unwrap();
        let peer = pairing.wait_joined(tcp::SYNC_PORT).unwrap();
        let device = a.app.pin_device(peer).unwrap();
        (a, device)
    });

    let mut b = fresh();
    let mut joining = Joining::connect(&code).unwrap();
    let bundle = joining.receive_bundle().unwrap();
    assert_eq!(
        b.app.join_account(&bundle, "wrong".into()).unwrap_err(),
        AppError::WrongCredentials
    );
    assert!(
        !b.app.status().initialized,
        "a wrong password writes nothing"
    );
    b.app.join_account(&bundle, PW.into()).unwrap();
    b.app.complete_join(joining, "Phone").unwrap();
    let (a, device) = inviter.join().unwrap();
    assert_eq!(device.name, "Phone");
    assert_eq!(device.addr, Some(format!("127.0.0.1:{}", tcp::SYNC_PORT)));
    (a, b)
}

/// B connects to A (listening on a free port) and both sync.
fn sync(
    mut a: crate::service::tests::Fixture,
    b: &mut crate::service::tests::Fixture,
) -> (crate::service::tests::Fixture, SyncResultDto, SyncResultDto) {
    let listener = TcpListener::bind((LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let key = a.app.network_key().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, from) = tcp::accept(&listener, &key).unwrap();
        let result = a.app.sync_over(&mut stream, Some(from.ip())).unwrap();
        (a, result)
    });
    let mut stream = tcp::connect(addr, &b.app.network_key().unwrap()).unwrap();
    let on_b = b.app.sync_over(&mut stream, None).unwrap();
    let (a, on_a) = server.join().unwrap();
    (a, on_a, on_b)
}

#[test]
fn a_new_device_joins_then_syncs_and_both_list_each_other() {
    let (a, mut b) = paired();
    assert!(
        b.keys.load().unwrap().is_some(),
        "Secret Key kept in the keychain"
    );
    assert_eq!(b.app.this_device().unwrap().name, "Phone");
    let (mut a, on_a, on_b) = sync(a, &mut b);
    assert_eq!((on_a.changed, on_b.changed), (0, 1));
    assert!(on_b.conflicts.is_empty());
    assert_eq!(on_b.device.name, "Laptop");
    let titles: Vec<String> = b
        .app
        .items("", None)
        .unwrap()
        .into_iter()
        .map(|i| i.title)
        .collect();
    assert_eq!(titles, vec!["Mail"]);

    let on_a_list = a.app.devices().unwrap();
    assert_eq!(on_a_list.len(), 1);
    assert_eq!(on_a_list[0].pending_changes, 0);
    assert!(on_a_list[0].last_sync.is_some());
    let b_id = on_a_list[0].id.clone();
    assert_eq!(a.app.device_address(&b_id).unwrap().port(), tcp::SYNC_PORT);

    a.app.set_device_address(&b_id, " 10.0.0.9:47480 ").unwrap();
    assert_eq!(
        a.app.device_address(&b_id).unwrap().to_string(),
        "10.0.0.9:47480"
    );
    assert_eq!(
        a.app
            .set_device_address(&b_id, "nowhere")
            .unwrap_err()
            .code(),
        "validation"
    );
    assert_eq!(
        a.app.device_key(&b_id).unwrap(),
        b.app.network_key().unwrap().public()
    );
    assert!(a.app.sync_listener_wanted());
    a.app.remove_device(&b_id).unwrap();
    assert!(
        !a.app.sync_listener_wanted(),
        "no paired device: nothing listens"
    );
    assert!(a.app.devices().unwrap().is_empty());
    assert_eq!(
        a.app.remove_device(&b_id).unwrap_err(),
        AppError::NotFound("device")
    );
}

#[test]
fn concurrent_passwords_are_flagged_for_review_until_dismissed() {
    let (a, mut b) = paired();
    let (mut a, _, _) = sync(a, &mut b);
    let id = b.app.items("", None).unwrap()[0].id.clone();
    for (fx, pw) in [(&mut a, "from-a"), (&mut b, "from-b")] {
        let item = fx.app.item(&id).unwrap();
        let input = crate::item_input::ItemInput {
            password: Some(pw.into()),
            ..login("Mail", pw)
        };
        fx.app.update_item(&id, item.version, &input).unwrap();
    }
    let (mut a, on_a, on_b) = sync(a, &mut b);
    assert_eq!(
        (on_a.conflicts.clone(), on_b.conflicts),
        (vec![id.clone()], vec![id.clone()])
    );
    assert!(a.app.items("", None).unwrap()[0].needs_review);
    assert!(a.app.item(&id).unwrap().summary.needs_review);
    a.app.mark_reviewed(&id).unwrap();
    assert!(!a.app.item(&id).unwrap().summary.needs_review);
    assert!(
        b.app.item(&id).unwrap().summary.needs_review,
        "each device reviews on its own"
    );
    assert_eq!(
        a.app.mark_reviewed("nope").unwrap_err().code(),
        "validation"
    );
}

#[test]
fn sync_files_carry_changes_without_a_network() {
    let (mut a, mut b) = paired();
    let b_id = a.app.devices().unwrap()[0].id.clone();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("for-phone.qlsync");
    std::fs::write(&path, b"old file").unwrap();
    assert_eq!(
        a.app.export_sync_file(&b_id, &path).unwrap(),
        1,
        "replaces an older file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let result = b.app.import_sync_file(&path).unwrap();
    assert_eq!((result.changed, result.device.name.as_str()), (1, "Laptop"));
    assert_eq!(b.app.items("", None).unwrap()[0].title, "Mail");

    assert_eq!(
        b.app
            .import_sync_file(&dir.path().join("missing.qlsync"))
            .unwrap_err()
            .code(),
        "io"
    );
    std::fs::write(&path, b"junk").unwrap();
    assert_eq!(
        b.app.import_sync_file(&path).unwrap_err().code(),
        "validation"
    );
    let blocked = dir.path().join("no-such-dir").join("x").join("f.qlsync");
    std::fs::write(dir.path().join("no-such-dir"), b"a file, not a dir").unwrap();
    assert_eq!(
        a.app.export_sync_file(&b_id, &blocked).unwrap_err().code(),
        "io"
    );
    assert_eq!(
        a.app.export_sync_file("nope", &path).unwrap_err().code(),
        "validation"
    );
}

#[test]
fn an_existing_account_cannot_join_another() {
    let mut a = with_account();
    assert_eq!(
        a.app.join_account(b"x", PW.into()).unwrap_err(),
        AppError::AlreadyInitialized
    );
}

#[test]
fn bad_ids_unknown_addresses_and_locked_vaults_are_reported() {
    let mut a = with_account();
    assert_eq!(
        a.app.remove_device("nope").unwrap_err().code(),
        "validation"
    );
    assert_eq!(
        a.app
            .device_address(&DeviceId::new().to_string())
            .unwrap_err(),
        AppError::NotFound("device")
    );
    a.app.lock();
    assert!(!a.app.sync_listener_wanted());
    assert_eq!(a.app.devices().unwrap_err(), AppError::Locked);
    assert_eq!(a.app.this_device().unwrap_err(), AppError::Locked);
}

#[test]
fn a_device_without_an_address_cannot_be_synced_to() {
    let (mut a, _b) = paired();
    let id = a.app.devices().unwrap()[0].id.clone();
    let device = id.parse().unwrap();
    a.app
        .session()
        .unwrap()
        .set_peer_addr(device, None)
        .unwrap();
    assert_eq!(a.app.device_address(&id).unwrap_err().code(), "validation");
}

#[test]
fn bad_vault_ids_are_refused_before_sending() {
    let mut a = with_account();
    let host = a.app.start_pairing(LOCALHOST).unwrap();
    let code = host.invite_code();
    let joiner = thread::spawn(move || Joining::connect(&code).unwrap());
    let mut pairing = host.accept().unwrap();
    let _joining = joiner.join().unwrap();
    let err = a
        .app
        .send_pairing_bundle(&mut pairing, &["nope".into()])
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn out_of_sync_means_missing_changes_for_a_week_or_very_many() {
    let day = 24 * 60 * 60 * 1000;
    let t = |ms: i64| Timestamp::from_millis(ms);
    assert!(
        !is_out_of_sync(0, t(0), t(100 * day)),
        "nothing missing: fine"
    );
    assert!(!is_out_of_sync(3, t(0), t(7 * day)), "exactly a week: fine");
    assert!(is_out_of_sync(3, t(0), t(7 * day + 1)));
    assert!(is_out_of_sync(OUT_OF_SYNC_CHANGES, t(0), t(0)));
    assert!(!is_out_of_sync(OUT_OF_SYNC_CHANGES - 1, t(0), t(0)));
}

#[test]
fn transport_errors_read_well() {
    let io = net_error(NetError::Io(std::io::Error::other("refused")));
    assert_eq!(
        io,
        AppError::Io("Could not reach the other device: refused".into())
    );
    assert_eq!(net_error(NetError::Cancelled).code(), "validation");
    assert_eq!(
        net_error(NetError::Vault(
            password365_vault::VaultError::WrongCredentials
        )),
        AppError::WrongCredentials
    );
}

/// A bundle for A's account, as the pairing channel would deliver it.
fn bundle_of(a: &mut crate::service::tests::Fixture) -> Vec<u8> {
    let key = a.keys.load().unwrap().unwrap();
    let personal = a
        .app
        .session()
        .unwrap()
        .vault_by_name("Personal")
        .unwrap()
        .id;
    a.app
        .session()
        .unwrap()
        .join_bundle(&key, &[personal])
        .unwrap()
        .to_vec()
}

#[test]
fn joining_again_after_an_interrupted_attempt_reuses_the_left_over_key() {
    let mut a = with_account();
    let bundle = bundle_of(&mut a);
    let mut b = fresh();
    b.keys.save(&a.keys.load().unwrap().unwrap()).unwrap();
    b.app.join_account(&bundle, PW.into()).unwrap();
    assert!(b.app.status().unlocked);
}

#[test]
fn a_different_accounts_key_is_kept_and_nothing_is_created() {
    let mut a = with_account();
    let bundle = bundle_of(&mut a);
    let other = with_account();
    let mut b = fresh();
    let foreign = other.keys.load().unwrap().unwrap();
    b.keys.save(&foreign).unwrap();
    assert_eq!(
        b.app.join_account(&bundle, PW.into()).unwrap_err().code(),
        "validation"
    );
    assert!(!b.app.status().initialized, "no store is left behind");
    assert_eq!(b.keys.load().unwrap().unwrap().expose(), foreign.expose());
}

#[test]
fn sync_watch_reports_local_changes_only_while_paired_and_unlocked() {
    let fx = with_account();
    assert_eq!(fx.app.sync_watch(), None, "nobody to sync with");
    let (a, mut b) = paired();
    assert!(a.app.sync_watch().is_some());
    let before = b.app.sync_watch().unwrap();
    b.app.create_item(&login("New", "pw")).unwrap();
    assert!(b.app.sync_watch().unwrap() > before);
    b.app.lock();
    assert_eq!(b.app.sync_watch(), None);
}

#[test]
fn a_master_password_changed_on_one_device_is_adopted_on_the_other() {
    let (mut a, mut b) = paired();
    a.app
        .change_master_password(PW.into(), NEW_PW.into(), &NEW_PW.into())
        .unwrap();
    assert_eq!(a.app.pending_password_change().unwrap(), None);
    sync(a, &mut b);
    let pending = b.app.pending_password_change().unwrap().unwrap();
    assert_eq!(pending.device_name, "Laptop");
    assert_eq!(
        b.app.adopt_master_password(PW.into()).unwrap_err(),
        AppError::WrongCredentials
    );
    b.app.adopt_master_password(NEW_PW.into()).unwrap();
    assert_eq!(b.app.pending_password_change().unwrap(), None);
    b.app.lock();
    assert_eq!(
        b.app.unlock(PW.into(), None).unwrap_err(),
        AppError::WrongCredentials
    );
    b.app.unlock(NEW_PW.into(), None).unwrap();
}

const NEW_PW: &str = "a brand new master password";

#[test]
fn archiving_reaches_the_other_device() {
    let (mut a, mut b) = paired();
    let id = a.app.items("", None).unwrap()[0].id.clone();
    a.app.set_archived(&id, true).unwrap();
    let (_a, _, _) = sync(a, &mut b);
    assert!(b.app.items("", None).unwrap().is_empty());
    assert_eq!(b.app.archived_items("").unwrap()[0].id, id);
}

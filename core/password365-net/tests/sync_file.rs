//! `.p365sync` files between two paired stores.

use password365_net::NetError;
use password365_net::sync_file::{MAGIC, export, import};
use password365_vault::VaultError;

mod common;
use common::{Pair, add, paired, titles};

#[test]
fn a_file_carries_changes_to_its_recipient_only_once() {
    let Pair { _dir, mut a, mut b } = paired();
    add(&mut a, "Mail");
    let file = export(&a, b.device_id()).expect("export");
    assert!(file.starts_with(MAGIC));
    assert!(
        !file.windows(4).any(|w| w == b"Mail"),
        "the content is encrypted"
    );

    let outcome = import(&mut b, &file).expect("import");
    assert_eq!(
        (outcome.peer, outcome.report.changed.len()),
        (a.device_id(), 1)
    );
    assert_eq!(titles(&b), vec!["Mail"]);
    assert_eq!(
        import(&mut b, &file).expect("again").report.changed.len(),
        0,
        "idempotent"
    );
    assert_eq!(
        b.peer(a.device_id()).expect("peer").received_seq,
        Some(a.sync_seq())
    );
}

#[test]
fn a_reply_file_tells_the_sender_what_arrived() {
    let Pair { _dir, mut a, mut b } = paired();
    add(&mut a, "Mail");
    let to_b = export(&a, b.device_id()).expect("export");
    import(&mut b, &to_b).expect("import");
    assert!(a.changes_pending_for(b.device_id()).expect("peer") > 0);
    add(&mut b, "From B");
    let reply = export(&b, a.device_id()).expect("reply");
    import(&mut a, &reply).expect("import reply");
    assert_eq!(titles(&a), vec!["From B", "Mail"]);
    assert_eq!(
        a.changes_pending_for(b.device_id()).expect("peer"),
        1,
        "only the merge result is left to send"
    );
}

#[test]
fn files_for_other_devices_damaged_or_foreign_are_refused() {
    let Pair { _dir, mut a, b } = paired();
    let Pair {
        a: mut stranger,
        _dir: _d2,
        ..
    } = paired();
    let file = export(&b, a.device_id()).expect("export");

    assert!(
        matches!(import(&mut stranger, &file), Err(NetError::Channel(_))),
        "not addressed to it"
    );
    assert!(matches!(
        import(&mut a, b"hello"),
        Err(NetError::Vault(VaultError::NotASyncFile))
    ));
    let mut truncated = file.clone();
    truncated.truncate(file.len() - 3);
    assert!(
        import(&mut a, &truncated).is_err(),
        "nothing applied from a cut file"
    );
    let mut tampered = file.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(import(&mut a, &tampered).is_err());
    assert!(titles(&a).is_empty());
    assert!(matches!(
        export(&a, password365_sync::DeviceId::new()),
        Err(NetError::Vault(VaultError::NotFound(_)))
    ));
}

#[test]
fn a_file_from_an_unpaired_sender_is_refused() {
    let Pair { _dir, mut a, b } = paired();
    let file = export(&b, a.device_id()).expect("export");
    a.remove_peer(b.device_id()).expect("remove");
    assert!(matches!(
        import(&mut a, &file),
        Err(NetError::Vault(VaultError::UnknownDevice))
    ));
}

#[test]
fn a_file_carries_removed_devices_and_a_removed_sender_is_refused() {
    let Pair { _dir, mut a, mut b } = paired();
    let lost = password365_sync::DeviceId::new();
    a.revoke_device(lost).expect("revoke");
    let file = export(&a, b.device_id()).expect("export");
    import(&mut b, &file).expect("import");
    assert!(b.is_revoked(lost), "B learnt about the removed device");

    let Pair {
        _dir: _d2,
        a,
        mut b,
    } = paired();
    let a_id = a.device_id();
    b.revoke_device(a_id).expect("B removes A");
    assert!(b.peer(a_id).is_err(), "B forgot A when removing it");
    let from_a = export(&a, b.device_id()).expect("export");
    assert!(matches!(
        import(&mut b, &from_a).unwrap_err(),
        NetError::Vault(VaultError::UnknownDevice)
    ));
}

#[test]
fn a_file_carries_a_master_password_change() {
    let Pair { _dir, mut a, mut b } = paired();
    a.change_master_password(
        &password365_vault::Credentials {
            password: "a new master password".into(),
            secret_key: password365_crypto::secret_key::AccountSecretKey::generate()
                .expect("random"),
        },
        password365_crypto::kdf::KdfParams::insecure_for_tests(),
    )
    .expect("change");
    let file = export(&a, b.device_id()).expect("export");
    assert!(
        import(&mut b, &file)
            .expect("import")
            .report
            .password_changed
    );
    assert!(b.pending_password_change().is_some());
}

#[test]
fn a_file_carries_attachments_too() {
    let Pair { _dir, mut a, mut b } = paired();
    let id = add(&mut a, "Passport");
    let bytes: Vec<u8> = (0..150_000u32).map(|i| (i % 253) as u8).collect();
    let file = a.attach(id, "scan.bin", "", &bytes).expect("attach");
    let sync_file = export(&a, b.device_id()).expect("export");
    let outcome = import(&mut b, &sync_file).expect("import");
    assert_eq!(outcome.report.files, 1);
    assert_eq!(
        b.attachment_bytes(id, file.id).expect("bytes").as_slice(),
        bytes.as_slice()
    );
}

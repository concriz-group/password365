use std::sync::Mutex;

use password365_app::item_input::ItemInput;

use super::*;

/// A Keychain in memory.
#[derive(Default)]
pub(crate) struct MemoryKeeper(Mutex<Option<String>>);

impl KeyKeeper for MemoryKeeper {
    fn load(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
    fn save(&self, key: String) -> bool {
        *self.0.lock().unwrap() = Some(key);
        true
    }
    fn remove(&self) {
        *self.0.lock().unwrap() = None;
    }
}

pub(crate) const PASSWORD: &str = "correct horse battery";

pub(crate) fn vault(dir: &tempfile::TempDir, keeper: Arc<MemoryKeeper>) -> Arc<Password365> {
    Password365::with_kdf(
        dir.path().to_str().unwrap(),
        keeper,
        KdfParams::insecure_for_tests(),
    )
}

#[test]
fn creates_unlocks_lists_and_reveals_one_secret_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let keeper = Arc::new(MemoryKeeper::default());
    let p = vault(&dir, keeper.clone());
    assert_eq!(
        p.status(),
        Status {
            initialized: false,
            unlocked: false
        }
    );
    let secret_key = p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    assert_eq!(
        keeper.load().as_deref(),
        Some(secret_key.as_str()),
        "kept by the app's Keychain"
    );
    assert_eq!(
        p.status(),
        Status {
            initialized: true,
            unlocked: true
        }
    );

    let created = p
        .app()
        .create_item(&ItemInput {
            title: "GitHub".into(),
            username: Some("octo".into()),
            urls: vec!["https://github.com".into()],
            password: Some("pw".into()),
            totp: Some("JBSWY3DPEHPK3PXP".into()),
            notes: Some("note".into()),
            ..ItemInput::default()
        })
        .unwrap();
    let id = created.summary.id;
    let rows = p.items(String::new()).unwrap();
    assert_eq!(
        rows,
        [ItemRow {
            id: id.clone(),
            title: "GitHub".into(),
            subtitle: "octo".into(),
            kind: "login".into(),
            favorite: false,
            tags: Vec::new(),
        }]
    );
    assert!(p.items("nothing".into()).unwrap().is_empty());

    let view = p.item(id.clone()).unwrap();
    assert!(view.has_password && view.has_notes && view.has_totp);
    assert_eq!(
        (view.urls.clone(), view.passkey_site.clone()),
        (vec!["https://github.com".to_owned()], None)
    );
    assert_eq!(
        p.reveal_password(id.clone()).unwrap().as_deref(),
        Some("pw")
    );
    assert_eq!(p.reveal_notes(id.clone()).unwrap().as_deref(), Some("note"));
    let code = p.totp(id.clone()).unwrap().unwrap();
    assert_eq!((code.code.len(), code.period), (6, 30));

    p.lock();
    assert_eq!(
        p.status(),
        Status {
            initialized: true,
            unlocked: false
        }
    );
    assert!(
        matches!(p.items(String::new()), Err(Failure::Failed { code, .. }) if code == "locked")
    );
    assert!(
        matches!(p.unlock("wrong".into(), None), Err(Failure::Failed { code, .. }) if code == "wrong_credentials")
    );
    p.unlock(PASSWORD.into(), None).unwrap();
    assert_eq!(p.items(String::new()).unwrap().len(), 1);
    assert!(p.reveal_field(id, "missing".into()).unwrap().is_none());
}

#[test]
fn creates_logins_leaving_empty_values_out() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    let id = p
        .create_login("Mail".into(), "ana".into(), "pw".into(), " ".into())
        .unwrap();
    let view = p.item(id.clone()).unwrap();
    assert_eq!(
        (view.username.as_deref(), view.urls.len(), view.has_password),
        (Some("ana"), 0, true)
    );
    assert_eq!(p.reveal_password(id).unwrap().as_deref(), Some("pw"));
    assert!(
        p.create_login(String::new(), String::new(), String::new(), String::new())
            .is_err(),
        "a title is required"
    );
}

#[test]
fn a_device_without_the_secret_key_asks_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let keeper = Arc::new(MemoryKeeper::default());
    let secret_key = vault(&dir, keeper.clone())
        .create_account(PASSWORD.into(), PASSWORD.into())
        .unwrap();
    keeper.remove();
    let p = vault(&dir, keeper.clone());
    let failure = p.unlock(PASSWORD.into(), None).unwrap_err();
    assert!(
        matches!(&failure, Failure::Failed { code, .. } if code == "secret_key_required"),
        "{failure}"
    );
    p.unlock(PASSWORD.into(), Some(secret_key.clone())).unwrap();
    assert_eq!(keeper.load(), Some(secret_key), "remembered again");
}

#[test]
fn creating_twice_or_with_a_weak_password_fails() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    assert!(p.create_account("short".into(), "short".into()).is_err());
    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    assert!(
        matches!(p.create_account(PASSWORD.into(), PASSWORD.into()), Err(Failure::Failed { code, .. }) if code == "already_initialized")
    );
}

#[test]
fn editing_keeps_one_time_password_fields_that_are_not_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    let mut draft = p.new_draft("login".into()).unwrap();
    draft.title = "Bank".into();
    draft.fields.push(crate::editing::FieldDraft {
        label: "backup code".into(),
        value: "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP".into(),
        section: "More".into(),
        input: "totp".into(),
        concealed: false,
    });
    let id = p.create_item(draft).unwrap();
    let mut edit = p.edit_draft(id.clone()).unwrap();
    edit.title = "Bank 2".into();
    p.update_item(id.clone(), edit).unwrap();
    assert_eq!(
        p.reveal_field(id, "backup code".into()).unwrap().as_deref(),
        Some("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP")
    );
}

#[test]
fn drafts_create_edit_and_keep_secrets_until_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();

    let cards = p
        .categories()
        .into_iter()
        .find(|c| c.kind == "card")
        .unwrap();
    assert_eq!(cards.label, "Credit Card");
    let mut draft = p.new_draft("card".into()).unwrap();
    assert!(
        draft
            .fields
            .iter()
            .any(|f| f.label == "number" && f.concealed)
    );
    draft.title = "Visa".into();
    for f in &mut draft.fields {
        match f.label.as_str() {
            "number" => f.value = "4111111111111111".into(),
            "expiry date" => f.value = "2029-04".into(),
            _ => {}
        }
    }
    draft.tags = vec![" bank ".into(), String::new()];
    draft.notes = "pin in the safe".into();
    let id = p.create_item(draft).unwrap();

    let mut edit = p.edit_draft(id.clone()).unwrap();
    assert_eq!(edit.tags, ["bank"]);
    assert_eq!(edit.notes, "pin in the safe");
    assert_eq!(
        edit.fields
            .iter()
            .find(|f| f.label == "number")
            .unwrap()
            .value,
        "4111111111111111",
        "concealed values in clear for the editor"
    );
    edit.title = "Visa Gold".into();
    edit.notes = String::new();
    let stale = edit.clone();
    p.update_item(id.clone(), edit).unwrap();
    let view = p.item(id.clone()).unwrap();
    assert_eq!(
        (view.title.as_str(), view.has_notes),
        ("Visa Gold", false),
        "an emptied note is cleared"
    );
    assert_eq!(
        p.reveal_field(id.clone(), "number".into())
            .unwrap()
            .as_deref(),
        Some("4111111111111111")
    );
    assert!(
        matches!(p.update_item(id.clone(), stale), Err(Failure::Failed { code, .. }) if code == "conflict")
    );

    p.set_favorite(id.clone(), true).unwrap();
    assert!(p.items(String::new()).unwrap()[0].favorite);
    assert_eq!(p.tags().unwrap(), ["bank"]);
    assert_eq!(
        p.items(String::new()).unwrap()[0].tags,
        ["bank"],
        "rows carry their tags (list filter)"
    );
    p.set_archived(id.clone(), true).unwrap();
    assert!(p.items(String::new()).unwrap().is_empty());
    assert_eq!(p.archived(String::new()).unwrap().len(), 1);
    assert!(p.item(id.clone()).unwrap().archived);
    p.set_archived(id.clone(), false).unwrap();
    p.delete_item(id).unwrap();
    assert!(p.items(String::new()).unwrap().is_empty());

    let bad = ItemDraft {
        fields: vec![FieldDraft {
            label: "x".into(),
            value: "y".into(),
            section: String::new(),
            input: "nope".into(),
            concealed: false,
        }],
        ..p.new_draft("login".into()).unwrap()
    };
    assert!(p.create_item(bad).is_err());
    assert!(p.template("not-a-kind".into(), String::new()).is_err());
    assert!(
        !p.template("identity".into(), "BR".into())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn generates_passwords_passphrases_and_pins() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    let pw = p
        .generate(Recipe::Password {
            length: 24,
            digits: true,
            symbols: false,
        })
        .unwrap();
    assert_eq!(pw.value.chars().count(), 24);
    assert!(pw.value.chars().all(char::is_alphanumeric));
    // A few list words contain '-' ("t-shirt"): at least 4 parts.
    let phrase = p.generate(Recipe::Passphrase { words: 4 }).unwrap().value;
    assert!(phrase.split('-').count() >= 4, "{phrase}");
    let pin = p.generate(Recipe::Pin { digits: 6 }).unwrap();
    assert!(pin.value.len() == 6 && pin.value.chars().all(|c| c.is_ascii_digit()));
    assert!(["weak", "fair", "good", "strong", "excellent"].contains(&pw.strength.as_str()));
}

mod joining {
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::thread;

    use password365_app::secret_key_store::MemorySecretKeyStore;
    use password365_net::tcp;

    use super::*;

    const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    /// A desktop with an account and one item.
    fn mac(dir: &tempfile::TempDir) -> AppService {
        let config = AppConfig {
            store_path: dir.path().join("mac"),
            kdf: KdfParams::insecure_for_tests(),
            auto_lock_ms: 0,
            settings_path: None,
        };
        let mut mac = AppService::new(config, Box::new(MemorySecretKeyStore::default()));
        mac.create_account(PASSWORD.into(), &PASSWORD.into())
            .unwrap();
        mac.rename_this_device("Mac").unwrap();
        mac.create_item(&ItemInput {
            title: "From the Mac".into(),
            password: Some("pw".into()),
            ..ItemInput::default()
        })
        .unwrap();
        mac
    }

    #[test]
    fn a_phone_joins_the_macs_account_and_both_sync() {
        let dir = tempfile::tempdir().unwrap();
        let mut mac = mac(&dir);
        let personal = mac.vaults().unwrap()[0].id.clone();
        let host = mac.start_pairing(LOCALHOST).unwrap();
        let code = host.invite_code();
        let inviter = thread::spawn(move || {
            let mut pairing = host.accept().unwrap();
            mac.send_pairing_bundle(&mut pairing, &[personal]).unwrap();
            let peer = pairing.wait_joined(tcp::SYNC_PORT).unwrap();
            let phone = mac.pin_device(peer).unwrap();
            (mac, phone)
        });

        let keeper = Arc::new(MemoryKeeper::default());
        let phone = vault(&dir, keeper.clone());
        assert!(
            phone.join_finish(PASSWORD.into(), "iPhone".into()).is_err(),
            "nothing to finish yet"
        );
        let digits = phone.join_connect(code).unwrap();
        assert_eq!(
            digits.chars().filter(char::is_ascii_digit).count(),
            6,
            "{digits}"
        );
        phone.join_wait().unwrap();
        phone.join_wait().unwrap();
        assert!(
            matches!(phone.join_finish("wrong".into(), "iPhone".into()), Err(Failure::Failed { code, .. }) if code == "wrong_credentials")
        );
        assert!(
            !phone.status().initialized,
            "a wrong password writes nothing"
        );
        phone.join_finish(PASSWORD.into(), "iPhone".into()).unwrap();
        assert!(
            keeper.load().is_some(),
            "the account's Secret Key is in the Keychain"
        );
        let (mac, phone_on_mac) = inviter.join().unwrap();
        assert_eq!(phone_on_mac.name, "iPhone");
        let devices = phone.devices().unwrap();
        assert_eq!(
            devices.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            ["Mac"]
        );
        assert_eq!(phone.device_name().unwrap(), "iPhone");

        // The Mac listens (on a free port here); the phone syncs.
        let (addr, server) = serve_once(mac);
        // The address remembered at pairing leads nowhere here: found by discovery.
        let mac_id = devices[0].id.clone();
        phone
            .app()
            .set_device_address(&mac_id, &dead().to_string())
            .unwrap();
        let summary = phone.sync_device(&mac_id, || vec![addr]).unwrap();
        assert_eq!(summary.changed, 1);
        let mac = server.join().unwrap();
        let row = phone.items(String::new()).unwrap().remove(0);
        assert_eq!(row.title, "From the Mac");
        assert_eq!(
            phone.reveal_password(row.id).unwrap().as_deref(),
            Some("pw")
        );

        // And back: a login made on the phone reaches the Mac.
        phone
            .create_login(
                "From the phone".into(),
                String::new(),
                "x".into(),
                String::new(),
            )
            .unwrap();
        let (addr, server) = serve_once(mac);
        phone
            .app()
            .set_device_address(&mac_id, &dead().to_string())
            .unwrap();
        let found = Arc::new(Found::new(vec![addr.to_string(), "not an address".into()]));
        let summary = phone.sync_all(Some(found.clone())).unwrap();
        assert_eq!((summary.changed, summary.unreachable), (0, 0));
        assert_eq!(found.calls(), 1, "looked for once");
        let mut mac = server.join().unwrap();
        assert_eq!(mac.items("From the phone", None).unwrap().len(), 1);

        // Nothing listens at the remembered address: counted, not an error.
        let _ = &mut mac;
        phone
            .app()
            .set_device_address(&mac_id, &dead().to_string())
            .unwrap();
        let unreachable = phone.sync_all(None).unwrap();
        assert_eq!((unreachable.changed, unreachable.unreachable), (0, 1));
    }

    #[test]
    fn a_cancelled_or_bad_code_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let phone = vault(&dir, Arc::new(MemoryKeeper::default()));
        assert!(phone.join_connect("not a code".into()).is_err());
        assert!(phone.join_wait().is_err());
        phone.join_cancel();
        assert!(!phone.status().initialized);
    }
    /// A Mac and a phone that joined its account (the phone's id on the Mac).
    fn paired(dir: &tempfile::TempDir) -> (AppService, Arc<Password365>, String) {
        let mut mac = mac(dir);
        let personal = mac.vaults().unwrap()[0].id.clone();
        let host = mac.start_pairing(LOCALHOST).unwrap();
        let code = host.invite_code();
        let inviter = thread::spawn(move || {
            let mut pairing = host.accept().unwrap();
            mac.send_pairing_bundle(&mut pairing, &[personal]).unwrap();
            let peer = pairing.wait_joined(tcp::SYNC_PORT).unwrap();
            let phone = mac.pin_device(peer).unwrap();
            (mac, phone)
        });
        let phone = vault(dir, Arc::new(MemoryKeeper::default()));
        phone.join_connect(code).unwrap();
        phone.join_wait().unwrap();
        phone.join_wait().unwrap();
        phone.join_finish(PASSWORD.into(), "iPhone".into()).unwrap();
        let (mac, phone_on_mac) = inviter.join().unwrap();
        (mac, phone, phone_on_mac.id)
    }

    #[test]
    fn sync_files_carry_changes_both_ways() {
        let dir = tempfile::tempdir().unwrap();
        let (mut mac, phone, phone_id) = paired(&dir);
        let to_phone = dir.path().join("for the phone.p365sync");
        mac.export_sync_file(&phone_id, &to_phone).unwrap();
        let got = phone
            .import_sync_file(to_phone.display().to_string())
            .unwrap();
        assert_eq!((got.device.as_str(), got.changed), ("Mac", 1));
        assert_eq!(phone.items(String::new()).unwrap()[0].title, "From the Mac");

        phone
            .create_login(
                "From the phone".into(),
                String::new(),
                "x".into(),
                String::new(),
            )
            .unwrap();
        let mac_id = phone.devices().unwrap()[0].id.clone();
        let to_mac = dir.path().join("for the Mac.p365sync");
        let carried = phone
            .export_sync_file(mac_id, to_mac.display().to_string())
            .unwrap();
        assert!(carried >= 1);
        mac.import_sync_file(&to_mac).unwrap();
        assert_eq!(mac.items("From the phone", None).unwrap().len(), 1);

        let junk = dir.path().join("junk.p365sync");
        std::fs::write(&junk, b"not a sync file").unwrap();
        assert!(phone.import_sync_file(junk.display().to_string()).is_err());
        assert!(
            phone
                .export_sync_file(
                    "not a device".into(),
                    dir.path().join("x").display().to_string()
                )
                .is_err()
        );
    }

    /// The Mac listens on a free port and answers one sync.
    fn serve_once(mut mac: AppService) -> (std::net::SocketAddr, thread::JoinHandle<AppService>) {
        let listener = TcpListener::bind((LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let key = mac.network_key().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = tcp::accept(&listener, &key).unwrap();
            mac.sync_over(&mut stream, None).unwrap();
            mac
        });
        (addr, server)
    }

    /// A local address nothing listens on.
    fn dead() -> std::net::SocketAddr {
        TcpListener::bind((LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
    }

    /// Discovery that returns fixed addresses and counts how often it was asked.
    struct Found {
        addrs: Vec<String>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl Found {
        fn new(addrs: Vec<String>) -> Self {
            Self {
                addrs,
                calls: 0.into(),
            }
        }
        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl crate::pairing::Discovery for Found {
        fn browse(&self) -> Vec<String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.addrs.clone()
        }
    }
}

#[test]
fn face_id_unlock_and_phone_settings() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    assert_eq!(
        p.mobile_settings().auto_lock_minutes,
        1,
        "a phone locks after a minute by default"
    );
    p.set_mobile_settings(MobileSettings {
        auto_lock_minutes: 5,
        clipboard_clear_seconds: 30,
        region: "BR".into(),
    })
    .unwrap();
    let again = vault(&dir, Arc::new(MemoryKeeper::default()));
    assert_eq!(
        again.mobile_settings(),
        MobileSettings {
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 30,
            region: "BR".into()
        }
    );

    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    assert!(!p.quick_unlock_ready());
    let key = p.enable_quick_unlock().unwrap();
    assert_eq!(key.len(), 32);
    p.lock();
    p.unlock_quick(key.clone()).unwrap();
    assert!(p.status().unlocked);
    p.lock();
    assert!(
        matches!(p.unlock_quick(vec![0; 32]), Err(Failure::Failed { code, .. }) if code == "password_required")
    );
    assert!(!p.quick_unlock_ready());
    p.unlock(PASSWORD.into(), None).unwrap();
    p.enable_quick_unlock().unwrap();
    p.disable_quick_unlock();
    assert!(!p.quick_unlock_ready());
}

#[test]
fn autofill_lists_and_fills_logins_codes_and_passkeys() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    let id = p
        .create_login(
            "GitHub".into(),
            "octo".into(),
            "pw".into(),
            "https://github.com".into(),
        )
        .unwrap();
    let mut draft = p.edit_draft(id.clone()).unwrap();
    draft.totp = "JBSWY3DPEHPK3PXP".into();
    p.update_item(id.clone(), draft).unwrap();

    let logins = p.login_identities().unwrap();
    assert_eq!(
        logins,
        [LoginIdentity {
            item_id: id.clone(),
            username: "octo".into(),
            urls: vec!["https://github.com".into()]
        }]
    );
    assert_eq!(p.logins_for_service("github.com".into()).unwrap()[0].id, id);
    assert!(
        p.logins_for_service("https://evil.com/login".into())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        p.fill(id.clone(), "gist.github.com".into()).unwrap(),
        FillValues {
            username: "octo".into(),
            password: "pw".into()
        }
    );
    assert!(
        p.fill(id.clone(), "evil.com".into()).is_err(),
        "never filled into another site"
    );
    assert_eq!(
        p.fill(id.clone(), String::new()).unwrap().password,
        "pw",
        "picked from the whole list"
    );
    assert_eq!(p.one_time_code(id).unwrap().map(|c| c.len()), Some(6));

    let created = p
        .passkey_create("example.com".into(), b"u".to_vec(), "ana".into(), vec![-7])
        .unwrap();
    let identities = p.passkey_identities().unwrap();
    assert_eq!(
        (
            identities[0].rp_id.as_str(),
            identities[0].credential_id.clone()
        ),
        ("example.com", created.credential_id.clone())
    );
    let signed = p
        .passkey_sign_in("example.com".into(), created.credential_id, vec![1; 32])
        .unwrap();
    assert_eq!(signed.user_handle, b"u");
}

#[test]
fn the_vault_locks_after_the_setting_unless_the_app_says_the_user_is_there() {
    use password365_model::Timestamp;
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    p.create_account(PASSWORD.into(), PASSWORD.into()).unwrap();
    let start = Timestamp::now().as_millis();
    let at = |seconds: i64| Timestamp::from_millis(start + seconds * 1000);
    // In use: every report starts the minute again; other calls do not.
    assert!(p.status_at(at(50)).unlocked);
    p.user_active_at(at(50));
    p.items(String::new()).unwrap();
    assert!(p.status_at(at(100)).unlocked, "50 s after the last report");
    // Away for more than the minute: asking the status locks it…
    assert!(!p.status_at(at(111)).unlocked);
    assert!(!p.status().unlocked);
    // …and so does coming back late, instead of starting the time again.
    p.unlock(PASSWORD.into(), None).unwrap();
    let later = Timestamp::now().as_millis() + 10 * 60 * 1000;
    p.user_active_at(Timestamp::from_millis(later));
    assert!(!p.status().unlocked);
}

#[test]
fn locking_as_soon_as_the_app_leaves_is_a_setting_that_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let p = vault(&dir, Arc::new(MemoryKeeper::default()));
    let at_once = MobileSettings {
        auto_lock_minutes: 0,
        clipboard_clear_seconds: 90,
        region: String::new(),
    };
    p.set_mobile_settings(at_once.clone()).unwrap();
    assert_eq!(p.mobile_settings(), at_once);
    // Still chosen after the app starts again.
    let again = vault(&dir, Arc::new(MemoryKeeper::default()));
    assert_eq!(again.mobile_settings().auto_lock_minutes, 0);
    // Back to minutes.
    again
        .set_mobile_settings(MobileSettings {
            auto_lock_minutes: 5,
            ..at_once
        })
        .unwrap();
    assert_eq!(again.mobile_settings().auto_lock_minutes, 5);
}

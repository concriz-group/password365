//! Tests for `AppService` (kept in a separate file to keep `service.rs` focused).

use password365_crypto::kdf::KdfParams;
use password365_generator::{PassphraseRecipe, PasswordRecipe};

use super::*;
use crate::secret_key_store::MemorySecretKeyStore;
use crate::settings::Settings;

pub(crate) const PW: &str = "correct horse battery";

pub(crate) struct Fixture {
    _dir: tempfile::TempDir,
    pub(crate) keys: MemorySecretKeyStore,
    pub(crate) app: AppService,
}

fn config(dir: &tempfile::TempDir) -> AppConfig {
    AppConfig {
        store_path: dir.path().join("store.qlvault"),
        kdf: KdfParams::insecure_for_tests(),
        auto_lock_ms: 0,
        settings_path: None,
    }
}

pub(crate) fn fresh() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let keys = MemorySecretKeyStore::default();
    let app = AppService::new(config(&dir), Box::new(keys.clone()));
    Fixture {
        _dir: dir,
        keys,
        app,
    }
}

pub(crate) fn with_account() -> Fixture {
    let mut fx = fresh();
    fx.app.create_account(PW.into(), &PW.into()).unwrap();
    fx
}

pub(crate) fn login(title: &str, password: &str) -> ItemInput {
    ItemInput {
        title: title.into(),
        username: Some("me".into()),
        urls: vec![format!("https://{}.com", title.to_lowercase())],
        password: Some(password.into()),
        ..ItemInput::default()
    }
}

#[test]
fn desktop_config_defaults() {
    let c = AppConfig::desktop("/x".into());
    assert_eq!(
        (c.kdf, c.auto_lock_ms),
        (KdfParams::DESKTOP, DEFAULT_TIMEOUT_MS)
    );
}

#[test]
fn account_lifecycle() {
    let mut fx = fresh();
    assert_eq!(
        fx.app.status(),
        AppStatus {
            initialized: false,
            unlocked: false
        }
    );
    assert_eq!(
        fx.app.unlock(PW.into(), None).unwrap_err(),
        AppError::SecretKeyRequired
    );

    let secret_key = fx.app.create_account(PW.into(), &PW.into()).unwrap();
    assert!(secret_key.starts_with("P365-"));
    assert_eq!(
        fx.app.status(),
        AppStatus {
            initialized: true,
            unlocked: true
        }
    );
    assert_eq!(
        fx.app.create_account(PW.into(), &PW.into()).unwrap_err(),
        AppError::AlreadyInitialized
    );

    fx.app.lock();
    assert_eq!(fx.app.vaults().unwrap_err(), AppError::Locked);
    assert_eq!(
        fx.app.unlock("wrong password".into(), None).unwrap_err(),
        AppError::WrongCredentials
    );
    fx.app.unlock(PW.into(), None).unwrap();
    assert!(fx.app.status().unlocked);
}

#[test]
fn create_account_validates_password() {
    let mut fx = fresh();
    assert_eq!(
        fx.app
            .create_account("short".into(), &"short".into())
            .unwrap_err()
            .code(),
        "validation"
    );
    assert!(fx.keys.load().unwrap().is_none());
}

#[test]
fn failed_store_creation_rolls_back_secret_key() {
    let keys = MemorySecretKeyStore::default();
    // A path below a regular file cannot be created on any system.
    let file = tempfile::NamedTempFile::new().unwrap();
    let cfg = AppConfig {
        store_path: file.path().join("x").join("store"),
        kdf: KdfParams::insecure_for_tests(),
        auto_lock_ms: 0,
        settings_path: None,
    };
    let mut app = AppService::new(cfg, Box::new(keys.clone()));
    assert!(app.create_account(PW.into(), &PW.into()).is_err());
    assert!(keys.load().unwrap().is_none());
}

#[test]
fn unlock_on_new_device_with_typed_secret_key_remembers_it() {
    let mut fx = with_account();
    let key = fx.keys.load().unwrap().unwrap().to_display_string();
    fx.keys.remove().unwrap();
    fx.app.lock();
    assert_eq!(
        fx.app
            .unlock(PW.into(), Some("P365-garbage"))
            .unwrap_err()
            .code(),
        "validation"
    );
    fx.app.unlock(PW.into(), Some(&key)).unwrap();
    assert!(fx.keys.load().unwrap().is_some());
}

#[test]
fn auto_lock_after_idle() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(&dir);
    cfg.auto_lock_ms = 1_000;
    let mut app = AppService::new(cfg, Box::new(MemorySecretKeyStore::default()));
    app.create_account(PW.into(), &PW.into()).unwrap();
    let now = Timestamp::now();
    assert!(!app.check_auto_lock(now));
    assert!(app.check_auto_lock(Timestamp::from_millis(now.as_millis() + 5_000)));
    assert!(!app.status().unlocked);
    assert!(
        !app.check_auto_lock(Timestamp::from_millis(now.as_millis() + 9_000)),
        "already locked"
    );
}

#[test]
fn change_master_password() {
    let mut fx = with_account();
    let new = "a completely new password";
    assert_eq!(
        fx.app
            .change_master_password("wrong one!!".into(), new.into(), &new.into())
            .unwrap_err(),
        AppError::WrongCredentials
    );
    assert_eq!(
        fx.app
            .change_master_password(PW.into(), "short".into(), &"short".into())
            .unwrap_err()
            .code(),
        "validation"
    );
    fx.app
        .change_master_password(PW.into(), new.into(), &new.into())
        .unwrap();
    fx.app.lock();
    assert!(fx.app.unlock(PW.into(), None).is_err());
    fx.app.unlock(new.into(), None).unwrap();
    fx.app.lock();
    assert_eq!(
        fx.app
            .change_master_password(new.into(), PW.into(), &PW.into())
            .unwrap_err(),
        AppError::Locked
    );
}

#[test]
fn vault_operations() {
    let mut fx = with_account();
    let work = fx.app.create_vault("Work").unwrap();
    assert_eq!(work.item_count, 0);
    fx.app.rename_vault(&work.id, "Job").unwrap();
    let names: Vec<String> = fx
        .app
        .vaults()
        .unwrap()
        .into_iter()
        .map(|v| v.name)
        .collect();
    assert_eq!(names, vec!["Job", "Personal"]);
    fx.app.delete_vault(&work.id).unwrap();
    assert_eq!(fx.app.vaults().unwrap().len(), 1);
    assert_eq!(
        fx.app.delete_vault("not-an-id").unwrap_err().code(),
        "validation"
    );
}

#[test]
fn item_crud_and_secrets_on_demand() {
    let mut fx = with_account();
    let created = fx.app.create_item(&login("GitHub", "s3cret")).unwrap();
    assert!(created.has_password);
    assert_eq!(
        fx.app
            .reveal(&created.summary.id, SecretField::Password)
            .unwrap()
            .as_deref(),
        Some("s3cret")
    );
    assert_eq!(
        fx.app
            .reveal(&created.summary.id, SecretField::Totp)
            .unwrap(),
        None
    );
    assert_eq!(
        fx.app
            .reveal(&created.summary.id, SecretField::Notes)
            .unwrap(),
        None
    );

    let mut edit = login("GitHub", "n3w");
    edit.notes = Some("hello".into());
    let updated = fx
        .app
        .update_item(&created.summary.id, created.version, &edit)
        .unwrap();
    assert_eq!(updated.password_history_count, 1);
    assert_eq!(
        fx.app
            .reveal(&created.summary.id, SecretField::Notes)
            .unwrap()
            .as_deref(),
        Some("hello")
    );

    assert_eq!(
        fx.app
            .update_item(&created.summary.id, created.version, &edit)
            .unwrap_err(),
        AppError::Conflict
    );

    fx.app.delete_item(&created.summary.id).unwrap();
    assert_eq!(
        fx.app.item(&created.summary.id).unwrap_err(),
        AppError::NotFound("item")
    );
}

#[test]
fn create_item_validation() {
    let mut fx = with_account();
    let bad_kind = ItemInput {
        kind: Some("spaceship".into()),
        ..login("X", "p")
    };
    assert_eq!(
        fx.app.create_item(&bad_kind).unwrap_err().code(),
        "validation"
    );
    let bad_vault = ItemInput {
        vault_id: Some(VaultId::new().to_string()),
        ..login("X", "p")
    };
    assert_eq!(
        fx.app.create_item(&bad_vault).unwrap_err(),
        AppError::NotFound("vault")
    );
    let note = ItemInput {
        kind: Some("note".into()),
        ..login("Memo", "")
    };
    assert_eq!(fx.app.create_item(&note).unwrap().summary.kind, "note");
}

#[test]
fn items_search_filter_and_move() {
    let mut fx = with_account();
    let work = fx.app.create_vault("Work").unwrap();
    let a = fx.app.create_item(&login("Alpha", "1")).unwrap();
    fx.app
        .create_item(&ItemInput {
            vault_id: Some(work.id.clone()),
            ..login("Beta", "2")
        })
        .unwrap();
    assert_eq!(fx.app.items("", None).unwrap().len(), 2);
    assert_eq!(fx.app.items("alp", None).unwrap()[0].title, "Alpha");
    assert_eq!(fx.app.items("", Some(&work.id)).unwrap()[0].title, "Beta");
    assert!(fx.app.items("", Some("bad")).is_err());

    let moved = ItemInput {
        vault_id: Some(work.id.clone()),
        ..login("Alpha", "1")
    };
    fx.app
        .update_item(&a.summary.id, a.version, &moved)
        .unwrap();
    assert_eq!(fx.app.items("", Some(&work.id)).unwrap().len(), 2);
}

#[test]
fn default_vault_falls_back_when_personal_is_gone() {
    let mut fx = with_account();
    let work = fx.app.create_vault("Work").unwrap();
    let personal = fx
        .app
        .vaults()
        .unwrap()
        .into_iter()
        .find(|v| v.name == "Personal")
        .unwrap();
    fx.app.delete_vault(&personal.id).unwrap();
    let item = fx.app.create_item(&login("X", "p")).unwrap();
    assert_eq!(item.summary.vault_id, work.id);
    fx.app.delete_item(&item.summary.id).unwrap();
    fx.app.delete_vault(&work.id).unwrap();
    assert_eq!(
        fx.app.create_item(&login("Y", "p")).unwrap_err(),
        AppError::NotFound("vault")
    );
}

#[test]
fn generator_works_locked() {
    let mut fx = fresh();
    let pw = fx
        .app
        .generate(&GenerateRequest::Password(PasswordRecipe::default()))
        .unwrap();
    assert_eq!((pw.value.chars().count(), pw.strength), (20, "excellent"));
    let phrase = fx
        .app
        .generate(&GenerateRequest::Passphrase(PassphraseRecipe {
            separator: " ".into(), // some words contain '-' ("t-shirt")
            ..PassphraseRecipe::default()
        }))
        .unwrap();
    assert_eq!(phrase.value.split(' ').count(), 5);
    let pin = fx
        .app
        .generate(&GenerateRequest::Pin { digits: 4 })
        .unwrap();
    assert!(pin.value.chars().all(|c| c.is_ascii_digit()));
    assert!(
        fx.app
            .generate(&GenerateRequest::Pin { digits: 1 })
            .is_err()
    );
}

#[test]
fn import_and_export_files() {
    let mut fx = with_account();
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("in.csv");
    std::fs::write(&csv, "name,url,username,password\nA,https://a.com,u,1\n").unwrap();
    assert_eq!(fx.app.import_csv(&csv, None, false).unwrap().imported, 1);
    assert_eq!(
        fx.app
            .import_csv(&dir.path().join("missing.csv"), None, false)
            .unwrap_err()
            .code(),
        "io"
    );

    let out = dir.path().join("out.csv");
    assert_eq!(fx.app.export_csv(&out, None).unwrap(), 1);
    assert_eq!(
        fx.app.export_csv(&out, None).unwrap_err().code(),
        "io",
        "never overwrite"
    );
    let vault = fx.app.vaults().unwrap()[0].id.clone();
    assert_eq!(
        fx.app
            .export_csv(&dir.path().join("v.csv"), Some(&vault))
            .unwrap(),
        1
    );
    assert_eq!(
        fx.app
            .export_csv(&dir.path().join("x.csv"), Some(&VaultId::new().to_string()))
            .unwrap_err(),
        AppError::NotFound("vault")
    );
}

#[test]
fn settings_are_applied_and_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let cfg = AppConfig {
        settings_path: Some(path.clone()),
        ..config(&dir)
    };
    let mut app = AppService::new(cfg.clone(), Box::new(MemorySecretKeyStore::default()));
    assert_eq!(app.settings().auto_lock_minutes, 0);
    assert_eq!(
        app.update_settings(Settings {
            auto_lock_minutes: 0,
            ..Settings::default()
        })
        .unwrap_err()
        .code(),
        "validation"
    );
    app.update_settings(Settings {
        auto_lock_minutes: 5,
        ..Settings::default()
    })
    .unwrap();
    assert_eq!(app.settings().auto_lock_minutes, 5);
    app.update_settings(Settings {
        auto_lock_minutes: 5,
        language: "pt-BR".into(),
        ..Settings::default()
    })
    .unwrap();

    let reopened = AppService::new(cfg, Box::new(MemorySecretKeyStore::default()));
    assert_eq!(reopened.settings().auto_lock_minutes, 5, "loaded from disk");
    assert_eq!(reopened.settings().language, "pt-BR");
}

#[test]
fn settings_without_path_stay_in_memory() {
    let mut fx = fresh();
    fx.app
        .update_settings(Settings {
            auto_lock_minutes: 2,
            ..Settings::default()
        })
        .unwrap();
    assert_eq!(fx.app.settings().auto_lock_minutes, 2);
}

#[test]
fn with_settings_builder() {
    let c = AppConfig::desktop("/s".into()).with_settings("/p.json".into());
    assert_eq!(
        c.settings_path.as_deref(),
        Some(std::path::Path::new("/p.json"))
    );
}

#[test]
fn one_time_passwords_are_computed_without_revealing_the_secret() {
    let mut fx = with_account();
    let plain = fx.app.create_item(&login("Plain", "pw")).unwrap();
    assert_eq!(fx.app.totp_code(&plain.summary.id, None).unwrap(), None);

    let mut input = login("Bank", "pw");
    input.totp = Some("JBSW Y3DP EHPK 3PXP".into());
    let bank = fx.app.create_item(&input).unwrap();
    assert!(bank.has_totp);
    let code = fx.app.totp_code(&bank.summary.id, None).unwrap().unwrap();
    assert_eq!((code.code.len(), code.period), (6, 30));
    assert!((1..=30).contains(&code.remaining));

    input.totp = Some("not a secret!".into());
    assert_eq!(fx.app.create_item(&input).unwrap_err().code(), "validation");
}

#[test]
fn the_secret_key_is_shown_again_only_with_the_master_password() {
    let mut fx = with_account();
    assert_eq!(
        fx.app.reveal_secret_key("wrong".into()).unwrap_err(),
        AppError::WrongCredentials
    );
    let shown = fx.app.reveal_secret_key(PW.into()).unwrap();
    assert_eq!(shown, fx.keys.load().unwrap().unwrap().to_display_string());
    fx.app.lock();
    assert_eq!(
        fx.app.reveal_secret_key(PW.into()).unwrap_err().code(),
        "locked"
    );
}

#[test]
fn tags_are_listed_with_counts_in_alphabetical_order() {
    let mut fx = with_account();
    for (title, tags) in [
        ("A", vec!["work", "Dev"]),
        ("B", vec!["dev"]),
        ("C", vec![]),
    ] {
        fx.app
            .create_item(&ItemInput {
                tags: tags.into_iter().map(String::from).collect(),
                ..login(title, "1")
            })
            .unwrap();
    }
    let tags: Vec<_> = fx
        .app
        .tags()
        .unwrap()
        .into_iter()
        .map(|t| (t.name, t.count))
        .collect();
    assert_eq!(
        tags,
        [("Dev".into(), 1), ("dev".into(), 1), ("work".into(), 1)]
    );
    assert_eq!(fx.app.items("", None).unwrap()[0].tags, ["work", "Dev"]);
}

#[test]
fn an_item_is_added_to_and_taken_out_of_favorites() {
    let mut fx = with_account();
    let a = fx.app.create_item(&login("Alpha", "1")).unwrap();
    fx.app.set_favorite(&a.summary.id, true).unwrap();
    assert!(fx.app.item(&a.summary.id).unwrap().summary.favorite);
    let version = fx.app.item(&a.summary.id).unwrap().version;
    fx.app.set_favorite(&a.summary.id, true).unwrap();
    assert_eq!(
        fx.app.item(&a.summary.id).unwrap().version,
        version,
        "nothing to save"
    );
    fx.app.set_favorite(&a.summary.id, false).unwrap();
    assert!(!fx.app.item(&a.summary.id).unwrap().summary.favorite);
    assert!(fx.app.set_favorite("bad", true).is_err());
}

#[test]
fn archived_items_leave_the_list_counts_and_tags_but_stay_in_the_archive() {
    let mut fx = with_account();
    let a = fx
        .app
        .create_item(&ItemInput {
            tags: vec!["old".into()],
            ..login("Alpha", "1")
        })
        .unwrap();
    fx.app.create_item(&login("Beta", "2")).unwrap();
    fx.app.set_archived(&a.summary.id, true).unwrap();

    let titles =
        |items: Vec<ItemSummaryDto>| items.into_iter().map(|i| i.title).collect::<Vec<_>>();
    assert_eq!(titles(fx.app.items("", None).unwrap()), ["Beta"]);
    assert_eq!(titles(fx.app.archived_items("").unwrap()), ["Alpha"]);
    assert!(fx.app.archived_items("zzz").unwrap().is_empty());
    assert_eq!(fx.app.vaults().unwrap()[0].item_count, 1);
    assert!(fx.app.tags().unwrap().is_empty());
    assert!(fx.app.item(&a.summary.id).unwrap().summary.archived);

    fx.app.set_archived(&a.summary.id, false).unwrap();
    assert_eq!(fx.app.items("", None).unwrap().len(), 2);
    assert!(fx.app.archived_items("").unwrap().is_empty());
}

#[test]
fn editing_an_archived_item_keeps_it_archived() {
    let mut fx = with_account();
    let a = fx.app.create_item(&login("Alpha", "1")).unwrap();
    fx.app.set_archived(&a.summary.id, true).unwrap();
    let version = fx.app.item(&a.summary.id).unwrap().version;
    fx.app
        .update_item(&a.summary.id, version, &login("Alpha 2", "1"))
        .unwrap();
    assert!(fx.app.item(&a.summary.id).unwrap().summary.archived);
}

fn field(label: &str, value: &str) -> crate::item_input::FieldValue {
    crate::item_input::FieldValue {
        label: label.into(),
        value: value.into(),
        ..Default::default()
    }
}

#[test]
fn a_credit_card_keeps_its_fields_and_conceals_the_secret_ones() {
    let mut fx = with_account();
    let card = fx
        .app
        .create_item(&ItemInput {
            kind: Some("card".into()),
            title: "Visa".into(),
            fields: Some(vec![
                field("cardholder name", "Ana Lima"),
                field("number", "4111111111111111"),
                field("expiry date", "2029-04"),
                field("PIN", ""),
                field("my note", "x"),
            ]),
            ..ItemInput::default()
        })
        .unwrap();
    assert_eq!(card.summary.kind, "card");
    let shown: Vec<_> = card
        .fields
        .iter()
        .map(|f| (f.label.as_str(), f.concealed, f.value.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [
            ("cardholder name", false, Some("Ana Lima")),
            ("number", true, None),
            ("expiry date", false, Some("2029-04")),
            ("my note", false, Some("x")),
        ],
        "empty values are dropped; the number never leaves unrevealed"
    );
    let id = card.summary.id.clone();
    assert_eq!(
        fx.app.reveal_field(&id, "number").unwrap().as_deref(),
        Some("4111111111111111")
    );
    assert_eq!(fx.app.reveal_field(&id, "nope").unwrap(), None);

    let kept = fx
        .app
        .update_item(
            &id,
            card.version,
            &ItemInput {
                title: "Visa gold".into(),
                ..ItemInput::default()
            },
        )
        .unwrap();
    assert_eq!(kept.fields.len(), 4, "no fields sent: all kept");
    let replaced = fx
        .app
        .update_item(
            &id,
            kept.version,
            &ItemInput {
                title: "Visa gold".into(),
                fields: Some(vec![field("number", "5500000000000004")]),
                ..ItemInput::default()
            },
        )
        .unwrap();
    assert_eq!(replaced.fields.len(), 1);
    assert!(replaced.fields[0].concealed);
}

#[test]
fn extra_fields_keep_their_concealed_setting_on_edit() {
    let mut fx = with_account();
    let mut item = fx
        .app
        .create_item(&ItemInput {
            kind: Some("server".into()),
            title: "Box".into(),
            fields: Some(vec![field("password", "s3cret")]),
            ..ItemInput::default()
        })
        .unwrap();
    assert!(item.fields[0].concealed);
    item = fx
        .app
        .update_item(
            &item.summary.id,
            item.version,
            &ItemInput {
                title: "Box".into(),
                fields: Some(vec![field("password", "new"), field("rack", "B2")]),
                ..ItemInput::default()
            },
        )
        .unwrap();
    assert!(item.fields[0].concealed && !item.fields[1].concealed);
}

#[test]
fn categories_list_every_kind_with_their_fields() {
    let all = AppService::categories();
    assert_eq!(all[0].kind, "login");
    assert!(all[0].fields.is_empty());
    let card = all.iter().find(|c| c.kind == "card").unwrap();
    assert_eq!(card.label, "Credit Card");
    assert!(
        card.fields
            .iter()
            .any(|f| f.label == "number" && f.concealed)
    );
    assert_eq!(all.len(), 22, "1Password's 21 and Document");
}

fn placed(label: &str, value: &str, section: &str, input: &str) -> crate::item_input::FieldValue {
    crate::item_input::FieldValue {
        section: Some(section.into()),
        input: Some(serde_json::from_value(serde_json::json!(input)).unwrap()),
        ..field(label, value)
    }
}

#[test]
fn fields_keep_their_sections_and_types() {
    let mut fx = with_account();
    let input = ItemInput {
        kind: Some("identity".into()),
        title: "Me".into(),
        fields: Some(vec![
            field("first name", "Ana"),
            placed("spouse phone", "555", "Family", "phone"),
            placed("bank 2fa", "JBSWY3DPEHPK3PXP", "Family", "totp"),
        ]),
        ..ItemInput::default()
    };
    let item = fx.app.create_item(&input).unwrap();
    let shown: Vec<_> = item
        .fields
        .iter()
        .map(|f| (f.label.as_str(), f.section.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("first name", "Identification"),
            ("spouse phone", "Family"),
            ("bank 2fa", "Family")
        ]
    );
    let otp = &item.fields[2];
    assert_eq!(otp.value, None);
    assert_eq!(
        fx.app
            .reveal_field(&item.summary.id, crate::field_layout::LAYOUT_LABEL)
            .unwrap(),
        None
    );
    let code = fx
        .app
        .totp_code(&item.summary.id, Some("bank 2fa"))
        .unwrap()
        .unwrap();
    assert_eq!(code.code.len(), 6);
    assert_eq!(
        fx.app
            .totp_code(&item.summary.id, Some("spouse phone"))
            .unwrap(),
        None
    );
}

#[test]
fn duplicate_labels_and_bad_codes_are_refused() {
    let mut fx = with_account();
    let mut input = ItemInput {
        kind: Some("identity".into()),
        title: "Me".into(),
        fields: Some(vec![
            field("pet", "Rex"),
            placed("Pet", "Bob", "More", "text"),
        ]),
        ..ItemInput::default()
    };
    assert_eq!(fx.app.create_item(&input).unwrap_err().code(), "validation");
    input.fields = Some(vec![placed("x", "not a key!", "", "totp")]);
    assert!(fx.app.create_item(&input).is_err());
    input.fields = Some(vec![field(crate::field_layout::LAYOUT_LABEL, "{}")]);
    assert_eq!(fx.app.create_item(&input).unwrap_err().code(), "validation");
}

#[test]
fn items_can_be_moved_and_duplicated() {
    let mut fx = with_account();
    let first = fx.app.create_item(&login("Mail", "pw1")).unwrap();
    fx.app
        .update_item(
            &first.summary.id,
            first.version,
            &ItemInput {
                password: Some("pw2".into()),
                ..login("Mail", "pw2")
            },
        )
        .unwrap();
    let other = fx.app.create_vault("Work").unwrap();
    let moved = fx.app.move_item(&first.summary.id, &other.id).unwrap();
    assert_eq!(moved.summary.vault_id, other.id);
    assert_eq!(
        fx.app
            .move_item(&first.summary.id, &other.id)
            .unwrap()
            .version,
        moved.version,
        "same vault: no change"
    );
    assert!(
        fx.app
            .move_item(
                &first.summary.id,
                &password365_model::VaultId::new().to_string()
            )
            .is_err()
    );

    let copy = fx
        .app
        .duplicate_item(&first.summary.id, "Mail copy")
        .unwrap();
    assert_ne!(copy.summary.id, first.summary.id);
    assert_eq!(copy.summary.title, "Mail copy");
    assert_eq!(copy.summary.vault_id, other.id);
    assert_eq!(copy.password_history_count, 0);
    assert_eq!(
        fx.app
            .reveal(&copy.summary.id, SecretField::Password)
            .unwrap()
            .as_deref(),
        Some("pw2")
    );
    assert_eq!(
        fx.app
            .duplicate_item(&first.summary.id, " ")
            .unwrap_err()
            .code(),
        "validation"
    );
}

#[test]
fn an_item_keeps_its_own_icon_until_removed() {
    let mut fx = with_account();
    let png = "data:image/png;base64,iVBORw0KGgo=";
    let created = fx
        .app
        .create_item(&ItemInput {
            icon: Some(png.into()),
            ..login("Mail", "pw")
        })
        .unwrap();
    assert_eq!(created.icon.as_deref(), Some(png));
    assert!(created.fields.is_empty(), "the icon is not a field");
    let edited = fx
        .app
        .update_item(
            &created.summary.id,
            created.version,
            &ItemInput {
                fields: Some(vec![]),
                ..login("Mail", "pw")
            },
        )
        .unwrap();
    assert_eq!(
        edited.icon.as_deref(),
        Some(png),
        "kept when the form leaves it"
    );
    assert_eq!(
        fx.app
            .item_icons()
            .unwrap()
            .get(&created.summary.id)
            .map(String::as_str),
        Some(png)
    );
    let removed = fx
        .app
        .update_item(
            &created.summary.id,
            edited.version,
            &ItemInput {
                icon: Some(String::new()),
                ..login("Mail", "pw")
            },
        )
        .unwrap();
    assert_eq!(removed.icon, None);
    assert!(fx.app.item_icons().unwrap().is_empty());
    let bad = ItemInput {
        icon: Some("data:image/svg+xml;base64,x".into()),
        ..login("X", "pw")
    };
    assert_eq!(fx.app.create_item(&bad).unwrap_err().code(), "validation");
}

/// Throwaway keys made for these tests with `ssh-keygen -t ed25519` (never used anywhere).
const TEST_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACClIwgox17xbaFiAWOtAuYNM6oUlB4rckUUIOHWHtqI6wAAAIgRvxy2Eb8c
tgAAAAtzc2gtZWQyNTUxOQAAACClIwgox17xbaFiAWOtAuYNM6oUlB4rckUUIOHWHtqI6w
AAAEB+T1EbaJV6sUPO0BPiXN/yODFGP16hPNWTSaWBJrp05aUjCCjHXvFtoWIBY60C5g0z
qhSUHityRRQg4dYe2ojrAAAABHRlc3QB
-----END OPENSSH PRIVATE KEY-----";
const TEST_KEY_WITH_PASSPHRASE: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABDWwkQb/J
VBhkuikd0vBl/eAAAAGAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIHM7JxfyVuHqFepx
szWXacFHKxX3dKw8sui8Tkejj3ZWAAAAkM9ug4ERKqdInxQdgjehgBTo7Mw/Lia3ifm1Q1
eBHWWtatpyrP/IUlyweUf91yurlRSEU8QuaBdOIMqqHBvIiE9LnkWtVsBxTyQnkaFS52K2
UZhQyWaFUAPphlJcMDM8Biur+IEZ4F3zUK3gkEHyd5fOg3M4JHhcEkkqSW/M5hObxTvMvH
hDVxU1BFDKOysaZw==
-----END OPENSSH PRIVATE KEY-----";

#[test]
fn the_ssh_agent_offers_usable_keys_only_when_turned_on() {
    let mut fx = with_account();
    let key = |title: &str, pem: &str, passphrase: Option<&str>| {
        let mut fields = vec![field("private key", pem)];
        if let Some(p) = passphrase {
            fields.push(field("passphrase", p));
        }
        ItemInput {
            kind: Some("ssh_key".into()),
            title: title.into(),
            fields: Some(fields),
            ..ItemInput::default()
        }
    };
    fx.app.create_item(&key("GitHub", TEST_KEY, None)).unwrap();
    fx.app
        .create_item(&key("Locked", TEST_KEY_WITH_PASSPHRASE, None))
        .unwrap();
    fx.app
        .create_item(&key("Opened", TEST_KEY_WITH_PASSPHRASE, Some("pp")))
        .unwrap();
    fx.app
        .create_item(&key("Broken", "nonsense", None))
        .unwrap();
    let mut listed = fx.app.ssh_keys().unwrap();
    listed.sort_by(|a, b| a.title.cmp(&b.title));
    let summary: Vec<_> = listed
        .iter()
        .map(|k| {
            (
                k.title.as_str(),
                k.fingerprint.is_some(),
                k.problem.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("Broken", false, Some("unreadable")),
            ("GitHub", true, None),
            ("Locked", false, Some("encrypted")),
            ("Opened", true, None)
        ]
    );
    assert!(
        fx.app.ssh_agent_keys().unwrap().is_empty(),
        "off by default"
    );
    let on = Settings {
        ssh_agent: true,
        ..fx.app.settings()
    };
    fx.app
        .update_settings(Settings {
            auto_lock_minutes: 10,
            ..on
        })
        .unwrap();
    assert_eq!(fx.app.ssh_agent_keys().unwrap().len(), 2);
}

#[test]
fn wrong_passwords_make_the_next_try_wait_and_never_delete_anything() {
    let mut fx = with_account();
    fx.app.lock();
    let wrong = || SecretString::from("not the password");
    for _ in 0..2 {
        assert_eq!(
            fx.app.unlock(wrong(), None).unwrap_err(),
            AppError::WrongCredentials
        );
    }
    assert_eq!(
        fx.app.unlock(wrong(), None).unwrap_err(),
        AppError::WrongCredentials
    );
    assert!(
        matches!(fx.app.unlock(PW.into(), None).unwrap_err(), AppError::TooManyAttempts(s) if s >= 4),
        "even the right password waits"
    );
    for _ in 3..9 {
        fx.app.attempts.skip_wait();
        assert_eq!(
            fx.app.unlock(wrong(), None).unwrap_err(),
            AppError::WrongCredentials
        );
    }
    for _ in 0..5 {
        fx.app.attempts.skip_wait();
        assert_eq!(
            fx.app.unlock(wrong(), None).unwrap_err(),
            AppError::WrongCredentials,
            "many wrong passwords never delete the Secret Key"
        );
    }
    assert!(fx.keys.load().unwrap().is_some());
    fx.app.attempts.skip_wait();
    fx.app.unlock(PW.into(), None).unwrap();
    fx.app.lock();
    assert_eq!(
        fx.app.unlock(wrong(), None).unwrap_err(),
        AppError::WrongCredentials,
        "count started over"
    );
    fx.app.unlock(PW.into(), None).unwrap();
}

#[test]
fn sign_in_with_points_at_another_item() {
    let mut fx = with_account();
    let google = fx.app.create_item(&login("Google", "pw")).unwrap();
    let field = |value: &str| crate::item_input::FieldValue {
        label: "sign in with".into(),
        value: value.into(),
        input: Some(password365_model::item_template::FieldInput::SignInWith),
        ..crate::item_input::FieldValue::default()
    };
    let site = fx
        .app
        .create_item(&ItemInput {
            fields: Some(vec![field(&google.summary.id)]),
            ..login("Figma", "")
        })
        .unwrap();
    assert_eq!(
        site.fields[0].value.as_deref(),
        Some(google.summary.id.as_str())
    );
    let bad = ItemInput {
        fields: Some(vec![field("not an id")]),
        ..login("X", "")
    };
    assert_eq!(fx.app.create_item(&bad).unwrap_err().code(), "validation");
}

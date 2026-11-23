//! Tests for the browser-extension operations.

use password365_crypto::kdf::KdfParams;

use super::*;
use crate::AppConfig;
use crate::item_input::ItemInput;
use crate::secret_key_store::MemorySecretKeyStore;

const PW: &str = "correct horse battery";

fn app() -> (tempfile::TempDir, AppService) {
    let dir = tempfile::tempdir().unwrap();
    let config = AppConfig {
        store_path: dir.path().join("s.qlvault"),
        kdf: KdfParams::insecure_for_tests(),
        auto_lock_ms: 0,
        settings_path: None,
    };
    let mut app = AppService::new(config, Box::new(MemorySecretKeyStore::default()));
    app.create_account(PW.into(), &PW.into()).unwrap();
    (dir, app)
}

fn add(app: &mut AppService, title: &str, url: &str, user: &str, pw: &str) -> String {
    app.create_item(&ItemInput {
        title: title.into(),
        username: Some(user.into()),
        urls: vec![url.into()],
        password: Some(pw.into()),
        ..ItemInput::default()
    })
    .unwrap()
    .summary
    .id
}

fn submitted(url: &str, user: &str, pw: &str) -> SubmittedLogin {
    SubmittedLogin {
        page_url: url.into(),
        username: Some(user.into()),
        password: pw.into(),
        title: None,
    }
}

#[test]
fn candidates_are_matched_and_ranked() {
    let (_d, mut app) = app();
    add(
        &mut app,
        "Google (site)",
        "https://www.google.com",
        "a",
        "1",
    );
    add(
        &mut app,
        "Google (origin)",
        "https://accounts.google.com",
        "b",
        "2",
    );
    add(&mut app, "GitHub", "https://github.com", "c", "3");
    add(&mut app, "Phish", "https://google.com.evil.io", "d", "4");
    app.create_item(&ItemInput {
        kind: Some("note".into()),
        title: "Note".into(),
        urls: vec!["https://accounts.google.com".into()],
        ..ItemInput::default()
    })
    .unwrap();

    let found = app
        .logins_for_url("https://accounts.google.com/signin?x")
        .unwrap();
    let titles: Vec<_> = found
        .iter()
        .map(|c| (c.title.as_str(), c.match_level))
        .collect();
    assert_eq!(
        titles,
        vec![("Google (origin)", "origin"), ("Google (site)", "site")]
    );
    assert!(
        app.logins_for_url("https://example.org")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn no_candidates_on_http_for_https_logins() {
    let (_d, mut app) = app();
    add(&mut app, "Bank", "https://bank.example", "u", "p");
    assert!(
        app.logins_for_url("http://bank.example")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn fill_is_released_only_for_matching_pages() {
    let (_d, mut app) = app();
    let id = add(&mut app, "GitHub", "https://github.com", "octo", "s3cret");
    let creds = app
        .fill_credentials(&id, "https://github.com/login", false)
        .unwrap();
    assert_eq!(
        creds,
        FillCredentials {
            username: Some("octo".into()),
            password: Some("s3cret".into())
        }
    );

    let err = app
        .fill_credentials(&id, "https://github.com.evil.io/login", false)
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    assert_eq!(
        app.fill_credentials(&id, "not a url at all ://", false)
            .unwrap_err()
            .code(),
        "validation"
    );
}

#[test]
fn submission_classification() {
    let (_d, mut app) = app();
    let id = add(&mut app, "GitHub", "https://github.com", "Octo", "old");
    assert_eq!(
        app.check_submitted(&submitted("https://github.com/session", "octo", "old"))
            .unwrap(),
        SubmissionCheck::Known { id: id.clone() }
    );
    assert_eq!(
        app.check_submitted(&submitted("https://github.com/session", "octo", "new"))
            .unwrap(),
        SubmissionCheck::PasswordChanged { id }
    );
    assert_eq!(
        app.check_submitted(&submitted("https://github.com", "someone-else", "x"))
            .unwrap(),
        SubmissionCheck::New
    );
    assert_eq!(
        app.check_submitted(&submitted("https://gitlab.com", "octo", "old"))
            .unwrap(),
        SubmissionCheck::New
    );
}

#[test]
fn saving_a_new_login() {
    let (_d, mut app) = app();
    let mut login = submitted("https://www.shop.example/login?next=/", "me@x.com", "pw1");
    let saved = app.save_submitted(&login, None).unwrap();
    assert_eq!(saved.summary.title, "shop.example");
    assert_eq!(saved.summary.urls, vec!["https://www.shop.example"]);
    assert_eq!(
        app.check_submitted(&login).unwrap(),
        SubmissionCheck::Known {
            id: saved.summary.id.clone()
        }
    );

    login.username = Some("other".into());
    login.title = Some("My Shop".into());
    assert_eq!(
        app.save_submitted(&login, None).unwrap().summary.title,
        "My Shop"
    );
}

#[test]
fn saving_updates_a_changed_password_and_keeps_history() {
    let (_d, mut app) = app();
    let id = add(&mut app, "GitHub", "https://github.com", "octo", "old");
    let updated = app
        .save_submitted(&submitted("https://github.com", "octo", "new"), None)
        .unwrap();
    assert_eq!(updated.summary.id, id);
    assert_eq!(updated.password_history_count, 1);
    assert_eq!(
        app.fill_credentials(&id, "https://github.com", false)
            .unwrap()
            .password
            .as_deref(),
        Some("new")
    );

    let same = app
        .save_submitted(&submitted("https://github.com", "octo", "new"), None)
        .unwrap();
    assert_eq!(
        same.version, updated.version,
        "unchanged password is a no-op"
    );
}

#[test]
fn saving_rejects_empty_password_and_bad_pages() {
    let (_d, mut app) = app();
    assert_eq!(
        app.save_submitted(&submitted("https://a.com", "u", ""), None)
            .unwrap_err()
            .code(),
        "validation"
    );
    assert_eq!(
        app.save_submitted(&submitted("chrome://settings", "u", "p"), None)
            .unwrap_err()
            .code(),
        "validation"
    );
}

#[test]
fn locked_vault_refuses_everything() {
    let (_d, mut app) = app();
    app.lock();
    assert_eq!(
        app.logins_for_url("https://a.com").unwrap_err(),
        AppError::Locked
    );
    assert_eq!(
        app.check_submitted(&submitted("https://a.com", "u", "p"))
            .unwrap_err(),
        AppError::Locked
    );
}

#[test]
fn submitted_login_json_shape() {
    let login: SubmittedLogin =
        serde_json::from_str(r#"{"page_url":"https://a.com","password":"p"}"#).unwrap();
    assert!(login.username.is_none() && login.title.is_none());
    let json = serde_json::to_string(&SubmissionCheck::PasswordChanged { id: "x".into() }).unwrap();
    assert_eq!(json, r#"{"status":"password_changed","id":"x"}"#);
}

#[test]
fn archived_logins_are_not_offered() {
    let (_d, mut app) = app();
    let id = add(&mut app, "Old bank", "https://bank.example", "u", "p");
    app.set_archived(&id, true).unwrap();
    assert!(
        app.logins_for_url("https://bank.example")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_related_site_asks_once_and_is_remembered_and_other_sites_still_ask() {
    let (_dir, mut app) = app();
    let shop = add(&mut app, "Amazon", "https://www.amazon.com", "ana", "pw1");
    // Offered on a related country site, marked as new there.
    let offered = app
        .logins_for_url("https://www.amazon.es/ap/signin")
        .unwrap();
    let candidate = offered.iter().find(|l| l.id == shop).unwrap();
    assert_eq!(
        (candidate.match_level, candidate.new_site),
        ("related", true)
    );
    // Without the person's confirmation nothing is released.
    assert_eq!(
        app.fill_credentials(&shop, "https://www.amazon.es/", false)
            .unwrap_err()
            .code(),
        "new_site"
    );
    // Confirmed: released, and the site is remembered (also across a lock and unlock).
    let creds = app
        .fill_credentials(&shop, "https://www.amazon.es/", true)
        .unwrap();
    assert_eq!(creds.password.as_deref(), Some("pw1"));
    app.lock();
    app.unlock(PW.into(), None).unwrap();
    let again = app.logins_for_url("https://www.amazon.es/").unwrap();
    assert!(!again.iter().find(|l| l.id == shop).unwrap().new_site);
    assert!(
        app.fill_credentials(&shop, "https://www.amazon.es/", false)
            .is_ok()
    );
    // Another related site is new again.
    assert_eq!(
        app.fill_credentials(&shop, "https://www.amazon.de/", false)
            .unwrap_err()
            .code(),
        "new_site"
    );
    // The login's own site never asks.
    assert!(
        app.fill_credentials(&shop, "https://www.amazon.com/", false)
            .is_ok()
    );
}

#[test]
fn what_is_remembered_does_not_disturb_the_security_settings_sharing_the_sealed_block() {
    let (_dir, mut app) = app();
    let shop = add(&mut app, "Amazon", "https://amazon.com", "ana", "pw");
    app.fill_credentials(&shop, "https://amazon.es", true)
        .unwrap();
    let mut settings = app.settings();
    settings.auto_lock_minutes = 7;
    app.update_settings(settings).unwrap();
    app.lock();
    app.unlock(PW.into(), None).unwrap();
    assert_eq!(
        app.settings().auto_lock_minutes,
        7,
        "the security settings survive"
    );
    assert!(
        app.fill_credentials(&shop, "https://amazon.es", false)
            .is_ok(),
        "and so does the memory"
    );
}

#[test]
fn a_login_saved_for_one_country_site_is_offered_on_the_brands_others_and_a_wildcard_on_any() {
    let (_dir, mut app) = app();
    let shop = add(&mut app, "Amazon", "https://www.amazon.com", "ana", "pw1");
    let any = add(&mut app, "Brand", "mybrand.*", "ana", "pw2");
    let offered: Vec<String> = app
        .logins_for_url("https://www.amazon.es/ap/signin")
        .unwrap()
        .into_iter()
        .map(|l| l.id)
        .collect();
    assert!(offered.contains(&shop));
    for page in ["https://mybrand.es/login", "https://mybrand.co.uk/"] {
        assert!(
            app.logins_for_url(page)
                .unwrap()
                .iter()
                .any(|l| l.id == any),
            "{page}"
        );
        assert!(app.fill_credentials(&any, page, true).is_ok());
    }
    assert!(
        app.logins_for_url("https://mybrand-login.com/")
            .unwrap()
            .iter()
            .all(|l| l.id != any)
    );
    assert!(
        app.fill_credentials(&any, "https://mybrand-login.com/", true)
            .is_err()
    );
}

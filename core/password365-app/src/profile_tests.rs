//! Tests for the Profile (default identity and card).

use super::*;
use crate::item_input::{FieldValue, ItemInput};
use crate::service::tests::with_account;

fn field(label: &str, value: &str) -> FieldValue {
    FieldValue {
        label: label.into(),
        value: value.into(),
        ..Default::default()
    }
}

fn add(app: &mut AppService, kind: &str, title: &str, fields: Vec<FieldValue>) -> String {
    app.create_item(&ItemInput {
        kind: Some(kind.into()),
        title: title.into(),
        fields: Some(fields),
        ..ItemInput::default()
    })
    .unwrap()
    .summary
    .id
}

fn card(app: &mut AppService, title: &str) -> String {
    add(
        app,
        "card",
        title,
        vec![
            field("cardholder name", "Ana Lima"),
            field("number", "4111 1111 1111 4242"),
            field("verification number", "123"),
            field("expiry date", "2029-04"),
        ],
    )
}

#[test]
fn the_only_identity_and_card_are_used_until_one_is_chosen() {
    let mut fx = with_account();
    assert_eq!(fx.app.profile().unwrap(), ProfileDto::default());
    let visa = card(&mut fx.app, "Visa");
    let me = add(
        &mut fx.app,
        "identity",
        "Me",
        vec![field("first name", "Ana")],
    );
    let p = fx.app.profile().unwrap();
    assert_eq!(p.identity.unwrap().id, me);
    let shown = p.card.unwrap();
    assert_eq!(
        (shown.id.as_str(), shown.last4.as_deref()),
        (visa.as_str(), Some("4242"))
    );

    let master = card(&mut fx.app, "Master");
    assert_eq!(
        fx.app.profile().unwrap().card,
        None,
        "two cards: choose one"
    );
    fx.app
        .set_profile(ProfileKind::Card, Some(&master))
        .unwrap();
    assert_eq!(fx.app.profile().unwrap().card.unwrap().title, "Master");
    fx.app.set_archived(&master, true).unwrap();
    assert_eq!(
        fx.app.profile().unwrap().card.unwrap().title,
        "Visa",
        "an archived choice falls back to the only one left"
    );
    fx.app.set_profile(ProfileKind::Card, None).unwrap();
}

#[test]
fn only_items_of_the_right_kind_can_be_chosen() {
    let mut fx = with_account();
    let me = add(&mut fx.app, "identity", "Me", vec![]);
    let err = fx
        .app
        .set_profile(ProfileKind::Card, Some(&me))
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn the_choice_is_remembered_on_this_device() {
    let dir = tempfile::tempdir().unwrap();
    let settings = dir.path().join("settings.json");
    let config = |dir: &tempfile::TempDir| crate::AppConfig {
        store_path: dir.path().join("s.qlvault"),
        kdf: password365_crypto::kdf::KdfParams::insecure_for_tests(),
        auto_lock_ms: 0,
        settings_path: Some(settings.clone()),
    };
    let keys = crate::secret_key_store::MemorySecretKeyStore::default();
    let mut app = AppService::new(config(&dir), Box::new(keys.clone()));
    let pw = "correct horse battery";
    app.create_account(pw.into(), &pw.into()).unwrap();
    card(&mut app, "A");
    let b = card(&mut app, "B");
    app.set_profile(ProfileKind::Card, Some(&b)).unwrap();
    let mut again = AppService::new(config(&dir), Box::new(keys));
    again.unlock(pw.into(), None).unwrap();
    assert_eq!(again.profile().unwrap().card.unwrap().id, b);
}

#[test]
fn card_values_follow_html_autocomplete_names_and_need_https() {
    let mut fx = with_account();
    card(&mut fx.app, "Visa");
    let v = fx
        .app
        .profile_form_values(ProfileKind::Card, "https://shop.example/pay")
        .unwrap();
    assert_eq!(v["cc-name"], "Ana Lima");
    assert_eq!(v["cc-number"], "4111111111114242");
    assert_eq!(v["cc-csc"], "123");
    assert_eq!(
        (
            v["cc-exp-month"].as_str(),
            v["cc-exp-year"].as_str(),
            v["cc-exp"].as_str()
        ),
        ("04", "2029", "04/29")
    );
    let err = fx
        .app
        .profile_form_values(ProfileKind::Card, "http://shop.example/pay")
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    assert!(
        fx.app
            .profile_form_values(ProfileKind::Card, "http://localhost:3000/pay")
            .is_ok()
    );
    assert_eq!(
        fx.app
            .profile_form_values(ProfileKind::Identity, "https://x.example")
            .unwrap_err()
            .code(),
        "not_found"
    );
}

#[test]
fn identity_values_include_full_name_address_lines_and_birthday_parts() {
    let mut fx = with_account();
    add(
        &mut fx.app,
        "identity",
        "Me",
        vec![
            field("first name", "Ana"),
            field("initial", "M"),
            field("last name", "Lima"),
            field("address", "Rua A, 10\nApto 3\n\n"),
            field("zip", "01000-000"),
            field("birth date", "1990-07-15"),
            field("email", "ana@example.com"),
        ],
    );
    let v = fx
        .app
        .profile_form_values(ProfileKind::Identity, "http://forms.example")
        .unwrap();
    assert_eq!(v["name"], "Ana M Lima");
    assert_eq!(v["address-line1"], "Rua A, 10");
    assert_eq!(v["address-line2"], "Apto 3");
    assert_eq!(v["postal-code"], "01000-000");
    assert_eq!(
        (
            v["bday-year"].as_str(),
            v["bday-month"].as_str(),
            v["bday-day"].as_str()
        ),
        ("1990", "07", "15")
    );
    assert_eq!(v["email"], "ana@example.com");
    assert!(!v.contains_key("tel"));
}

#[test]
fn a_brazilian_identity_fills_documents_split_address_and_own_fields() {
    let mut fx = with_account();
    fx.app
        .create_item(&ItemInput {
            kind: Some("identity".into()),
            title: "Eu".into(),
            region: Some("BR".into()),
            fields: Some(vec![
                field("first name", "Ana"),
                field("CPF", "123.456.789-09"),
                field("address", "Rua das Flores"),
                field("number", "100"),
                field("neighborhood", "Centro"),
                field("state", "SP"),
                field("matrícula", "A-77"),
            ]),
            ..ItemInput::default()
        })
        .unwrap();
    let v = fx
        .app
        .profile_form_values(ProfileKind::Identity, "https://loja.example")
        .unwrap();
    assert_eq!(v["cpf"], "123.456.789-09");
    assert_eq!(v["address-line1"], "Rua das Flores");
    assert_eq!(v["house-number"], "100");
    assert_eq!(v["street-address"], "Rua das Flores, 100");
    assert_eq!(v["address-level3"], "Centro");
    assert_eq!(v["address-level1"], "SP");
    assert_eq!(v["label:matrícula"], "A-77");
    assert!(!v.keys().any(|k| k.contains("layout")));
}

#[test]
fn an_unknown_country_is_refused() {
    let mut fx = with_account();
    let input = ItemInput {
        kind: Some("identity".into()),
        title: "Eu".into(),
        region: Some("brazil".into()),
        fields: Some(vec![field("first name", "Ana")]),
        ..ItemInput::default()
    };
    assert_eq!(fx.app.create_item(&input).unwrap_err().code(), "validation");
}

#[test]
fn every_identity_is_offered_and_any_can_fill() {
    let mut fx = with_account();
    let home = add(
        &mut fx.app,
        "identity",
        "Home",
        vec![field("first name", "Ana")],
    );
    let work = add(
        &mut fx.app,
        "identity",
        "Work",
        vec![field("first name", "Dra. Ana")],
    );
    let visa = card(&mut fx.app, "Visa");
    fx.app
        .set_profile(ProfileKind::Identity, Some(&work))
        .unwrap();
    let p = fx.app.profile().unwrap();
    assert_eq!(
        p.identities
            .iter()
            .map(|i| i.title.as_str())
            .collect::<Vec<_>>(),
        ["Work", "Home"]
    );
    assert_eq!(p.identity.unwrap().id, work);
    assert_eq!(p.cards.len(), 1);
    let v = fx
        .app
        .profile_form_values_of(ProfileKind::Identity, Some(&home), "http://forms.example")
        .unwrap();
    assert_eq!(v["given-name"], "Ana");
    let err = fx
        .app
        .profile_form_values_of(ProfileKind::Identity, Some(&visa), "https://x.example")
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

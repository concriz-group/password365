//! Tests for auto-type planning.

use password365_model::{CustomField, ItemDetails, ItemKind, ItemOverview, VaultId};

use super::*;

fn item(title: &str, extra: &[(&str, &str)]) -> Item {
    let mut details = ItemDetails::with_password("pw!".into());
    details.totp = Some("JBSWY3DPEHPK3PXP".into());
    for (label, value) in extra {
        details.custom_fields.push(CustomField {
            label: (*label).into(),
            value: (*value).into(),
            concealed: false,
        });
    }
    Item::new(
        VaultId::new(),
        ItemKind::Login,
        ItemOverview::new(title)
            .unwrap()
            .with_username("ana")
            .with_url("https://x.com"),
        details,
        Timestamp::from_millis(1),
    )
}

fn text(s: &str) -> Step {
    Step::Text(s.into())
}

#[test]
fn the_default_sequence_types_username_tab_password_enter() {
    let s = steps(&item("Mail", &[]), Timestamp::from_millis(0)).unwrap();
    assert_eq!(
        s,
        [
            Step::KeyDelay(DEFAULT_KEY_DELAY_MS),
            text("ana"),
            Step::Key(Key::Tab),
            text("pw!"),
            Step::Key(Key::Enter),
        ]
    );
}

#[test]
fn a_custom_sequence_with_otp_delays_fields_and_literals() {
    let it = item(
        "VPN",
        &[
            (
                SEQUENCE_LABEL,
                "{DELAY=80}{USERNAME}{TAB}{PASSWORD}{ENTER}{DELAY 1500}{TOTP}{enter}x{{}{}}{S:pin}{TITLE}{URL}",
            ),
            ("PIN", "1234"),
        ],
    );
    // RFC 6238 test time: 59 s → 287082 for this key with SHA-1 / 6 digits.
    let s = steps(&it, Timestamp::from_millis(59_000)).unwrap();
    assert_eq!(
        s,
        [
            Step::KeyDelay(DEFAULT_KEY_DELAY_MS),
            Step::KeyDelay(80),
            text("ana"),
            Step::Key(Key::Tab),
            text("pw!"),
            Step::Key(Key::Enter),
            Step::Pause(1500),
            text(&Totp::parse("JBSWY3DPEHPK3PXP").unwrap().code_at(59).code),
            Step::Key(Key::Enter),
            text("x"),
            text("{"),
            text("}"),
            text("1234"),
            text("VPN"),
            text("https://x.com"),
        ]
    );
}

#[test]
fn every_key_has_a_name() {
    let names = "{SPACE}{BS}{BACKSPACE}{ESC}{UP}{DOWN}{LEFT}{RIGHT}";
    let it = item("K", &[(SEQUENCE_LABEL, names)]);
    let keys: Vec<_> = steps(&it, Timestamp::from_millis(0)).unwrap()[1..].to_vec();
    assert_eq!(keys.len(), 8);
}

#[test]
fn missing_values_type_nothing() {
    let mut it = item("Bare", &[(SEQUENCE_LABEL, "{TOTP}{S:nope}")]);
    it.details.totp = None;
    let s = steps(&it, Timestamp::from_millis(0)).unwrap();
    assert_eq!(s[1..], [text(""), text("")]);
}

#[test]
fn bad_sequences_are_refused() {
    for bad in ["{USERNAME", "{NOPE}", "{DELAY x}", "{DELAY=99999999}"] {
        assert_eq!(validate(bad).unwrap_err().code(), "validation", "{bad}");
    }
    validate("plain text").unwrap();
    let it = item("Bad", &[(SEQUENCE_LABEL, "{NOPE}")]);
    assert!(steps(&it, Timestamp::from_millis(0)).is_err());
}

#[test]
fn windows_match_by_pattern_or_by_title() {
    let rdp = item(
        "Server",
        &[(WINDOW_LABEL, "*Remote Desktop*\n\n  Putty - * ")],
    );
    assert!(matches_window(&rdp, "srv01 - Remote Desktop Connection"));
    assert!(matches_window(&rdp, "PuTTY - root@box"));
    assert!(
        !matches_window(&rdp, "Server settings"),
        "patterns replace the title rule"
    );
    let mail = item("Outlook", &[]);
    assert!(matches_window(&mail, "Inbox - OUTLOOK"));
    assert!(!matches_window(&mail, "Notes"));
    assert!(
        !matches_window(&item("ab", &[]), "lab report"),
        "too short to guess"
    );
}

#[test]
fn wildcards() {
    assert!(wildcard("a*c", "abc"));
    assert!(wildcard("a*c", "ac"));
    assert!(wildcard("*", ""));
    assert!(wildcard("abc", "abc"));
    assert!(!wildcard("abc", "abcd"));
    assert!(!wildcard("a*b*c", "acb"));
    assert!(wildcard("a*b*c", "aXbYc"));
    assert!(!wildcard("ab*ba", "aba"), "the ends may not overlap");
}

#[test]
fn the_service_lists_window_candidates_and_plans_steps() {
    let mut fx = crate::service::tests::with_account();
    let a = fx
        .app
        .create_item(&crate::service::tests::login("Gmail", "pw"))
        .unwrap();
    fx.app
        .create_item(&crate::service::tests::login("Bank", "pw"))
        .unwrap();
    let found = fx.app.auto_type_candidates("Gmail - Inbox").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, a.summary.id);
    fx.app.set_archived(&a.summary.id, true).unwrap();
    assert!(
        fx.app
            .auto_type_candidates("Gmail - Inbox")
            .unwrap()
            .is_empty()
    );
    let s = fx.app.auto_type_steps(&a.summary.id).unwrap();
    assert_eq!(s[1], text("me"));
}

#[test]
fn a_bad_sequence_is_refused_when_saving() {
    let mut fx = crate::service::tests::with_account();
    let input = crate::item_input::ItemInput {
        fields: Some(vec![crate::item_input::FieldValue {
            label: SEQUENCE_LABEL.into(),
            value: "{USERNAME".into(),
            ..Default::default()
        }]),
        ..crate::service::tests::login("X", "pw")
    };
    assert_eq!(fx.app.create_item(&input).unwrap_err().code(), "validation");
}

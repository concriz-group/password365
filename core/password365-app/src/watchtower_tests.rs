//! Tests for the Watchtower report.

use password365_model::{CustomField, ItemDetails, ItemKind, ItemOverview, VaultId};

use super::*;

const STRONG: &str = "correct horse battery staple vault";
const NOW_MS: i64 = 1_794_528_000_000; // 2026-11-13

fn login(title: &str, password: Option<&str>, url: &str) -> Item {
    Item::new(
        VaultId::new(),
        ItemKind::Login,
        ItemOverview::new(title).unwrap().with_url(url),
        password.map_or_else(ItemDetails::default, |p| {
            ItemDetails::with_password(p.into())
        }),
        Timestamp::from_millis(1),
    )
}

fn card(title: &str, expiry: &str) -> Item {
    let mut item = Item::new(
        VaultId::new(),
        ItemKind::CreditCard,
        ItemOverview::new(title).unwrap(),
        ItemDetails::default(),
        Timestamp::from_millis(1),
    );
    item.details.custom_fields.push(CustomField {
        label: "expiry date".into(),
        value: expiry.into(),
        concealed: false,
    });
    item
}

fn titles(refs: &[ItemRef]) -> Vec<&str> {
    refs.iter().map(|r| r.title.as_str()).collect()
}

#[test]
fn weak_reused_and_unsecured_items_are_found() {
    let items = [
        login("Strong", Some(STRONG), "https://a.com"),
        login("Weak", Some("password1"), "https://b.com"),
        login("Copy 1", Some("Tr0ub4dor&3-zebra-lamp"), "https://c.com"),
        login("Copy 2", Some("Tr0ub4dor&3-zebra-lamp"), "http://d.com"),
        login("No password", None, "HTTP://e.com"),
    ];
    let r = report(&items, Timestamp::from_millis(NOW_MS));
    assert_eq!(titles(&r.weak), ["Weak"]);
    assert_eq!(r.reused.len(), 1);
    assert_eq!(titles(&r.reused[0]), ["Copy 1", "Copy 2"]);
    assert_eq!(titles(&r.unsecured), ["Copy 2", "No password"]);
    assert_eq!(r.strength.weak, 1);
    assert_eq!(r.strength.strong + r.strength.good, 3);
    let score = r.score.unwrap();
    assert!(score < 500, "two reused and one weak out of four: {score}");
    assert_eq!(r.rating, Some(rating(score)));
}

#[test]
fn strong_unique_passwords_score_high_and_nothing_scores_nothing() {
    let items = [login("A", Some(STRONG), "https://a.com")];
    let r = report(&items, Timestamp::from_millis(NOW_MS));
    assert_eq!(r.score, Some(1000));
    assert_eq!(r.rating, Some("excellent"));
    let empty = report(&[], Timestamp::from_millis(NOW_MS));
    assert_eq!((empty.score, empty.rating), (None, None));
}

#[test]
fn ratings_follow_the_score() {
    assert_eq!(
        [950, 800, 650, 450, 100].map(rating),
        ["excellent", "very_good", "good", "fair", "poor"]
    );
}

#[test]
fn cards_expiring_soon_or_expired_are_listed() {
    let items = [
        card("Expired", "2026-10"),
        card("This month", "2026-11"),
        card("Next month", "2026-12-10"),
        card("Later", "2027-06"),
        card("Garbage", "soon"),
        card("Bad day", "2026-02-30"),
    ];
    let r = report(&items, Timestamp::from_millis(NOW_MS));
    let found: Vec<_> = r
        .expiring
        .iter()
        .map(|e| (e.item.title.as_str(), e.expired))
        .collect();
    assert_eq!(
        found,
        [
            ("Expired", true),
            ("This month", false),
            ("Next month", false)
        ]
    );
    assert_eq!(r.expiring[0].date, "2026-10");
}

#[test]
fn civil_dates_are_counted_from_1970() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    assert_eq!(
        last_valid_day("2024-02"),
        Some(days_from_civil(2024, 2, 29))
    );
    assert_eq!(
        last_valid_day("2023-02"),
        Some(days_from_civil(2023, 2, 28))
    );
    assert_eq!(last_valid_day("2023-13"), None);
    assert_eq!(last_valid_day("2023-01-01-01"), None);
    assert_eq!(last_valid_day("2023-x"), None);
}

#[test]
fn the_service_leaves_archived_items_out() {
    let mut fx = crate::service::tests::with_account();
    let weak = fx
        .app
        .create_item(&crate::service::tests::login("Weak", "password1"))
        .unwrap();
    assert_eq!(fx.app.watchtower().unwrap().weak.len(), 1);
    fx.app.set_archived(&weak.summary.id, true).unwrap();
    let r = fx.app.watchtower().unwrap();
    assert!(r.weak.is_empty() && r.score.is_none());
}

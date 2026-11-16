//! Helpers shared by the integration tests.
#![allow(dead_code)]

use password365_crypto::kdf::KdfParams;
use password365_crypto::secret_key::AccountSecretKey;
use password365_model::{ItemDetails, ItemKind, ItemOverview, Timestamp};
use password365_vault::{Credentials, Peer, Session, Store};

/// Two stores paired with each other (B joined A).
pub struct Pair {
    pub _dir: tempfile::TempDir,
    pub a: Session,
    pub b: Session,
}

pub fn paired() -> Pair {
    let dir = tempfile::tempdir().expect("temp dir");
    let secret_key = AccountSecretKey::generate().expect("random");
    let creds = Credentials {
        password: "master".into(),
        secret_key: secret_key.clone(),
    };
    let kdf = KdfParams::insecure_for_tests();
    let mut a = Store::create(&dir.path().join("a.qlvault"), &creds, kdf).expect("create");
    let vault = a.vault_by_name("Personal").expect("default vault").id;
    let bundle = a.join_bundle(&secret_key, &[vault]).expect("bundle");
    let (b, _) =
        Store::join(&dir.path().join("b.qlvault"), &bundle, "master".into(), kdf).expect("join");
    a.add_peer(Peer::new(
        b.device_id(),
        "B",
        b.static_key().public(),
        Timestamp::now(),
    ))
    .expect("peer");
    Pair { _dir: dir, a, b }
}

/// Adds a login titled `title` to the Personal vault.
pub fn add(s: &mut Session, title: &str) -> password365_model::ItemId {
    let vault = s.vault_by_name("Personal").expect("vault").id;
    s.add_item(
        vault,
        ItemKind::Login,
        ItemOverview::new(title).expect("title"),
        ItemDetails::default(),
    )
    .expect("add")
}

/// Titles of all items.
pub fn titles(s: &Session) -> Vec<String> {
    s.items().iter().map(|i| i.overview.title.clone()).collect()
}

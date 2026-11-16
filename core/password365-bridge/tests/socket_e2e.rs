//! End to end: real socket + real dispatcher + real (unlocked) vault.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use password365_app::secret_key_store::MemorySecretKeyStore;
use password365_app::{AppConfig, AppService};
use password365_bridge::client::BridgeClient;
use password365_bridge::dispatch::handle_bytes;
use password365_bridge::endpoint::Endpoint;
use password365_bridge::server::serve;
use password365_crypto::kdf::KdfParams;

#[test]
fn extension_requests_reach_the_vault() {
    let dir = tempfile::tempdir().unwrap();
    let config = AppConfig {
        store_path: dir.path().join("s"),
        kdf: KdfParams::insecure_for_tests(),
        auto_lock_ms: 0,
        settings_path: None,
    };
    let mut app = AppService::new(config, Box::new(MemorySecretKeyStore::default()));
    let pw = "long enough password";
    app.create_account(pw.into(), &pw.into()).unwrap();
    let app = Arc::new(Mutex::new(app));

    let endpoint = Endpoint::at(dir.path().join("b.sock"));
    let shared = Arc::clone(&app);
    serve(
        &endpoint,
        Arc::new(move |req: &[u8]| handle_bytes(&mut shared.lock().unwrap(), req)),
    )
    .unwrap();

    let mut client = BridgeClient::connect(&endpoint).unwrap();
    let save = br#"{"id":1,"type":"save_submitted","login":{"page_url":"https://shop.example/login","username":"me","password":"pw"}}"#;
    let saved: serde_json::Value = serde_json::from_slice(&client.exchange(save).unwrap()).unwrap();
    assert_eq!(saved["ok"], true, "{saved}");

    let list = br#"{"id":2,"type":"logins_for_url","page_url":"https://shop.example/"}"#;
    let listed: serde_json::Value =
        serde_json::from_slice(&client.exchange(list).unwrap()).unwrap();
    assert_eq!(listed["result"][0]["username"], "me");

    app.lock().unwrap().lock();
    let locked: serde_json::Value =
        serde_json::from_slice(&client.exchange(list).unwrap()).unwrap();
    assert_eq!(locked["error"]["code"], "locked");
}

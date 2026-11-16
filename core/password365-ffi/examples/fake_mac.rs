//! A stand-in "computer" for the iOS join UI test: an account with one item
//! that invites a device on 127.0.0.1, approves it as soon as it connects
//! (as if the user confirmed the six digits) and pins it. Prints
//! `CODE <invite>` and later `JOINED <name>`. With `FAKE_MAC_SYNC_FILE` set,
//! it then adds "Carried by file" and writes a sync file for the phone there
//! (`SYNCFILE <path>`). Not part of the app.

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};

use password365_app::item_input::ItemInput;
use password365_app::secret_key_store::MemorySecretKeyStore;
use password365_app::{AppConfig, AppService};
use password365_crypto::kdf::KdfParams;
use password365_net::tcp::SYNC_PORT;

fn main() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = AppConfig {
        store_path: dir.path().join("mac"),
        // The phone refuses weaker vaults than this (the security floor).
        kdf: KdfParams::MINIMUM,
        auto_lock_ms: 0,
        settings_path: None,
    };
    let password = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "correct horse battery staple".into());
    let mut mac = AppService::new(config, Box::new(MemorySecretKeyStore::default()));
    mac.create_account(password.as_str().into(), &password.as_str().into())
        .expect("account");
    mac.rename_this_device("Test Mac").expect("name");
    mac.create_item(&ItemInput {
        title: "From the Mac".into(),
        ..ItemInput::default()
    })
    .expect("item");
    let vault = mac.vaults().expect("vaults")[0].id.clone();
    let host = mac
        .start_pairing(IpAddr::V4(Ipv4Addr::LOCALHOST))
        .expect("invite");
    println!("CODE {}", host.invite_code());
    std::io::stdout().flush().ok();
    let mut pairing = host.accept().expect("phone connects");
    mac.send_pairing_bundle(&mut pairing, &[vault])
        .expect("approve");
    let peer = pairing.wait_joined(SYNC_PORT).expect("phone joins");
    let phone = mac.pin_device(peer).expect("pin");
    println!("JOINED {}", phone.name);
    if let Ok(path) = std::env::var("FAKE_MAC_SYNC_FILE") {
        mac.create_item(&ItemInput {
            title: "Carried by file".into(),
            ..ItemInput::default()
        })
        .expect("item");
        mac.export_sync_file(&phone.id, std::path::Path::new(&path))
            .expect("sync file");
        println!("SYNCFILE {path}");
    }
}

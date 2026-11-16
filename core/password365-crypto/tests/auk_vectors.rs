//! End-to-end key-derivation vectors (master password + Secret Key → AUK),
//! published in `tests/vectors/auk.json` for other implementations (mobile
//! apps, other languages): each must produce the same `k_pw`, `k_sk` and `auk`.
//! Regenerate with `cargo test -p password365-crypto --test auk_vectors -- --ignored`
//! only if the format changes (which would break every existing vault).
#![allow(clippy::unwrap_used, clippy::expect_used)] // a test: stop loudly

use password365_crypto::hkdf::derive_key;
use password365_crypto::kdf::{KdfParams, derive_password_key};
use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;
use password365_crypto::unlock::{UnlockMaterial, derive_account_unlock_key};
use serde_json::{Value, json};

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/auk.json");

struct Case {
    note: &'static str,
    password: &'static str,
    secret_key: [u8; 16],
    account_id: &'static [u8],
    salt: [u8; 32],
    params: KdfParams,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            note: "ASCII password, minimum parameters",
            password: "correct horse battery staple",
            secret_key: [0x11; 16],
            account_id: b"account-0001",
            salt: [0x22; 32],
            params: KdfParams::MINIMUM,
        },
        Case {
            note: "password with a precomposed é: it is NFKD-normalised before Argon2id",
            password: "caf\u{e9} pass\u{2460}",
            secret_key: *b"0123456789abcdef",
            account_id: b"account-0002",
            salt: [0x5a; 32],
            params: KdfParams::MINIMUM,
        },
        Case {
            note: "desktop parameters (256 MiB, 3 passes, 4 lanes)",
            password: "another long master password",
            secret_key: [0xfe; 16],
            account_id: &[0u8; 16],
            salt: [0x01; 32],
            params: KdfParams::DESKTOP,
        },
    ]
}

fn compute(case: &Case) -> Value {
    let password = SecretString::from(case.password);
    let secret_key = AccountSecretKey::from_bytes(case.secret_key);
    let k_pw = derive_password_key(&password, &case.salt, case.params).unwrap();
    let k_sk = derive_key(
        secret_key.expose(),
        case.account_id,
        b"quillock/v1/secret-key",
    )
    .unwrap();
    let auk = derive_account_unlock_key(&UnlockMaterial {
        password: &password,
        secret_key: &secret_key,
        account_id: case.account_id,
        salt: &case.salt,
        params: case.params,
    })
    .unwrap();
    json!({
        "note": case.note,
        "password_utf8": case.password,
        "secret_key_hex": hex::encode(case.secret_key),
        "secret_key_display": secret_key.to_display_string(),
        "account_id_hex": hex::encode(case.account_id),
        "salt_hex": hex::encode(case.salt),
        "argon2id": { "memory_kib": case.params.memory_kib, "iterations": case.params.iterations, "parallelism": case.params.parallelism, "version": "0x13", "output_len": 32 },
        "k_pw_hex": hex::encode(k_pw.expose()),
        "k_sk_hex": hex::encode(k_sk.expose()),
        "auk_hex": hex::encode(auk.expose()),
    })
}

fn document() -> Value {
    json!({
        "description": "Password365 account unlock key (AUK) derivation, format v1. See docs/crypto-spec.md §3.",
        "algorithm": [
            "k_pw = Argon2id(NFKD(password), salt, params), 32 bytes",
            "k_sk = HKDF-SHA256(ikm = secret_key, salt = account_id, info = \"quillock/v1/secret-key\"), 32 bytes",
            "AUK  = HKDF-SHA256(ikm = k_pw || k_sk, salt = salt, info = \"quillock/v1/auk\"), 32 bytes"
        ],
        "cases": cases().iter().map(compute).collect::<Vec<_>>(),
    })
}

#[test]
fn published_vectors_still_hold() {
    let published: Value =
        serde_json::from_str(&std::fs::read_to_string(FILE).expect("tests/vectors/auk.json"))
            .unwrap();
    assert_eq!(
        published,
        document(),
        "the derivation changed: every existing vault would stop opening"
    );
}

#[test]
#[ignore = "writes tests/vectors/auk.json"]
fn write_vectors() {
    std::fs::write(
        FILE,
        serde_json::to_string_pretty(&document()).unwrap() + "\n",
    )
    .unwrap();
}

//! Import files come from anywhere: no input may make a reader panic.
#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use password365_import::{bitwarden, csv_import, kdbx, onepux};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn csv_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = csv_import::parse(bytes.as_slice());
    }

    #[test]
    fn csv_with_known_headers_never_panics(rows in proptest::collection::vec("[^\\n]{0,40}", 0..8)) {
        let text = format!("name,url,username,password,notes,totp,tags,favorite\n{}", rows.join("\n"));
        let _ = csv_import::parse(text.as_bytes());
    }

    #[test]
    fn json_exports_never_panic(text in "\\PC{0,300}") {
        let _ = bitwarden::parse(&text);
        let _ = onepux::parse(&text);
    }

    #[test]
    fn json_shaped_exports_never_panic(kind in 0u64..10, name in "\\PC{0,20}", field in prop_oneof![Just("null"), Just("1"), Just("\"x\""), Just("{}"), Just("[]")]) {
        let bitwarden = format!(r#"{{"items":[{{"type":{kind},"name":{name:?},"login":{field},"card":{field},"identity":{field},"fields":[{{"name":{field},"value":{field}}}]}}]}}"#);
        let _ = bitwarden::parse(&bitwarden);
        let onepux = format!(r#"{{"accounts":[{{"vaults":[{{"items":[{{"categoryUuid":"00{kind}","overview":{field},"details":{{"sections":[{{"fields":[{{"value":{{"date":{field}}}}}]}}]}}}}]}}]}}]}}"#);
        let _ = onepux::parse(&onepux);
    }

    #[test]
    fn archives_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = onepux::read(Cursor::new(bytes.clone()));
        let _ = kdbx::read(&mut bytes.as_slice(), "password", None);
    }
}

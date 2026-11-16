//! A vault file may be damaged or replaced: opening it must fail cleanly.
#![allow(clippy::unwrap_used)]

use password365_vault::Store;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn damaged_vault_files_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        std::fs::write(&path, &bytes).unwrap();
        let _ = Store::open(&path);
    }
}

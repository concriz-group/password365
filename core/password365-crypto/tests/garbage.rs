//! Typed or pasted text: no input may make a parser panic.
use password365_crypto::secret_key::AccountSecretKey;
use password365_crypto::totp::Totp;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    #[test]
    fn secret_key_parsing_never_panics(text in "\\PC{0,80}") {
        let _ = AccountSecretKey::parse(&text);
    }

    #[test]
    fn secret_key_like_text_never_panics(text in "(P365|Q1)-[A-Z0-9 -]{0,60}") {
        let _ = AccountSecretKey::parse(&text);
    }

    #[test]
    fn totp_parsing_never_panics(text in "\\PC{0,120}") {
        let _ = Totp::parse(&text);
    }

    #[test]
    fn otpauth_like_text_never_panics(query in "[a-zA-Z0-9=&%._-]{0,80}") {
        let _ = Totp::parse(&format!("otpauth://totp/x?{query}"));
    }
}

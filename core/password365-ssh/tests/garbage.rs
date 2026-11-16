//! Any local program can talk to the agent: no message may make it panic.
use password365_ssh::{load_key, read_request};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    #[test]
    fn requests_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
        let mut reader = bytes.as_slice();
        while let Ok(Some(_)) = read_request(&mut reader) {}
    }

    #[test]
    fn sized_requests_never_panic(kind in any::<u8>(), body in proptest::collection::vec(any::<u8>(), 0..300)) {
        let mut message = u32::try_from(body.len() + 1).unwrap_or(u32::MAX).to_be_bytes().to_vec();
        message.push(kind);
        message.extend(body);
        let _ = read_request(&mut message.as_slice());
    }

    #[test]
    fn private_keys_never_panic(text in "\\PC{0,400}") {
        let _ = load_key("i", "t", &text, None);
        let _ = load_key("i", "t", &format!("-----BEGIN OPENSSH PRIVATE KEY-----\n{text}\n-----END OPENSSH PRIVATE KEY-----"), Some("x"));
    }
}

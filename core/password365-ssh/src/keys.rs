//! SSH keys from vault items: parsed when needed, never stored here.

use signature::Signer;
use ssh_encoding::Encode;
use ssh_key::{HashAlg, PrivateKey, Signature};

fn encoded(value: &impl Encode) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode(&mut out).map_or_else(|_| Vec::new(), |()| out)
}

/// Why a key cannot be used.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyError {
    /// Not an OpenSSH Ed25519 private key (another type, another format, or damaged).
    #[error("not an OpenSSH Ed25519 private key")]
    Unreadable,
    /// The key is protected by a passphrase that was not given or is wrong.
    #[error("the key is protected by a passphrase")]
    Encrypted,
    /// A key type the agent does not sign with (RSA, ECDSA, DSA …).
    #[error("only Ed25519 keys are supported")]
    Unsupported,
}

/// A usable key: what the agent lists and signs with.
pub struct AgentKey {
    /// Vault item id (for the approval prompt and remembering it).
    pub item_id: String,
    /// Item title (shown as the key's comment and in the prompt).
    pub title: String,
    private: PrivateKey,
}

impl std::fmt::Debug for AgentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentKey")
            .field("item_id", &self.item_id)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

/// Parses an item's private key (OpenSSH format), decrypting it with `passphrase` if needed.
pub fn load_key(
    item_id: &str,
    title: &str,
    pem: &str,
    passphrase: Option<&str>,
) -> Result<AgentKey, KeyError> {
    let mut private = PrivateKey::from_openssh(pem.trim()).map_err(|_| KeyError::Unreadable)?;
    if private.is_encrypted() {
        let passphrase = passphrase
            .filter(|p| !p.is_empty())
            .ok_or(KeyError::Encrypted)?;
        private = private
            .decrypt(passphrase)
            .map_err(|_| KeyError::Encrypted)?;
    }
    if private.algorithm() != ssh_key::Algorithm::Ed25519 {
        return Err(KeyError::Unsupported);
    }
    Ok(AgentKey {
        item_id: item_id.to_owned(),
        title: title.to_owned(),
        private,
    })
}

impl AgentKey {
    /// The public key in wire form (what clients send back to pick the key).
    #[must_use]
    pub fn public_blob(&self) -> Vec<u8> {
        encoded(self.private.public_key().key_data())
    }

    /// The public key as an `authorized_keys` line.
    #[must_use]
    pub fn public_openssh(&self) -> String {
        self.private.public_key().to_openssh().unwrap_or_default()
    }

    /// SHA-256 fingerprint (`SHA256:…`).
    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.private
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string()
    }

    /// Signs `data`; returns the signature blob.
    #[must_use]
    pub fn sign(&self, data: &[u8]) -> Option<Vec<u8>> {
        let signature: Signature = self.private.try_sign(data).ok()?;
        Some(encoded(&signature))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use rand_core::OsRng;
    use signature::Verifier;
    use ssh_encoding::Decode;
    use ssh_key::{Algorithm, LineEnding, PrivateKey, PublicKey};

    use super::*;

    pub(crate) fn ed25519_pem() -> String {
        PrivateKey::random(&mut OsRng, Algorithm::Ed25519)
            .unwrap()
            .to_openssh(LineEnding::LF)
            .unwrap()
            .to_string()
    }

    #[test]
    fn loads_and_signs_with_ed25519() {
        {
            let private = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
            let pem = private.to_openssh(LineEnding::LF).unwrap();
            let key = load_key("i1", "GitHub", &pem, None).unwrap();
            assert_eq!(key.public_blob(), encoded(private.public_key().key_data()));
            assert!(key.fingerprint().starts_with("SHA256:"));
            assert!(PublicKey::from_openssh(&key.public_openssh()).is_ok());
            let blob = key.sign(b"challenge").unwrap();
            let signature = Signature::decode(&mut blob.as_slice()).unwrap();
            let public = private.public_key().key_data();
            Verifier::verify(public, b"challenge", &signature).unwrap();
            assert!(Verifier::verify(public, b"other", &signature).is_err());
        }
    }

    #[test]
    fn refuses_unreadable_and_locked_keys() {
        assert_eq!(
            load_key("i", "t", "not a key", None).unwrap_err(),
            KeyError::Unreadable
        );
        let locked = PrivateKey::random(&mut OsRng, Algorithm::Ed25519)
            .unwrap()
            .encrypt(&mut OsRng, "secret")
            .unwrap()
            .to_openssh(LineEnding::LF)
            .unwrap();
        assert_eq!(
            load_key("i", "t", &locked, None).unwrap_err(),
            KeyError::Encrypted
        );
        assert_eq!(
            load_key("i", "t", &locked, Some("wrong")).unwrap_err(),
            KeyError::Encrypted
        );
        assert!(load_key("i", "t", &locked, Some("secret")).is_ok());
    }
}

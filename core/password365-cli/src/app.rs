//! Shared command context: paths, I/O and the unlock flow.

use std::io::Write;

use anyhow::Result;
use password365_app::password_policy::validate_new_master_password;
use password365_app::secret_key_store::{FileSecretKeyStore, SecretKeyStore};
use password365_crypto::kdf::KdfParams;
use password365_crypto::secret::SecretString;
use password365_crypto::secret_key::AccountSecretKey;
use password365_vault::{Credentials, Session, Store};

use crate::paths::Paths;
use crate::prompt::Prompter;

/// Everything a command needs.
pub struct App<'a> {
    /// File locations.
    pub paths: Paths,
    /// Argon2id parameters for new stores / password changes.
    pub kdf: KdfParams,
    /// User input.
    pub prompter: &'a mut dyn Prompter,
    /// Standard output.
    pub out: &'a mut dyn Write,
}

impl App<'_> {
    /// This device's Secret Key: from the key file, or typed by the user.
    pub fn secret_key(&mut self) -> Result<AccountSecretKey> {
        if let Some(key) = self.key_store().load()? {
            return Ok(key);
        }
        let typed = self.prompter.secret("Secret Key (P365-…)")?;
        Ok(AccountSecretKey::parse(typed.expose())?)
    }

    /// Asks for the master password and unlocks the store. Wrong passwords
    /// make the next try wait, like the app (the count is shared with it when
    /// both use the same folder).
    pub fn unlock(&mut self) -> Result<Session> {
        use password365_app::attempts;
        let store = Store::open(&self.paths.store)?;
        let store_path = self.paths.store.clone();
        let beside = Some(store_path.as_path());
        let mut tries = attempts::load(beside);
        tries.check(password365_model::Timestamp::now().as_millis())?;
        let secret_key = self.secret_key()?;
        let password = self.prompter.secret("Master password")?;
        let result = store.unlock(&Credentials {
            password,
            secret_key,
        });
        match &result {
            Err(password365_vault::VaultError::WrongCredentials) => {
                tries.failed(password365_model::Timestamp::now().as_millis());
                attempts::save(beside, &tries);
            }
            Ok(_) if tries.failures() > 0 => {
                tries.succeeded();
                attempts::save(beside, &tries);
            }
            _ => {}
        }
        Ok(result?)
    }

    /// Asks for a new master password twice and validates it.
    pub fn new_master_password(&mut self) -> Result<SecretString> {
        let password = self.prompter.secret("New master password")?;
        let again = self.prompter.secret("Repeat master password")?;
        validate_new_master_password(&password, &again)?;
        Ok(password)
    }

    /// Where this device's Secret Key is stored.
    pub fn key_store(&self) -> FileSecretKeyStore {
        FileSecretKeyStore::new(self.paths.secret_key.clone())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::prompt::ScriptedPrompter;

    /// Test fixture owning everything an `App` borrows.
    pub(crate) struct Fixture {
        pub dir: tempfile::TempDir,
        pub prompter: ScriptedPrompter,
        pub out: Vec<u8>,
    }

    pub(crate) const PASSWORD: &str = "correct horse battery";

    impl Fixture {
        pub(crate) fn new(answers: &[&str]) -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
                prompter: ScriptedPrompter::new(answers),
                out: Vec::new(),
            }
        }

        pub(crate) fn app(&mut self) -> App<'_> {
            App {
                paths: Paths {
                    store: self.dir.path().join("store.qlvault"),
                    secret_key: self.dir.path().join("secret-key"),
                },
                kdf: KdfParams::insecure_for_tests(),
                prompter: &mut self.prompter,
                out: &mut self.out,
            }
        }

        /// Replaces the scripted answers.
        pub(crate) fn answers(&mut self, answers: &[&str]) {
            self.prompter = ScriptedPrompter::new(answers);
        }

        /// Output written so far (and clears it).
        pub(crate) fn take_output(&mut self) -> String {
            String::from_utf8(std::mem::take(&mut self.out)).unwrap()
        }

        /// Creates an account with [`PASSWORD`].
        pub(crate) fn initialized(extra_answers: &[&str]) -> Self {
            let mut fx = Self::new(&[PASSWORD, PASSWORD]);
            crate::commands::account::init(&mut fx.app()).unwrap();
            fx.take_output();
            fx.answers(extra_answers);
            fx
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;

    #[test]
    fn new_password_must_be_long_enough() {
        let mut fx = Fixture::new(&["short", "short"]);
        let err = fx.app().new_master_password().unwrap_err();
        assert!(err.to_string().contains("at least"));
    }

    #[test]
    fn new_password_must_match() {
        let mut fx = Fixture::new(&["long enough pw", "different pw!!"]);
        assert!(
            fx.app()
                .new_master_password()
                .unwrap_err()
                .to_string()
                .contains("do not match")
        );
    }

    #[test]
    fn unlock_with_wrong_password_fails() {
        let mut fx = Fixture::initialized(&["wrong password"]);
        let err = fx.app().unlock().err().unwrap();
        assert!(err.to_string().contains("wrong master password"));
    }

    #[test]
    fn wrong_passwords_make_the_next_try_wait() {
        let mut fx = Fixture::initialized(&["wrong 1", "wrong 2", "wrong 3", PASSWORD]);
        for _ in 0..3 {
            assert!(
                fx.app()
                    .unlock()
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("wrong master password")
            );
        }
        let err = fx.app().unlock().err().unwrap();
        assert!(err.to_string().contains("try again"), "{err}");
    }

    #[test]
    fn unlock_prompts_for_secret_key_when_file_missing() {
        let mut fx = Fixture::initialized(&[]);
        let key = std::fs::read_to_string(fx.dir.path().join("secret-key")).unwrap();
        std::fs::remove_file(fx.dir.path().join("secret-key")).unwrap();
        fx.answers(&[key.trim(), PASSWORD]);
        assert!(fx.app().unlock().is_ok());
    }

    #[test]
    fn unlock_without_store_fails() {
        let mut fx = Fixture::new(&[PASSWORD]);
        assert!(fx.app().unlock().is_err());
    }
}

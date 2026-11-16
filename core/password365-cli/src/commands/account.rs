//! `init` and `passwd`.

use anyhow::{Result, bail};
use password365_crypto::secret_key::AccountSecretKey;
use password365_vault::{Credentials, Store};

use password365_app::secret_key_store::SecretKeyStore;

use crate::app::App;

/// Creates a new account: Secret Key file + encrypted store with a Personal vault.
pub fn init(app: &mut App) -> Result<()> {
    if app.paths.store.exists() {
        bail!("a store already exists at {}", app.paths.store.display());
    }
    let password = app.new_master_password()?;
    let secret_key = AccountSecretKey::generate()?;
    app.key_store().save(&secret_key)?;

    let credentials = Credentials {
        password,
        secret_key,
    };
    if let Err(e) = Store::create(&app.paths.store, &credentials, app.kdf) {
        // Do not leave an orphan key file behind.
        let _ = app.key_store().remove();
        return Err(e.into());
    }

    writeln!(app.out, "Account created.\n")?;
    writeln!(
        app.out,
        "Your Secret Key:\n\n    {}\n",
        credentials.secret_key.to_display_string()
    )?;
    writeln!(
        app.out,
        "Write it down and keep it with your master password in a safe place (Emergency Kit)."
    )?;
    writeln!(
        app.out,
        "Without BOTH you cannot open your vault on a new device — there is no server to recover it."
    )?;
    writeln!(app.out, "\nStore:      {}", app.paths.store.display())?;
    writeln!(app.out, "Secret Key: {}", app.paths.secret_key.display())?;
    Ok(())
}

/// Changes the master password (the Secret Key stays the same).
pub fn passwd(app: &mut App) -> Result<()> {
    let mut session = app.unlock()?;
    let password = app.new_master_password()?;
    let secret_key = app.secret_key()?;
    session.change_credentials(
        &Credentials {
            password,
            secret_key,
        },
        app.kdf,
    )?;
    writeln!(app.out, "Master password changed.")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};

    #[test]
    fn init_creates_store_and_key_and_prints_secret_key() {
        let mut fx = Fixture::new(&[PASSWORD, PASSWORD]);
        init(&mut fx.app()).unwrap();
        let out = fx.take_output();
        assert!(out.contains("Your Secret Key:"));
        assert!(out.contains("P365-"));
        assert!(fx.dir.path().join("store.qlvault").is_file());
        assert!(fx.dir.path().join("secret-key").is_file());
    }

    #[test]
    fn init_refuses_existing_store() {
        let mut fx = Fixture::initialized(&[PASSWORD, PASSWORD]);
        assert!(
            init(&mut fx.app())
                .unwrap_err()
                .to_string()
                .contains("already exists")
        );
    }

    #[test]
    fn init_with_bad_password_creates_nothing() {
        let mut fx = Fixture::new(&["short", "short"]);
        assert!(init(&mut fx.app()).is_err());
        assert!(!fx.dir.path().join("secret-key").exists());
    }

    #[test]
    fn init_removes_key_file_when_store_creation_fails() {
        let mut fx = Fixture::new(&[PASSWORD, PASSWORD]);
        // A folder inside a file: impossible on every system.
        let blocker = fx.dir.path().join("a-file");
        std::fs::write(&blocker, b"x").unwrap();
        let mut app = fx.app();
        app.paths.store = blocker.join("x").join("store");
        assert!(init(&mut app).is_err());
        assert!(!fx.dir.path().join("secret-key").exists());
    }

    #[test]
    fn passwd_changes_the_password() {
        let new = "a brand new password";
        let mut fx = Fixture::initialized(&[PASSWORD, new, new]);
        passwd(&mut fx.app()).unwrap();
        assert!(fx.take_output().contains("changed"));
        fx.answers(&[PASSWORD]);
        assert!(fx.app().unlock().is_err());
        fx.answers(&[new]);
        assert!(fx.app().unlock().is_ok());
    }
}

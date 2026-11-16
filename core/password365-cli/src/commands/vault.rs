//! `vault list|create|rename|delete`.

use anyhow::Result;

use crate::app::App;
use crate::cli::VaultCommand;
use crate::resolve;

/// Runs a vault subcommand.
pub fn run(app: &mut App, command: &VaultCommand) -> Result<()> {
    let mut session = app.unlock()?;
    match command {
        VaultCommand::List => {
            for vault in session.vaults() {
                let count = session.items_in(vault.id).len();
                writeln!(app.out, "{:<32}{count} items", vault.name)?;
            }
        }
        VaultCommand::Create { name } => {
            session.create_vault(name)?;
            writeln!(app.out, "Vault '{}' created.", name.trim())?;
        }
        VaultCommand::Rename { name, new_name } => {
            let id = resolve::vault(&session, Some(name))?;
            session.rename_vault(id, new_name)?;
            writeln!(app.out, "Vault renamed to '{}'.", new_name.trim())?;
        }
        VaultCommand::Delete { name } => {
            let id = resolve::vault(&session, Some(name))?;
            session.delete_vault(id)?;
            writeln!(app.out, "Vault '{name}' deleted.")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};

    fn exec(fx: &mut Fixture, command: &VaultCommand) -> Result<String> {
        fx.answers(&[PASSWORD]);
        run(&mut fx.app(), command)?;
        Ok(fx.take_output())
    }

    #[test]
    fn full_vault_lifecycle() {
        let mut fx = Fixture::initialized(&[]);
        assert!(
            exec(
                &mut fx,
                &VaultCommand::Create {
                    name: "Work".into()
                }
            )
            .unwrap()
            .contains("created")
        );
        let list = exec(&mut fx, &VaultCommand::List).unwrap();
        assert!(list.contains("Personal") && list.contains("Work"));
        exec(
            &mut fx,
            &VaultCommand::Rename {
                name: "work".into(),
                new_name: "Job".into(),
            },
        )
        .unwrap();
        assert!(exec(&mut fx, &VaultCommand::List).unwrap().contains("Job"));
        assert!(
            exec(&mut fx, &VaultCommand::Delete { name: "Job".into() })
                .unwrap()
                .contains("deleted")
        );
        assert!(!exec(&mut fx, &VaultCommand::List).unwrap().contains("Job"));
    }

    #[test]
    fn unknown_vault_errors() {
        let mut fx = Fixture::initialized(&[]);
        assert!(
            exec(
                &mut fx,
                &VaultCommand::Delete {
                    name: "nope".into()
                }
            )
            .is_err()
        );
        assert!(
            exec(
                &mut fx,
                &VaultCommand::Rename {
                    name: "nope".into(),
                    new_name: "x".into()
                }
            )
            .is_err()
        );
    }
}

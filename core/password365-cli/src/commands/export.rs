//! `export <file.csv>` — Chrome-compatible plaintext export.

use anyhow::{Context, Result, bail};
use password365_app::transfer;
use password365_vault::private_file::create_private_file;

use crate::app::App;
use crate::cli::ExportArgs;
use crate::resolve;

/// Exports logins to a new owner-only CSV file after an explicit warning.
pub fn run(app: &mut App, args: &ExportArgs) -> Result<()> {
    if args.file.exists() {
        bail!(
            "{} already exists — refusing to overwrite",
            args.file.display()
        );
    }
    let session = app.unlock()?;
    let vault_id = args
        .vault
        .as_deref()
        .map(|v| resolve::vault(&session, Some(v)))
        .transpose()?;

    if !args.yes
        && !app
            .prompter
            .confirm("The export will contain ALL selected passwords in PLAINTEXT. Continue?")?
    {
        bail!("cancelled");
    }

    let file = create_private_file(&args.file)
        .with_context(|| format!("cannot create {}", args.file.display()))?;
    let count = transfer::export_csv(&session, file, vault_id)?;
    writeln!(
        app.out,
        "Exported {count} logins to {}.",
        args.file.display()
    )?;
    writeln!(
        app.out,
        "⚠ Import it into Chrome, then delete the file securely. Do not open it in a spreadsheet app."
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};
    use crate::cli::ItemCommand;

    fn seed(fx: &mut Fixture) {
        fx.answers(&[PASSWORD, "pw-123", "pw-123"]);
        let add = ItemCommand::Add(crate::cli::AddItemArgs {
            title: "Site".into(),
            kind: "login".into(),
            vault: None,
            username: Some("me".into()),
            urls: vec!["https://site.com".into()],
            notes: None,
            tags: vec![],
            generate: false,
        });
        crate::commands::item::run(&mut fx.app(), &add).unwrap();
        fx.take_output();
    }

    fn target(fx: &Fixture) -> PathBuf {
        fx.dir.path().join("out.csv")
    }

    #[test]
    fn exports_after_confirmation() {
        let mut fx = Fixture::initialized(&[]);
        seed(&mut fx);
        fx.answers(&[PASSWORD, "yes"]);
        let file = target(&fx);
        run(
            &mut fx.app(),
            &ExportArgs {
                file: file.clone(),
                vault: None,
                yes: false,
            },
        )
        .unwrap();
        assert!(fx.take_output().contains("Exported 1 logins"));
        let text = std::fs::read_to_string(file).unwrap();
        assert!(text.contains("Site,https://site.com,me,pw-123,"));
    }

    #[test]
    fn declining_writes_nothing() {
        let mut fx = Fixture::initialized(&[PASSWORD, "no"]);
        let file = target(&fx);
        assert!(
            run(
                &mut fx.app(),
                &ExportArgs {
                    file: file.clone(),
                    vault: None,
                    yes: false
                }
            )
            .is_err()
        );
        assert!(!file.exists());
    }

    #[test]
    fn refuses_existing_file_and_unknown_vault() {
        let mut fx = Fixture::initialized(&[PASSWORD]);
        let file = target(&fx);
        let args = ExportArgs {
            file: file.clone(),
            vault: Some("Nope".into()),
            yes: true,
        };
        assert!(run(&mut fx.app(), &args).is_err());
        std::fs::write(&file, "x").unwrap();
        let args = ExportArgs {
            file,
            vault: None,
            yes: true,
        };
        assert!(
            run(&mut fx.app(), &args)
                .unwrap_err()
                .to_string()
                .contains("already exists")
        );
    }

    #[test]
    fn vault_filter_limits_export() {
        let mut fx = Fixture::initialized(&[]);
        seed(&mut fx);
        fx.answers(&[PASSWORD]);
        crate::commands::vault::run(
            &mut fx.app(),
            &crate::cli::VaultCommand::Create {
                name: "Empty".into(),
            },
        )
        .unwrap();
        fx.answers(&[PASSWORD]);
        let file = target(&fx);
        run(
            &mut fx.app(),
            &ExportArgs {
                file,
                vault: Some("Empty".into()),
                yes: true,
            },
        )
        .unwrap();
        assert!(fx.take_output().contains("Exported 0 logins"));
    }
}

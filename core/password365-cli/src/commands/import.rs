//! `import <file.csv>` — browser CSV import with duplicate detection.

use std::fs::File;

use anyhow::{Context, Result};
use password365_app::transfer;
use password365_model::Timestamp;

use crate::app::App;
use crate::cli::ImportArgs;
use crate::resolve;

/// Imports logins from a CSV file into a vault.
pub fn run(app: &mut App, args: &ImportArgs) -> Result<()> {
    let file =
        File::open(&args.file).with_context(|| format!("cannot open {}", args.file.display()))?;
    let mut session = app.unlock()?;
    let vault_id = resolve::vault(&session, args.vault.as_deref())?;
    let summary =
        transfer::import_csv(&mut session, file, vault_id, args.update, Timestamp::now())?;

    writeln!(app.out, "Imported:            {}", summary.imported)?;
    writeln!(app.out, "Already up to date:  {}", summary.identical)?;
    if args.update {
        writeln!(app.out, "Updated passwords:   {}", summary.updated)?;
    } else {
        writeln!(
            app.out,
            "Different password:  {} (re-run with --update to overwrite)",
            summary.changed
        )?;
    }
    writeln!(
        app.out,
        "Duplicates in file:  {}",
        summary.duplicates_in_file
    )?;
    for (line, reason) in &summary.skipped {
        writeln!(app.out, "Skipped line {line}: {reason}")?;
    }
    writeln!(
        app.out,
        "\n⚠ {} contains your passwords in PLAINTEXT. Delete it securely now.",
        args.file.display()
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};

    fn write_csv(fx: &Fixture, body: &str) -> PathBuf {
        let path = fx.dir.path().join("chrome.csv");
        std::fs::write(&path, format!("name,url,username,password,note\n{body}")).unwrap();
        path
    }

    fn import(fx: &mut Fixture, file: PathBuf, update: bool) -> Result<String> {
        fx.answers(&[PASSWORD]);
        run(
            &mut fx.app(),
            &ImportArgs {
                file,
                vault: None,
                update,
            },
        )?;
        Ok(fx.take_output())
    }

    #[test]
    fn imports_then_detects_identical_and_changed() {
        let mut fx = Fixture::initialized(&[]);
        let csv = write_csv(
            &fx,
            "GitHub,https://github.com,octo,pw1,\nMail,https://mail.com,me,pw2,\n,,,\n",
        );
        let out = import(&mut fx, csv, false).unwrap();
        assert!(out.contains("Imported:            2"));
        assert!(out.contains("Skipped line 4"));
        assert!(out.contains("PLAINTEXT"));

        let csv = write_csv(
            &fx,
            "GitHub,https://github.com,octo,pw1,\nMail,https://mail.com,me,CHANGED,\n",
        );
        let out = import(&mut fx, csv.clone(), false).unwrap();
        assert!(out.contains("Imported:            0"));
        assert!(out.contains("Already up to date:  1"));
        assert!(out.contains("Different password:  1"));

        let out = import(&mut fx, csv, true).unwrap();
        assert!(out.contains("Updated passwords:   1"));
    }

    #[test]
    fn missing_file_errors_before_unlock() {
        let mut fx = Fixture::initialized(&[]);
        let err = import(&mut fx, "/nonexistent.csv".into(), false).unwrap_err();
        assert!(err.to_string().contains("cannot open"));
    }

    #[test]
    fn unknown_vault_errors() {
        let mut fx = Fixture::initialized(&[PASSWORD]);
        let csv = write_csv(&fx, "a,https://a.com,u,p,\n");
        let args = ImportArgs {
            file: csv,
            vault: Some("Nope".into()),
            update: false,
        };
        assert!(run(&mut fx.app(), &args).is_err());
    }
}

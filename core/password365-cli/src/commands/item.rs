//! `item list|add|show|set-password|rm`.

use anyhow::{Result, bail};
use password365_crypto::secret::SecretString;
use password365_model::{ItemDetails, ItemKind, ItemOverview, Timestamp};
use password365_vault::Session;

use crate::app::App;
use crate::cli::{AddItemArgs, ItemCommand};
use crate::commands::generate::default_password;
use crate::{output, resolve};

/// Runs an item subcommand.
pub fn run(app: &mut App, command: &ItemCommand) -> Result<()> {
    let mut session = app.unlock()?;
    match command {
        ItemCommand::List { vault, search } => {
            list(app, &session, vault.as_deref(), search.as_deref())
        }
        ItemCommand::Add(args) => add(app, &mut session, args),
        ItemCommand::Show { item, reveal } => show(app, &session, item, *reveal),
        ItemCommand::SetPassword { item, generate } => {
            set_password(app, &mut session, item, *generate)
        }
        ItemCommand::Rm { item, yes } => remove(app, &mut session, item, *yes),
    }
}

fn list(app: &mut App, session: &Session, vault: Option<&str>, search: Option<&str>) -> Result<()> {
    let vault_id = vault
        .map(|name| resolve::vault(session, Some(name)))
        .transpose()?;
    let items: Vec<_> = session
        .search(search.unwrap_or(""))
        .into_iter()
        .filter(|s| vault_id.is_none_or(|v| s.vault_id == v))
        .collect();
    output::item_table(app.out, session, &items)?;
    Ok(())
}

fn add(app: &mut App, session: &mut Session, args: &AddItemArgs) -> Result<()> {
    let kind: ItemKind = args.kind.parse().map_err(anyhow::Error::msg)?;
    let vault_id = resolve::vault(session, args.vault.as_deref())?;
    let mut overview =
        ItemOverview::new(&args.title)?.with_username(args.username.as_deref().unwrap_or(""));
    for url in &args.urls {
        overview = overview.with_url(url);
    }
    for tag in &args.tags {
        overview = overview.with_tag(tag);
    }
    let password = match kind {
        ItemKind::SecureNote => SecretString::default(),
        _ => obtain_password(app, args.generate)?,
    };
    let details =
        ItemDetails::with_password(password).notes(args.notes.as_deref().unwrap_or("").into());
    let id = session.add_item(vault_id, kind, overview, details)?;
    writeln!(app.out, "Item created: {id}")?;
    Ok(())
}

fn show(app: &mut App, session: &Session, query: &str, reveal: bool) -> Result<()> {
    let item = session.item(resolve::item(session, query)?)?;
    let vault = session.vault(item.vault_id)?.name.clone();
    output::item_detail(app.out, &item, &vault, reveal)?;
    Ok(())
}

fn set_password(app: &mut App, session: &mut Session, query: &str, generate: bool) -> Result<()> {
    let mut item = session.item(resolve::item(session, query)?)?;
    let password = obtain_password(app, generate)?;
    if !item.set_password(Some(password), Timestamp::now()) {
        writeln!(app.out, "Password unchanged.")?;
        return Ok(());
    }
    session.save_item(&item)?;
    writeln!(app.out, "Password updated (previous one kept in history).")?;
    Ok(())
}

fn remove(app: &mut App, session: &mut Session, query: &str, yes: bool) -> Result<()> {
    let id = resolve::item(session, query)?;
    let title = session.item(id)?.overview.title;
    if !yes
        && !app
            .prompter
            .confirm(&format!("Delete '{title}' permanently?"))?
    {
        bail!("cancelled");
    }
    session.delete_item(id)?;
    writeln!(app.out, "Deleted '{title}'.")?;
    Ok(())
}

/// Generated password, or one typed twice by the user (empty = none).
fn obtain_password(app: &mut App, generate: bool) -> Result<SecretString> {
    if generate {
        return default_password();
    }
    let password = app.prompter.secret("Password (empty for none)")?;
    if !password.is_empty() && app.prompter.secret("Repeat password")? != password {
        bail!("passwords do not match");
    }
    Ok(password)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};
    use crate::cli::{Cli, Command};

    fn command(argv: &[&str]) -> ItemCommand {
        let mut full = vec!["password365", "item"];
        full.extend_from_slice(argv);
        match Cli::parse_from(full).command {
            Command::Item(c) => c,
            _ => unreachable!(),
        }
    }

    fn exec(fx: &mut Fixture, answers: &[&str], argv: &[&str]) -> Result<String> {
        let mut all = vec![PASSWORD];
        all.extend_from_slice(answers);
        fx.answers(&all);
        run(&mut fx.app(), &command(argv))?;
        Ok(fx.take_output())
    }

    #[test]
    fn add_list_show_flow() {
        let mut fx = Fixture::initialized(&[]);
        let out = exec(
            &mut fx,
            &["s3cret-pw", "s3cret-pw"],
            &[
                "add",
                "--title",
                "GitHub",
                "--username",
                "octo",
                "--url",
                "https://github.com",
                "--tag",
                "dev",
                "--notes",
                "n",
            ],
        )
        .unwrap();
        assert!(out.contains("Item created"));

        let list = exec(&mut fx, &[], &["list"]).unwrap();
        assert!(list.contains("GitHub") && list.contains("octo") && list.contains("Personal"));
        assert!(
            exec(&mut fx, &[], &["list", "--search", "nothing"])
                .unwrap()
                .contains("No items")
        );
        assert!(
            exec(&mut fx, &[], &["list", "--vault", "Personal"])
                .unwrap()
                .contains("GitHub")
        );
        assert!(exec(&mut fx, &[], &["list", "--vault", "Missing"]).is_err());

        let hidden = exec(&mut fx, &[], &["show", "github"]).unwrap();
        assert!(!hidden.contains("s3cret-pw"));
        assert!(
            exec(&mut fx, &[], &["show", "github", "--reveal"])
                .unwrap()
                .contains("s3cret-pw")
        );
    }

    #[test]
    fn add_with_generated_password_and_note_kind() {
        let mut fx = Fixture::initialized(&[]);
        exec(&mut fx, &[], &["add", "--title", "Gen", "--generate"]).unwrap();
        let shown = exec(&mut fx, &[], &["show", "Gen", "--reveal"]).unwrap();
        let pw_line = shown.lines().find(|l| l.starts_with("Password:")).unwrap();
        assert_eq!(
            pw_line
                .trim_start_matches("Password:")
                .trim()
                .chars()
                .count(),
            20
        );

        exec(
            &mut fx,
            &[],
            &["add", "--title", "Memo", "--kind", "note", "--notes", "hi"],
        )
        .unwrap();
        assert!(
            !exec(&mut fx, &[], &["show", "Memo"])
                .unwrap()
                .contains("Password:")
        );
    }

    #[test]
    fn add_rejects_bad_input() {
        let mut fx = Fixture::initialized(&[]);
        assert!(
            exec(
                &mut fx,
                &[],
                &["add", "--title", "x", "--kind", "spaceship"]
            )
            .is_err()
        );
        assert!(
            exec(&mut fx, &["a", "b"], &["add", "--title", "x"]).is_err(),
            "mismatch"
        );
        assert!(
            exec(
                &mut fx,
                &[],
                &["add", "--title", "x", "--vault", "nope", "--generate"]
            )
            .is_err()
        );
    }

    #[test]
    fn add_without_password() {
        let mut fx = Fixture::initialized(&[]);
        exec(&mut fx, &[""], &["add", "--title", "NoPw"]).unwrap();
        assert!(
            !exec(&mut fx, &[], &["show", "NoPw"])
                .unwrap()
                .contains("Password:")
        );
    }

    #[test]
    fn set_password_keeps_history() {
        let mut fx = Fixture::initialized(&[]);
        exec(&mut fx, &["old-pw", "old-pw"], &["add", "--title", "Bank"]).unwrap();
        assert!(
            exec(&mut fx, &["old-pw", "old-pw"], &["set-password", "Bank"])
                .unwrap()
                .contains("unchanged")
        );
        assert!(
            exec(&mut fx, &[], &["set-password", "Bank", "--generate"])
                .unwrap()
                .contains("updated")
        );
        assert!(
            exec(&mut fx, &[], &["show", "Bank"])
                .unwrap()
                .contains("1 previous passwords")
        );
    }

    #[test]
    fn remove_with_and_without_confirmation() {
        let mut fx = Fixture::initialized(&[]);
        exec(&mut fx, &[], &["add", "--title", "Tmp", "--generate"]).unwrap();
        assert!(
            exec(&mut fx, &["n"], &["rm", "Tmp"])
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        assert!(
            exec(&mut fx, &["y"], &["rm", "Tmp"])
                .unwrap()
                .contains("Deleted")
        );
        exec(&mut fx, &[], &["add", "--title", "Tmp2", "--generate"]).unwrap();
        assert!(
            exec(&mut fx, &[], &["rm", "Tmp2", "--yes"])
                .unwrap()
                .contains("Deleted")
        );
        assert!(exec(&mut fx, &[], &["show", "Tmp2"]).is_err());
    }
}

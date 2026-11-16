//! One module per top-level command.

pub mod account;
pub mod export;
pub mod generate;
pub mod import;
pub mod item;
pub mod vault;

use anyhow::Result;

use crate::app::App;
use crate::cli::Command;

/// Dispatches a parsed command.
pub fn dispatch(app: &mut App, command: &Command) -> Result<()> {
    match command {
        Command::Init => account::init(app),
        Command::Passwd => account::passwd(app),
        Command::Vault(c) => vault::run(app, c),
        Command::Item(c) => item::run(app, c),
        Command::Generate(a) => generate::run(app, a),
        Command::Import(a) => import::run(app, a),
        Command::Export(a) => export::run(app, a),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};
    use crate::cli::Cli;

    fn run(fx: &mut Fixture, answers: &[&str], argv: &[&str]) -> Result<String> {
        fx.answers(answers);
        let mut full = vec!["password365"];
        full.extend_from_slice(argv);
        dispatch(&mut fx.app(), &Cli::parse_from(full).command)?;
        Ok(fx.take_output())
    }

    #[test]
    fn dispatches_every_command() {
        let mut fx = Fixture::new(&[]);
        assert!(
            run(&mut fx, &[PASSWORD, PASSWORD], &["init"])
                .unwrap()
                .contains("Secret Key")
        );
        assert!(run(&mut fx, &[], &["generate", "-q"]).is_ok());
        assert!(
            run(&mut fx, &[PASSWORD], &["vault", "list"])
                .unwrap()
                .contains("Personal")
        );
        assert!(
            run(&mut fx, &[PASSWORD], &["item", "list"])
                .unwrap()
                .contains("No items")
        );
        let csv = fx.dir.path().join("in.csv");
        std::fs::write(&csv, "name,url,username,password\na,https://a.com,u,p\n").unwrap();
        assert!(
            run(&mut fx, &[PASSWORD], &["import", csv.to_str().unwrap()])
                .unwrap()
                .contains("Imported")
        );
        let out = fx.dir.path().join("out.csv");
        assert!(
            run(
                &mut fx,
                &[PASSWORD],
                &["export", out.to_str().unwrap(), "--yes"]
            )
            .unwrap()
            .contains("Exported 1")
        );
        let new = "another long password";
        assert!(
            run(&mut fx, &[PASSWORD, new, new], &["passwd"])
                .unwrap()
                .contains("changed")
        );
    }
}

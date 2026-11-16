//! `password365` — command-line interface for the Password365 password manager.

mod app;
mod cli;
mod commands;
mod output;
mod paths;
mod prompt;
mod resolve;

use std::io;
use std::process::ExitCode;

use clap::Parser;
use password365_crypto::kdf::KdfParams;

use crate::app::App;
use crate::cli::Cli;
use crate::paths::Paths;
use crate::prompt::TerminalPrompter;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let paths = Paths::resolve(cli.store, cli.secret_key_file)?;
    let mut prompter = TerminalPrompter {
        password_stdin: cli.password_stdin,
    };
    let mut stdout = io::stdout().lock();
    let mut app = App {
        paths,
        kdf: KdfParams::DESKTOP,
        prompter: &mut prompter,
        out: &mut stdout,
    };
    commands::dispatch(&mut app, &cli.command)
}

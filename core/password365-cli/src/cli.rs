//! Command-line argument definitions.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Password365 — offline-first password manager.
#[derive(Debug, Parser)]
#[command(name = "password365", version, about)]
pub struct Cli {
    /// Path of the vault store file.
    #[arg(long, global = true, env = "PASSWORD365_STORE")]
    pub store: Option<PathBuf>,

    /// Path of the file holding this device's Secret Key.
    #[arg(long, global = true, env = "PASSWORD365_SECRET_KEY_FILE")]
    pub secret_key_file: Option<PathBuf>,

    /// Read passwords from stdin, one per line, instead of the terminal (for scripts).
    #[arg(long, global = true)]
    pub password_stdin: bool,

    /// Command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new account (store + Secret Key).
    Init,
    /// Change the master password.
    Passwd,
    /// Manage vaults.
    #[command(subcommand)]
    Vault(VaultCommand),
    /// Manage items.
    #[command(subcommand)]
    Item(ItemCommand),
    /// Generate a password, passphrase or PIN (no unlock needed).
    Generate(GenerateArgs),
    /// Import passwords from a browser CSV export (Chrome, Edge, Firefox, Safari …).
    Import(ImportArgs),
    /// Export passwords to a Chrome-compatible CSV (PLAINTEXT!).
    Export(ExportArgs),
}

/// Vault subcommands.
#[derive(Debug, Subcommand)]
pub enum VaultCommand {
    /// List vaults.
    List,
    /// Create a vault.
    Create {
        /// Vault name.
        name: String,
    },
    /// Rename a vault.
    Rename {
        /// Current name.
        name: String,
        /// New name.
        new_name: String,
    },
    /// Delete an empty vault.
    Delete {
        /// Vault name.
        name: String,
    },
}

/// Item subcommands.
#[derive(Debug, Subcommand)]
pub enum ItemCommand {
    /// List items.
    List {
        /// Only items in this vault.
        #[arg(long)]
        vault: Option<String>,
        /// Only items matching this text.
        #[arg(long)]
        search: Option<String>,
    },
    /// Add an item.
    Add(AddItemArgs),
    /// Show an item.
    Show {
        /// Item id (or unique prefix) or exact title.
        item: String,
        /// Print secrets in clear text.
        #[arg(long)]
        reveal: bool,
    },
    /// Replace an item's password (old one kept in history).
    SetPassword {
        /// Item id (or unique prefix) or exact title.
        item: String,
        /// Generate a strong password instead of prompting.
        #[arg(long)]
        generate: bool,
    },
    /// Delete an item.
    Rm {
        /// Item id (or unique prefix) or exact title.
        item: String,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// Arguments for `item add`.
#[derive(Debug, Args)]
pub struct AddItemArgs {
    /// Title.
    #[arg(long)]
    pub title: String,
    /// Kind: login, password or note.
    #[arg(long, default_value = "login")]
    pub kind: String,
    /// Vault name (default: Personal).
    #[arg(long)]
    pub vault: Option<String>,
    /// Username.
    #[arg(long)]
    pub username: Option<String>,
    /// Website URL (repeatable).
    #[arg(long = "url")]
    pub urls: Vec<String>,
    /// Ask for notes (typed at a prompt: never on the command line, where
    /// other programs could read them in the process list).
    #[arg(long)]
    pub notes: bool,
    /// Tag (repeatable).
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Generate a strong password instead of prompting.
    #[arg(long)]
    pub generate: bool,
}

/// Arguments for `generate`.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)]
pub struct GenerateArgs {
    /// Password length.
    #[arg(long, default_value_t = 20)]
    pub length: usize,
    /// No symbols.
    #[arg(long)]
    pub no_symbols: bool,
    /// No digits.
    #[arg(long)]
    pub no_digits: bool,
    /// No uppercase letters.
    #[arg(long)]
    pub no_uppercase: bool,
    /// Avoid look-alike characters (l, 1, O, 0 …).
    #[arg(long)]
    pub exclude_ambiguous: bool,
    /// Generate a memorable passphrase instead.
    #[arg(long, conflicts_with = "pin")]
    pub passphrase: bool,
    /// Words in the passphrase.
    #[arg(long, default_value_t = 5)]
    pub words: usize,
    /// Passphrase separator.
    #[arg(long, default_value = "-")]
    pub separator: String,
    /// Generate a numeric PIN of this many digits.
    #[arg(long)]
    pub pin: Option<usize>,
    /// Print only the secret.
    #[arg(long, short)]
    pub quiet: bool,
}

/// Arguments for `import`.
#[derive(Debug, Args)]
pub struct ImportArgs {
    /// CSV file exported from the browser.
    pub file: PathBuf,
    /// Target vault (default: Personal).
    #[arg(long)]
    pub vault: Option<String>,
    /// Also update existing logins whose password differs.
    #[arg(long)]
    pub update: bool,
}

/// Arguments for `export`.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Output CSV file (must not exist).
    pub file: PathBuf,
    /// Only this vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Skip the plaintext warning prompt.
    #[arg(long)]
    pub yes: bool,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_nested_commands() {
        let cli = Cli::parse_from([
            "password365",
            "--store",
            "/tmp/x",
            "item",
            "show",
            "abc",
            "--reveal",
        ]);
        assert_eq!(cli.store, Some(PathBuf::from("/tmp/x")));
        assert!(matches!(
            cli.command,
            Command::Item(ItemCommand::Show { reveal: true, .. })
        ));
    }

    #[test]
    fn passphrase_and_pin_conflict() {
        assert!(
            Cli::try_parse_from(["password365", "generate", "--passphrase", "--pin", "4"]).is_err()
        );
    }
}

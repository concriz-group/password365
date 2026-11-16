//! `generate` — passwords, passphrases and PINs.

use anyhow::Result;
use password365_crypto::secret::SecretString;
use password365_generator::charset::CharClass;
use password365_generator::entropy::strength_label;
use password365_generator::{PassphraseRecipe, PasswordRecipe};

use crate::app::App;
use crate::cli::GenerateArgs;

/// A secret plus its estimated entropy.
pub struct Generated {
    /// The generated secret.
    pub secret: SecretString,
    /// Estimated entropy in bits.
    pub bits: f64,
}

/// Generates according to the CLI arguments.
pub fn generate(args: &GenerateArgs) -> Result<Generated> {
    if let Some(digits) = args.pin {
        return from_password(&PasswordRecipe::pin(digits));
    }
    if args.passphrase {
        let recipe = PassphraseRecipe {
            words: args.words,
            separator: args.separator.clone(),
            ..PassphraseRecipe::default()
        };
        return Ok(Generated {
            secret: recipe.generate()?,
            bits: recipe.entropy_bits(),
        });
    }
    let classes = CharClass::ALL
        .into_iter()
        .filter(|c| match c {
            CharClass::Lowercase => true,
            CharClass::Uppercase => !args.no_uppercase,
            CharClass::Digits => !args.no_digits,
            CharClass::Symbols => !args.no_symbols,
        })
        .collect();
    from_password(&PasswordRecipe {
        length: args.length,
        classes,
        exclude_ambiguous: args.exclude_ambiguous,
    })
}

/// Default strong password used by `item add --generate` etc.
pub fn default_password() -> Result<SecretString> {
    Ok(PasswordRecipe::default().generate()?)
}

fn from_password(recipe: &PasswordRecipe) -> Result<Generated> {
    Ok(Generated {
        secret: recipe.generate()?,
        bits: recipe.entropy_bits(),
    })
}

/// Runs `generate` and prints the result.
pub fn run(app: &mut App, args: &GenerateArgs) -> Result<()> {
    let generated = generate(args)?;
    writeln!(app.out, "{}", generated.secret.expose())?;
    if !args.quiet {
        writeln!(
            app.out,
            "~{:.0} bits of entropy ({})",
            generated.bits,
            strength_label(generated.bits)
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::app::test_support::Fixture;
    use crate::cli::{Cli, Command};

    fn args(extra: &[&str]) -> GenerateArgs {
        let mut argv = vec!["password365", "generate"];
        argv.extend_from_slice(extra);
        match Cli::parse_from(argv).command {
            Command::Generate(a) => a,
            _ => unreachable!(),
        }
    }

    #[test]
    fn default_password() {
        let g = generate(&args(&[])).unwrap();
        assert_eq!(g.secret.expose().chars().count(), 20);
        assert!(g.bits > 120.0);
    }

    #[test]
    fn restricted_classes() {
        let g = generate(&args(&[
            "--length",
            "30",
            "--no-symbols",
            "--no-digits",
            "--no-uppercase",
        ]))
        .unwrap();
        assert!(g.secret.expose().chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn passphrase_and_pin() {
        let p = generate(&args(&["--passphrase", "--words", "4", "--separator", "_"])).unwrap();
        assert_eq!(p.secret.expose().split('_').count(), 4);
        let pin = generate(&args(&["--pin", "6"])).unwrap();
        assert!(pin.secret.expose().chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn invalid_recipe_errors() {
        assert!(generate(&args(&["--length", "2"])).is_err());
    }

    #[test]
    fn run_prints_secret_and_entropy() {
        let mut fx = Fixture::new(&[]);
        run(&mut fx.app(), &args(&[])).unwrap();
        let out = fx.take_output();
        assert_eq!(out.lines().count(), 2);
        assert!(out.contains("bits of entropy"));
        run(&mut fx.app(), &args(&["-q"])).unwrap();
        assert_eq!(fx.take_output().lines().count(), 1);
    }

    #[test]
    fn default_password_helper() {
        assert_eq!(super::default_password().unwrap().expose().len(), 20);
    }
}

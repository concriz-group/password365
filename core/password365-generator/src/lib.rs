//! Password, passphrase and PIN generation.
//!
//! All randomness comes from `password365_crypto::random` (OS CSPRNG, unbiased
//! sampling).

pub mod charset;
pub mod entropy;
mod error;
pub mod passphrase;
pub mod password;
mod pick;
pub mod wordlist;

pub use error::{GeneratorError, Result};
pub use passphrase::PassphraseRecipe;
pub use password::PasswordRecipe;

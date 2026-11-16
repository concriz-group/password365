//! Website matching for autofill.
//!
//! Decides whether a saved login belongs to the page the user is on. The rules
//! are deliberately strict to defeat phishing:
//! - sites are compared by **registrable domain** (eTLD+1, via the Public
//!   Suffix List): `accounts.google.com` ↔ `mail.google.com` match,
//!   `google.com.evil.io` does not;
//! - credentials saved for `https` are never filled into an `http` page;
//! - IP addresses and single-label hosts (`localhost`) must match exactly.

mod matching;
mod site;

pub use matching::{MatchLevel, fill_allowed, match_level};
pub use site::Site;

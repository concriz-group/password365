//! Item categories (1Password's list). New kinds are only ever appended: the
//! position is the stored tag.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

macro_rules! kinds {
    ($($variant:ident => $id:literal, $label:literal, $plural:literal;)*) => {
        /// What kind of secret an item holds. Drives which fields the UI shows.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum ItemKind {
            $(
                #[doc = $label]
                $variant,
            )*
        }

        impl ItemKind {
            /// All kinds, in stored order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            /// Stable lowercase identifier.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }

            /// Display name, e.g. "Credit Card".
            #[must_use]
            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)*
                }
            }

            /// Plural display name, e.g. "Credit Cards".
            #[must_use]
            pub const fn plural(self) -> &'static str {
                match self {
                    $(Self::$variant => $plural,)*
                }
            }
        }
    };
}

kinds! {
    Login => "login", "Login", "Logins";
    Password => "password", "Password", "Passwords";
    SecureNote => "note", "Secure Note", "Secure Notes";
    CreditCard => "card", "Credit Card", "Credit Cards";
    Identity => "identity", "Identity", "Identities";
    ApiCredential => "api_credential", "API Credential", "API Credentials";
    BankAccount => "bank_account", "Bank Account", "Bank Accounts";
    CryptoWallet => "crypto_wallet", "Crypto Wallet", "Crypto Wallets";
    Database => "database", "Database", "Databases";
    DriverLicense => "driver_license", "Driver License", "Driver Licenses";
    Email => "email", "Email", "Email Accounts";
    MedicalRecord => "medical_record", "Medical Record", "Medical Records";
    Membership => "membership", "Membership", "Memberships";
    OutdoorLicense => "outdoor_license", "Outdoor License", "Outdoor Licenses";
    Passport => "passport", "Passport", "Passports";
    Rewards => "rewards", "Rewards", "Reward Programs";
    SocialSecurityNumber => "ssn", "Social Security Number", "Social Security Numbers";
    SoftwareLicense => "software_license", "Software License", "Software Licenses";
    SshKey => "ssh_key", "SSH Key", "SSH Keys";
    Server => "server", "Server", "Servers";
    WirelessRouter => "wireless_router", "Wireless Router", "Wireless Routers";
    Document => "document", "Document", "Documents";
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ItemKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|k| k.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| format!("unknown item kind '{s}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_parse_round_trip() {
        for &kind in ItemKind::ALL {
            assert_eq!(kind.to_string().parse::<ItemKind>().unwrap(), kind);
        }
    }

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!(" LOGIN ".parse::<ItemKind>().unwrap(), ItemKind::Login);
        assert_eq!("Card".parse::<ItemKind>().unwrap(), ItemKind::CreditCard);
    }

    #[test]
    fn parse_rejects_unknown() {
        assert!(
            "spaceship"
                .parse::<ItemKind>()
                .unwrap_err()
                .contains("unknown item kind")
        );
    }

    #[test]
    fn the_first_kinds_keep_their_stored_tags() {
        assert_eq!(
            ItemKind::ALL[..3],
            [ItemKind::Login, ItemKind::Password, ItemKind::SecureNote]
        );
        assert_eq!(ItemKind::CreditCard.label(), "Credit Card");
        assert_eq!(ItemKind::Identity.plural(), "Identities");
    }
}

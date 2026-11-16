//! Formatting items and vaults for the terminal.

use std::io::{self, Write};

use password365_crypto::secret::SecretString;
use password365_model::Item;
use password365_vault::{ItemSummary, Session};

/// Placeholder shown instead of hidden secrets.
pub const MASK: &str = "••••••••";
/// Characters of the id shown in tables.
pub const SHORT_ID: usize = 8;

/// Prints items as a table.
pub fn item_table(
    out: &mut dyn Write,
    session: &Session,
    items: &[&ItemSummary],
) -> io::Result<()> {
    if items.is_empty() {
        return writeln!(out, "No items.");
    }
    writeln!(out, "{:<10}{:<32}{:<32}VAULT", "ID", "TITLE", "USERNAME")?;
    for s in items {
        let vault = session.vault(s.vault_id).map_or("?", |v| v.name.as_str());
        writeln!(
            out,
            "{:<10}{:<32}{:<32}{}",
            &s.id.to_string()[..SHORT_ID],
            truncate(&s.overview.title, 30),
            truncate(s.overview.username.as_deref().unwrap_or(""), 30),
            vault
        )?;
    }
    Ok(())
}

/// Prints one item; secrets are masked unless `reveal` is set.
pub fn item_detail(out: &mut dyn Write, item: &Item, vault: &str, reveal: bool) -> io::Result<()> {
    let secret = |s: &Option<SecretString>| match s {
        Some(v) if reveal => v.expose().to_owned(),
        Some(_) => MASK.to_owned(),
        None => String::new(),
    };
    writeln!(out, "Title:    {}", item.overview.title)?;
    writeln!(out, "Kind:     {}", item.kind)?;
    writeln!(out, "Vault:    {vault}")?;
    writeln!(out, "ID:       {}", item.id)?;
    field(
        out,
        "Username",
        item.overview.username.as_deref().unwrap_or(""),
    )?;
    field(out, "Password", &secret(&item.details.password))?;
    for url in &item.overview.urls {
        field(out, "URL", url)?;
    }
    field(out, "TOTP", &secret(&item.details.totp))?;
    field(out, "Notes", &secret(&item.details.notes))?;
    if !item.overview.tags.is_empty() {
        field(out, "Tags", &item.overview.tags.join(", "))?;
    }
    writeln!(
        out,
        "Version:  {} ({} previous passwords)",
        item.version,
        item.details.password_history.len()
    )
}

fn field(out: &mut dyn Write, label: &str, value: &str) -> io::Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    writeln!(out, "{:<10}{value}", format!("{label}:"))
}

/// Shortens `text` to at most `max` characters, adding an ellipsis.
#[must_use]
pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId};

    use super::*;

    fn item() -> Item {
        let mut details = ItemDetails::with_password("hunter2".into()).notes("note".into());
        details.totp = Some("otp".into());
        Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("Mail")
                .unwrap()
                .with_username("me")
                .with_url("https://m.com")
                .with_tag("t"),
            details,
            Timestamp::now(),
        )
    }

    fn render(reveal: bool) -> String {
        let mut out = Vec::new();
        item_detail(&mut out, &item(), "Personal", reveal).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn masks_secrets_by_default() {
        let text = render(false);
        assert!(text.contains("Password: ••••••••"));
        assert!(!text.contains("hunter2"));
        assert!(text.contains("Username: me"));
        assert!(text.contains("URL:      https://m.com"));
        assert!(text.contains("Tags:     t"));
    }

    #[test]
    fn reveals_on_request() {
        let text = render(true);
        assert!(text.contains("hunter2"));
        assert!(text.contains("TOTP:     otp"));
        assert!(text.contains("Notes:    note"));
    }

    #[test]
    fn truncates_long_text() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}

//! Turning user-typed names / ids into vault and item ids.

use anyhow::{Result, anyhow, bail};
use password365_model::{ItemId, VaultId};
use password365_vault::{DEFAULT_VAULT_NAME, Session};

/// Minimum length of an id prefix accepted on the command line.
pub const MIN_ID_PREFIX: usize = 4;

/// Vault by name, or the default vault when `name` is `None`.
pub fn vault(session: &Session, name: Option<&str>) -> Result<VaultId> {
    let name = name.unwrap_or(DEFAULT_VAULT_NAME);
    session
        .vault_by_name(name)
        .map(|v| v.id)
        .ok_or_else(|| anyhow!("no vault named '{name}'"))
}

/// Item by full id, unique id prefix, or unique case-insensitive title.
pub fn item(session: &Session, query: &str) -> Result<ItemId> {
    let query = query.trim();
    if let Ok(id) = query.parse::<ItemId>() {
        return Ok(id);
    }
    let by_prefix: Vec<ItemId> = if query.len() >= MIN_ID_PREFIX {
        session
            .items()
            .iter()
            .filter(|s| s.id.to_string().starts_with(query))
            .map(|s| s.id)
            .collect()
    } else {
        Vec::new()
    };
    let candidates = if by_prefix.is_empty() {
        session
            .items()
            .iter()
            .filter(|s| s.overview.title.eq_ignore_ascii_case(query))
            .map(|s| s.id)
            .collect()
    } else {
        by_prefix
    };
    match candidates.as_slice() {
        [one] => Ok(*one),
        [] => bail!("no item matches '{query}'"),
        many => bail!(
            "'{query}' matches {} items — use the id instead",
            many.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::*;
    use crate::app::test_support::{Fixture, PASSWORD};

    fn session_with(titles: &[&str]) -> (Fixture, Session, Vec<ItemId>) {
        let mut fx = Fixture::initialized(&[PASSWORD]);
        let mut session = fx.app().unlock().unwrap();
        let v = vault(&session, None).unwrap();
        let ids = titles
            .iter()
            .map(|t| {
                session
                    .add_item(
                        v,
                        ItemKind::Login,
                        ItemOverview::new(t).unwrap(),
                        ItemDetails::default(),
                    )
                    .unwrap()
            })
            .collect();
        (fx, session, ids)
    }

    #[test]
    fn default_and_named_vaults() {
        let (_fx, mut session, _) = session_with(&[]);
        let work = session.create_vault("Work").unwrap();
        assert_eq!(vault(&session, Some("work")).unwrap(), work);
        assert!(vault(&session, None).is_ok());
        assert!(vault(&session, Some("nope")).is_err());
    }

    #[test]
    fn item_by_full_id_prefix_and_title() {
        let (_fx, session, ids) = session_with(&["GitHub", "Mail"]);
        assert_eq!(item(&session, &ids[0].to_string()).unwrap(), ids[0]);
        assert_eq!(item(&session, &ids[1].to_string()[..8]).unwrap(), ids[1]);
        assert_eq!(item(&session, "github").unwrap(), ids[0]);
    }

    #[test]
    fn ambiguous_and_missing_items() {
        let (_fx, session, _) = session_with(&["Same", "same"]);
        assert!(
            item(&session, "SAME")
                .unwrap_err()
                .to_string()
                .contains("matches 2")
        );
        assert!(
            item(&session, "zzz")
                .unwrap_err()
                .to_string()
                .contains("no item")
        );
    }
}

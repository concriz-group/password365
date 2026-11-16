//! CSV import / export orchestration on an unlocked session.

use std::io::{Read, Write};

use password365_import::{chrome_export, csv_import, dedupe};
use password365_model::{Item, Timestamp, VaultId};
use password365_vault::Session;

use crate::Result;
use crate::dto::ImportSummary;

/// Imports a browser CSV into `vault_id`. With `update`, existing logins whose
/// password differs get the imported password (the old one goes to history).
pub fn import_csv<R: Read>(
    session: &mut Session,
    reader: R,
    vault_id: VaultId,
    update: bool,
    now: Timestamp,
) -> Result<ImportSummary> {
    let parsed = csv_import::parse(reader)?;
    session.vault(vault_id)?;
    // Only the destination vault: an import never touches items in other vaults.
    let existing = items(session, Some(vault_id))?;
    let plan = dedupe::plan(parsed.logins, &existing);

    let new_items = plan
        .new
        .into_iter()
        .map(|login| login.into_item(vault_id, now))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    session.add_items(&new_items)?;

    let mut updated = 0;
    if update {
        for (login, id) in &plan.changed {
            let mut item = session.item(*id)?;
            if item.set_password(login.password.clone(), now) {
                session.save_item(&item)?;
                updated += 1;
            }
        }
    }

    Ok(ImportSummary {
        imported: new_items.len(),
        identical: plan.identical.len(),
        changed: plan.changed.len(),
        updated,
        duplicates_in_file: plan.duplicates_in_file,
        skipped: parsed
            .skipped
            .into_iter()
            .map(|s| (s.line, s.reason.to_owned()))
            .collect(),
    })
}

/// Writes logins (optionally of one vault) as Chrome CSV. Returns rows written.
/// Chrome's format has no extra fields, so a login's own fields are added to
/// the end of its note (`label: value`), not lost.
pub fn export_csv<W: Write>(session: &Session, writer: W, vault: Option<VaultId>) -> Result<usize> {
    let items: Vec<Item> = items(session, vault)?
        .into_iter()
        .map(with_fields_in_note)
        .collect();
    Ok(chrome_export::write(writer, &items)?)
}

/// `item` with its custom fields appended to its note (links to other items
/// and internal fields left out).
fn with_fields_in_note(mut item: Item) -> Item {
    use password365_model::item_template::FieldInput;
    let lines: Vec<String> = crate::field_layout::describe(item.kind, &item.details.custom_fields)
        .into_iter()
        .filter(|(f, meta)| meta.input != FieldInput::SignInWith && !f.value.expose().is_empty())
        .map(|(f, _)| format!("{}: {}", f.label, f.value.expose()))
        .collect();
    if lines.is_empty() {
        return item;
    }
    let note = item
        .details
        .notes
        .as_ref()
        .map(|n| n.expose().trim_end().to_owned())
        .unwrap_or_default();
    let joined = if note.is_empty() {
        lines.join("\n")
    } else {
        format!("{note}\n\n{}", lines.join("\n"))
    };
    item.details.notes = Some(joined.into());
    item
}

/// Decrypts all items, optionally restricted to one vault.
fn items(session: &Session, vault: Option<VaultId>) -> Result<Vec<Item>> {
    session
        .items()
        .iter()
        .filter(|s| vault.is_none_or(|v| s.vault_id == v))
        .map(|s| session.item(s.id).map_err(Into::into))
        .collect()
}

#[cfg(test)]
mod tests {
    use password365_crypto::kdf::KdfParams;
    use password365_crypto::secret_key::AccountSecretKey;
    use password365_vault::{Credentials, Store};

    use super::*;

    #[test]
    fn a_logins_own_fields_go_into_the_exported_note() {
        use password365_model::{CustomField, ItemDetails, ItemKind, ItemOverview};
        let field = |label: &str, value: &str| CustomField {
            label: label.into(),
            value: value.into(),
            concealed: false,
        };
        let mut details = ItemDetails::with_password("pw".into()).notes("hello".into());
        details.custom_fields = vec![
            field("recovery code", "R1"),
            field("empty", ""),
            field(crate::item_icon::ICON_LABEL, "data:image/png;base64,x"),
        ];
        let item = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("A").unwrap(),
            details,
            Timestamp::now(),
        );
        let out = with_fields_in_note(item);
        assert_eq!(
            out.details.notes.unwrap().expose(),
            "hello\n\nrecovery code: R1"
        );
        let plain = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("B").unwrap(),
            ItemDetails::default(),
            Timestamp::now(),
        );
        assert!(with_fields_in_note(plain).details.notes.is_none());
    }

    fn session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let creds = Credentials {
            password: "pw".into(),
            secret_key: AccountSecretKey::from_bytes([1; 16]),
        };
        let s = Store::create(
            &dir.path().join("s"),
            &creds,
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        (dir, s)
    }

    const CSV: &str =
        "name,url,username,password\nA,https://a.com,u,1\nB,https://b.com,u,2\nbad,,,\n";

    #[test]
    fn import_then_reimport_with_changes() {
        let (_d, mut s) = session();
        let v = s.vault_by_name("Personal").unwrap().id;
        let first = import_csv(&mut s, CSV.as_bytes(), v, false, Timestamp::now()).unwrap();
        assert_eq!((first.imported, first.skipped.len()), (2, 1));

        let changed = "name,url,username,password\nA,https://a.com,u,1\nB,https://b.com,u,NEW\nB,https://b.com,u,NEW\n";
        let second = import_csv(&mut s, changed.as_bytes(), v, false, Timestamp::now()).unwrap();
        assert_eq!(
            (
                second.imported,
                second.identical,
                second.changed,
                second.updated
            ),
            (0, 1, 1, 0)
        );
        assert_eq!(second.duplicates_in_file, 1);

        let third = import_csv(&mut s, changed.as_bytes(), v, true, Timestamp::now()).unwrap();
        assert_eq!(third.updated, 1);
    }

    #[test]
    fn importing_into_one_vault_never_changes_another() {
        let (_d, mut s) = session();
        let personal = s.vault_by_name("Personal").unwrap().id;
        let work = s.create_vault("Work").unwrap();
        let csv = "name,url,username,password\nA,https://a.com,u,old\n";
        import_csv(&mut s, csv.as_bytes(), work, false, Timestamp::now()).unwrap();
        let other = "name,url,username,password\nA,https://a.com,u,NEW\n";
        let r = import_csv(&mut s, other.as_bytes(), personal, true, Timestamp::now()).unwrap();
        assert_eq!((r.imported, r.updated), (1, 0));
        let work_items = items(&s, Some(work)).unwrap();
        assert_eq!(
            work_items[0].details.password.as_ref().unwrap().expose(),
            "old"
        );
    }

    #[test]
    fn import_into_unknown_vault_fails() {
        let (_d, mut s) = session();
        assert!(
            import_csv(
                &mut s,
                CSV.as_bytes(),
                VaultId::new(),
                false,
                Timestamp::now()
            )
            .is_err()
        );
    }

    #[test]
    fn export_all_or_one_vault() {
        let (_d, mut s) = session();
        let v = s.vault_by_name("Personal").unwrap().id;
        let other = s.create_vault("Other").unwrap();
        import_csv(&mut s, CSV.as_bytes(), v, false, Timestamp::now()).unwrap();
        let mut out = Vec::new();
        assert_eq!(export_csv(&s, &mut out, None).unwrap(), 2);
        assert_eq!(export_csv(&s, Vec::new(), Some(other)).unwrap(), 0);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("A,https://a.com,u,1,")
        );
    }
}

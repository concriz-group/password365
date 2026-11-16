//! Item operations on an unlocked [`Session`].

use password365_model::{Item, ItemDetails, ItemId, ItemKind, ItemOverview, Timestamp, VaultId};

use password365_sync::{ItemState, SyncItem};

use crate::item_crypto;
use crate::item_summary::ItemSummary;
use crate::session::{ItemEntry, Session};
use crate::session_sync::Pending;
use crate::{Result, VaultError};

impl Session {
    /// Creates a new item in `vault_id` and returns its id.
    pub fn add_item(
        &mut self,
        vault_id: VaultId,
        kind: ItemKind,
        overview: ItemOverview,
        details: ItemDetails,
    ) -> Result<ItemId> {
        let item = Item::new(vault_id, kind, overview, details, Timestamp::now());
        self.add_items(std::slice::from_ref(&item))?;
        Ok(item.id)
    }

    /// Adds many prepared items atomically (used by import).
    pub fn add_items(&mut self, items: &[Item]) -> Result<()> {
        for item in items {
            self.vault_entry(item.vault_id)?;
        }
        let pending = items
            .iter()
            .map(|item| Pending {
                state: ItemState::live(SyncItem::capture(item, self.next_stamp())),
                version: item.version,
            })
            .collect();
        self.commit_states(pending)
    }

    /// Summaries of all items, sorted by title.
    #[must_use]
    pub fn items(&self) -> Vec<&ItemSummary> {
        self.filtered(|_| true)
    }

    /// Summaries of items in one vault, sorted by title.
    #[must_use]
    pub fn items_in(&self, vault_id: VaultId) -> Vec<&ItemSummary> {
        self.filtered(|s| s.vault_id == vault_id)
    }

    /// Items whose title, username, URL or tag contains `query`.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&ItemSummary> {
        self.filtered(|s| s.overview.matches(query))
    }

    /// Decrypts a full item, including secrets.
    pub fn item(&self, id: ItemId) -> Result<Item> {
        let entry = self.item_entry(id)?;
        item_crypto::open_item(&self.vault_entry(entry.record.vault_id)?.key, &entry.record)
    }

    /// Saves a modified item. Its `version` must be newer than the stored one
    /// (use the model's mutators, which bump it).
    pub fn save_item(&mut self, item: &Item) -> Result<()> {
        let stored = self.item_entry(item.id)?;
        if item.version <= stored.record.version {
            return Err(VaultError::StaleVersion);
        }
        self.vault_entry(item.vault_id)?;
        let mut state = self.sync_item(item.id)?;
        let stamp = self.next_stamp();
        state.record(item, stamp);
        self.commit_states(vec![Pending {
            state: ItemState::live(state),
            version: item.version,
        }])
    }

    /// Moves an item to Recently Deleted (restorable; peers delete it too).
    pub fn delete_item(&mut self, id: ItemId) -> Result<()> {
        let version = self.item_entry(id)?.record.version + 1;
        let state = ItemState::live(self.sync_item(id)?).deleted(self.next_stamp());
        self.commit_states(vec![Pending { state, version }])
    }

    /// Items in Recently Deleted with when they were deleted, newest first.
    #[must_use]
    pub fn deleted_items(&self) -> Vec<(&ItemSummary, Timestamp)> {
        let mut list: Vec<(&ItemSummary, Timestamp)> = self
            .items
            .values()
            .filter(|e| e.is_deleted())
            .map(|e| (&e.summary, Timestamp::from_millis(e.sync.latest.wall_ms)))
            .collect();
        list.sort_by_key(|(s, at)| (std::cmp::Reverse(*at), s.id));
        list
    }

    /// Brings an item back from Recently Deleted (on every device).
    pub fn restore_item(&mut self, id: ItemId) -> Result<()> {
        let entry = self.items.get(&id).filter(|e| e.is_deleted());
        let version = entry
            .ok_or(VaultError::NotFound("deleted item"))?
            .record
            .version
            + 1;
        let mut state = self.sync_item(id)?;
        state.restore(self.next_stamp());
        self.commit_states(vec![Pending {
            state: ItemState::live(state),
            version,
        }])
    }

    /// Wipes items deleted before `cutoff` for good, keeping only the
    /// deletion markers peers need. Returns how many were purged.
    pub fn purge_deleted(&mut self, cutoff: Timestamp) -> Result<usize> {
        let ids: Vec<ItemId> = self
            .items
            .iter()
            .filter(|(_, e)| e.is_deleted() && e.sync.latest.wall_ms < cutoff.as_millis())
            .map(|(id, _)| *id)
            .collect();
        self.purge(&ids)?;
        Ok(ids.len())
    }

    /// Deletes an item from Recently Deleted for good ("Delete Permanently").
    pub fn purge_item(&mut self, id: ItemId) -> Result<()> {
        if !self.items.get(&id).is_some_and(ItemEntry::is_deleted) {
            return Err(VaultError::NotFound("deleted item"));
        }
        self.purge(&[id])
    }

    pub(crate) fn purge(&mut self, ids: &[ItemId]) -> Result<()> {
        let pending = ids
            .iter()
            .map(|id| {
                Ok(Pending {
                    state: ItemState::live(self.sync_item(*id)?).purged(),
                    version: 0,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.commit_states(pending)
    }

    fn item_entry(&self, id: ItemId) -> Result<&ItemEntry> {
        self.items
            .get(&id)
            .filter(|e| !e.is_deleted())
            .ok_or(VaultError::NotFound("item"))
    }

    fn filtered(&self, keep: impl Fn(&ItemSummary) -> bool) -> Vec<&ItemSummary> {
        let mut list: Vec<&ItemSummary> = self
            .items
            .values()
            .filter(|e| !e.is_deleted() && keep(&e.summary))
            .map(|e| &e.summary)
            .collect();
        list.sort_by_key(|s| s.overview.title.to_lowercase());
        list
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{Item, ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId};

    use crate::store::test_support::new_session;
    use crate::{Session, VaultError};

    fn personal(s: &Session) -> VaultId {
        s.vault_by_name("Personal").unwrap().id
    }

    fn add(s: &mut Session, title: &str, pw: &str) -> password365_model::ItemId {
        let v = personal(s);
        s.add_item(
            v,
            ItemKind::Login,
            ItemOverview::new(title)
                .unwrap()
                .with_url("https://example.com"),
            ItemDetails::with_password(pw.into()),
        )
        .unwrap()
    }

    #[test]
    fn add_and_read_item() {
        let (_d, mut s) = new_session();
        let id = add(&mut s, "Mail", "pw1");
        let item = s.item(id).unwrap();
        assert_eq!(item.overview.title, "Mail");
        assert_eq!(item.details.password.unwrap().expose(), "pw1");
    }

    #[test]
    fn add_to_unknown_vault_fails() {
        let (_d, mut s) = new_session();
        let r = s.add_item(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("x").unwrap(),
            ItemDetails::default(),
        );
        assert!(matches!(r, Err(VaultError::NotFound("vault"))));
    }

    #[test]
    fn listing_search_and_vault_filter() {
        let (_d, mut s) = new_session();
        add(&mut s, "beta", "1");
        add(&mut s, "Alpha", "2");
        let work = s.create_vault("Work").unwrap();
        s.add_item(
            work,
            ItemKind::SecureNote,
            ItemOverview::new("Gamma").unwrap(),
            ItemDetails::default(),
        )
        .unwrap();
        let titles = |v: Vec<&crate::ItemSummary>| {
            v.iter()
                .map(|i| i.overview.title.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(s.items()), vec!["Alpha", "beta", "Gamma"]);
        assert_eq!(titles(s.items_in(work)), vec!["Gamma"]);
        assert_eq!(titles(s.search("alp")), vec!["Alpha"]);
        assert_eq!(s.search("example.com").len(), 2);
    }

    #[test]
    fn save_updates_and_keeps_history() {
        let (_d, mut s) = new_session();
        let id = add(&mut s, "Mail", "old");
        let mut item = s.item(id).unwrap();
        item.set_password(Some("new".into()), Timestamp::now());
        s.save_item(&item).unwrap();
        let reloaded = s.item(id).unwrap();
        assert_eq!(reloaded.details.password.unwrap().expose(), "new");
        assert_eq!(
            reloaded.details.password_history.entries()[0]
                .password
                .expose(),
            "old"
        );
        assert_eq!(reloaded.version, 2);
    }

    #[test]
    fn stale_save_is_rejected() {
        let (_d, mut s) = new_session();
        let id = add(&mut s, "Mail", "pw");
        let unchanged = s.item(id).unwrap();
        assert!(matches!(
            s.save_item(&unchanged),
            Err(VaultError::StaleVersion)
        ));
    }

    #[test]
    fn save_unknown_item_fails() {
        let (_d, mut s) = new_session();
        let v = personal(&s);
        let item = Item::new(
            v,
            ItemKind::Login,
            ItemOverview::new("x").unwrap(),
            ItemDetails::default(),
            Timestamp::now(),
        );
        assert!(matches!(
            s.save_item(&item),
            Err(VaultError::NotFound("item"))
        ));
    }

    #[test]
    fn move_item_between_vaults() {
        let (_d, mut s) = new_session();
        let id = add(&mut s, "Mail", "pw");
        let work = s.create_vault("Work").unwrap();
        let mut item = s.item(id).unwrap();
        item.move_to(work, Timestamp::now());
        s.save_item(&item).unwrap();
        assert_eq!(s.items_in(work).len(), 1);
        assert_eq!(s.item(id).unwrap().details.password.unwrap().expose(), "pw");
    }

    #[test]
    fn delete_item() {
        let (_d, mut s) = new_session();
        let id = add(&mut s, "Mail", "pw");
        s.delete_item(id).unwrap();
        assert!(s.items().is_empty());
        assert!(matches!(s.item(id), Err(VaultError::NotFound("item"))));
        assert!(matches!(
            s.delete_item(id),
            Err(VaultError::NotFound("item"))
        ));
    }

    #[test]
    fn add_items_in_bulk() {
        let (_d, mut s) = new_session();
        let v = personal(&s);
        let items: Vec<Item> = (0..5)
            .map(|i| {
                Item::new(
                    v,
                    ItemKind::Login,
                    ItemOverview::new(&format!("i{i}")).unwrap(),
                    ItemDetails::default(),
                    Timestamp::now(),
                )
            })
            .collect();
        s.add_items(&items).unwrap();
        assert_eq!(s.items().len(), 5);
    }

    #[test]
    fn add_items_rejects_unknown_vault_without_writing() {
        let (_d, mut s) = new_session();
        let v = personal(&s);
        let good = Item::new(
            v,
            ItemKind::Login,
            ItemOverview::new("a").unwrap(),
            ItemDetails::default(),
            Timestamp::now(),
        );
        let bad = Item::new(
            VaultId::new(),
            ItemKind::Login,
            ItemOverview::new("b").unwrap(),
            ItemDetails::default(),
            Timestamp::now(),
        );
        assert!(s.add_items(&[good, bad]).is_err());
        assert!(s.items().is_empty());
    }
}

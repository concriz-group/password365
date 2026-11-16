//! A set of keyed values where each entry is its own last-writer-wins
//! register (format v3): an item's custom fields and its attachments.
//! Editing one custom field on the phone and another on the laptop keeps
//! both; removing a field is a write too, so it does not come back.

use std::collections::BTreeMap;

use password365_model::{Attachment, CustomField};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::Hlc;

/// A value with a stable key (what makes two entries "the same field").
pub trait MapValue: Clone + PartialEq + Serialize + DeserializeOwned {
    /// The key: a custom field's label (any case), an attachment's id.
    fn key(&self) -> String;
}

impl MapValue for CustomField {
    fn key(&self) -> String {
        self.label.to_lowercase()
    }
}

impl MapValue for Attachment {
    fn key(&self) -> String {
        self.id.to_string()
    }
}

/// One entry: its value (`None` once removed), its place and its last write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry<V> {
    /// The value; `None` after it was removed.
    pub value: Option<V>,
    /// Position among the entries (the order the user arranged).
    pub order: u32,
    /// Stamp of the last write (value, position or removal).
    pub stamp: Hlc,
}

/// An entry's stamp and place without its value (stored next to the item,
/// which keeps the values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryStamp {
    /// Stamp of the last write.
    pub stamp: Hlc,
    /// Position.
    pub order: u32,
    /// `false` once removed.
    pub present: bool,
}

/// Stamps of every entry, by key.
pub type MapStamps = BTreeMap<String, EntryStamp>;

/// Entries by key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LwwMap<V> {
    /// Every entry ever written, removed ones included.
    pub entries: BTreeMap<String, Entry<V>>,
    /// The map's own stamp (when it was captured or restored): an empty
    /// map still has one.
    pub floor: Hlc,
}

impl<V> LwwMap<V> {
    /// No entries, stamped `floor`.
    #[must_use]
    pub const fn empty(floor: Hlc) -> Self {
        Self {
            entries: BTreeMap::new(),
            floor,
        }
    }
}

fn order_of(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

impl<V: MapValue> LwwMap<V> {
    /// `values`, in their order, all written at `stamp`.
    #[must_use]
    pub fn capture(values: &[V], stamp: Hlc) -> Self {
        let entries = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                (
                    v.key(),
                    Entry {
                        value: Some(v.clone()),
                        order: order_of(i),
                        stamp,
                    },
                )
            })
            .collect();
        Self {
            entries,
            floor: stamp,
        }
    }

    /// Rebuilds the map from stored values and their stamps; a value with no
    /// stamp of its own (an older record) gets `fallback`.
    #[must_use]
    pub fn with_stamps(values: &[V], stamps: &MapStamps, fallback: Hlc) -> Self {
        let mut entries: BTreeMap<String, Entry<V>> = stamps
            .iter()
            .filter(|(_, s)| !s.present)
            .map(|(k, s)| {
                (
                    k.clone(),
                    Entry {
                        value: None,
                        order: s.order,
                        stamp: s.stamp,
                    },
                )
            })
            .collect();
        for (i, value) in values.iter().enumerate() {
            let key = value.key();
            let (stamp, order) = stamps
                .get(&key)
                .filter(|s| s.present)
                .map_or((fallback, order_of(i)), |s| (s.stamp, s.order));
            entries.insert(
                key,
                Entry {
                    value: Some(value.clone()),
                    order,
                    stamp,
                },
            );
        }
        Self {
            entries,
            floor: fallback,
        }
    }

    /// Writes `values` (the whole list, in order) at `stamp`: new or changed
    /// entries and moved ones are restamped, missing ones are removed.
    /// Returns whether anything changed.
    pub fn set(&mut self, values: &[V], stamp: Hlc) -> bool {
        let mut changed = false;
        let wanted: BTreeMap<String, (u32, &V)> = values
            .iter()
            .enumerate()
            .map(|(i, v)| (v.key(), (order_of(i), v)))
            .collect();
        for (key, (order, value)) in &wanted {
            let same = self
                .entries
                .get(key)
                .is_some_and(|e| e.order == *order && e.value.as_ref() == Some(*value));
            if !same {
                self.entries.insert(
                    key.clone(),
                    Entry {
                        value: Some((*value).clone()),
                        order: *order,
                        stamp,
                    },
                );
                changed = true;
            }
        }
        for (key, entry) in &mut self.entries {
            if entry.value.is_some() && !wanted.contains_key(key) {
                entry.value = None;
                entry.stamp = stamp;
                changed = true;
            }
        }
        changed
    }

    /// Entry by entry, the newer write (stamps are unique per write).
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut entries = self.entries.clone();
        for (key, theirs) in &other.entries {
            match entries.get(key) {
                Some(ours) if ours.stamp >= theirs.stamp => {}
                _ => {
                    entries.insert(key.clone(), theirs.clone());
                }
            }
        }
        Self {
            entries,
            floor: self.floor.max(other.floor),
        }
    }

    /// The present values, in order (ties by key).
    #[must_use]
    pub fn values(&self) -> Vec<V> {
        let mut present: Vec<(&u32, &String, &V)> = self
            .entries
            .iter()
            .filter_map(|(k, e)| e.value.as_ref().map(|v| (&e.order, k, v)))
            .collect();
        present.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        present.into_iter().map(|(_, _, v)| v.clone()).collect()
    }

    /// The newest write (the map's own stamp at least).
    #[must_use]
    pub fn latest(&self) -> Hlc {
        self.entries
            .values()
            .map(|e| e.stamp)
            .fold(self.floor, Hlc::max)
    }

    /// Every entry restamped (an item restored from Recently Deleted).
    pub fn restamp(&mut self, stamp: Hlc) {
        self.floor = stamp;
        for entry in self.entries.values_mut() {
            entry.stamp = stamp;
        }
    }

    /// The stamps alone.
    #[must_use]
    pub fn stamps(&self) -> MapStamps {
        self.entries
            .iter()
            .map(|(k, e)| {
                (
                    k.clone(),
                    EntryStamp {
                        stamp: e.stamp,
                        order: e.order,
                        present: e.value.is_some(),
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use password365_crypto::secret::SecretString;

    use super::*;
    use crate::DeviceId;

    fn at(wall_ms: i64) -> Hlc {
        Hlc {
            wall_ms,
            counter: 0,
            device: DeviceId::from_bytes([1; 16]),
        }
    }

    fn f(label: &str, value: &str) -> CustomField {
        CustomField {
            label: label.into(),
            value: SecretString::from(value),
            concealed: false,
        }
    }

    fn labels(map: &LwwMap<CustomField>) -> Vec<String> {
        map.values()
            .into_iter()
            .map(|f| format!("{}={}", f.label, f.value.expose()))
            .collect()
    }

    #[test]
    fn edits_of_different_fields_on_two_devices_both_survive() {
        let base = LwwMap::capture(&[f("pin", "1"), f("club", "x")], at(1));
        let mut phone = base.clone();
        phone.set(&[f("pin", "2"), f("club", "x")], at(2));
        let mut laptop = base;
        laptop.set(&[f("pin", "1"), f("club", "y"), f("plan", "pro")], at(3));
        let merged = phone.merge(&laptop);
        assert_eq!(labels(&merged), ["pin=2", "club=y", "plan=pro"]);
        assert_eq!(
            merged,
            laptop.merge(&phone),
            "order of merging does not matter"
        );
    }

    #[test]
    fn a_removal_is_a_write_and_the_newer_one_wins() {
        let base = LwwMap::capture(&[f("pin", "1"), f("club", "x")], at(1));
        let mut removed = base.clone();
        assert!(removed.set(&[f("pin", "1")], at(2)));
        assert_eq!(
            labels(&removed.merge(&base)),
            ["pin=1"],
            "an older copy does not bring it back"
        );
        let mut edited_later = base;
        edited_later.set(&[f("pin", "1"), f("club", "z")], at(3));
        assert_eq!(
            labels(&removed.merge(&edited_later)),
            ["pin=1", "club=z"],
            "a later edit does"
        );
    }

    #[test]
    fn unchanged_lists_change_nothing_and_moves_are_writes() {
        let mut map = LwwMap::capture(&[f("a", "1"), f("b", "2")], at(1));
        assert!(!map.set(&[f("a", "1"), f("b", "2")], at(2)));
        assert!(map.set(&[f("b", "2"), f("a", "1")], at(3)));
        assert_eq!(labels(&map), ["b=2", "a=1"]);
        assert_eq!(map.latest(), at(3));
        assert_eq!(LwwMap::<CustomField>::empty(at(7)).latest(), at(7));
    }

    #[test]
    fn stamps_rebuild_the_map_with_its_removals() {
        let mut map = LwwMap::capture(&[f("a", "1"), f("b", "2")], at(1));
        map.set(&[f("a", "1")], at(2));
        let rebuilt = LwwMap::with_stamps(&map.values(), &map.stamps(), map.floor);
        assert_eq!(rebuilt, map);
        let older = LwwMap::with_stamps(&[f("a", "1")], &MapStamps::new(), at(5));
        assert_eq!(
            older.entries["a"].stamp,
            at(5),
            "values without stamps get the fallback"
        );
        let mut restamped = map;
        restamped.restamp(at(8));
        assert!(restamped.entries.values().all(|e| e.stamp == at(8)));
    }
}

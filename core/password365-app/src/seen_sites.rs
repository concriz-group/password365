//! Where each login has already been used, kept sealed in the vault (this device only).
//!
//! A login that matches a page only through a related domain (`amazon.com` on `amazon.es`) is not
//! filled the first time without the person's confirmation; afterwards that site is remembered here.
//! The data is the sealed app-data block of the store, shared with the security settings: the
//! settings are the top-level keys and this is the `seen_sites` key, and each writer keeps the
//! other's part.

use std::collections::BTreeMap;

use password365_vault::Session;
use serde_json::{Map, Value};

use crate::{AppError, Result};

const KEY: &str = "seen_sites";

/// The sealed block as a JSON object (empty when there is none or it is not one).
pub(crate) fn read_block(session: &Session) -> Map<String, Value> {
    session
        .app_data()
        .and_then(|d| serde_json::from_slice::<Value>(d).ok())
        .and_then(|v| match v {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

/// Writes `block` back (no write when it did not change).
pub(crate) fn write_block(session: &mut Session, block: &Map<String, Value>) -> Result<()> {
    let json = serde_json::to_vec(block).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(session.set_app_data(&json)?)
}

/// `item id → sites` already confirmed.
pub(crate) fn read(session: &Session) -> BTreeMap<String, Vec<String>> {
    read_block(session)
        .get(KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Whether `item` has been used on `site` (a registrable domain).
pub(crate) fn seen(map: &BTreeMap<String, Vec<String>>, item: &str, site: &str) -> bool {
    map.get(item)
        .is_some_and(|sites| sites.iter().any(|s| s == site))
}

/// Remembers that `item` was used on `site`.
pub(crate) fn remember(session: &mut Session, item: &str, site: &str) -> Result<()> {
    let mut block = read_block(session);
    let mut map = read(session);
    let sites = map.entry(item.to_owned()).or_default();
    if sites.iter().any(|s| s == site) {
        return Ok(());
    }
    sites.push(site.to_owned());
    block.insert(
        KEY.to_owned(),
        serde_json::to_value(&map).map_err(|e| AppError::Internal(e.to_string()))?,
    );
    write_block(session, &block)
}

/// Forgets what is kept about `item` (it was deleted for good).
pub(crate) fn forget(session: &mut Session, item: &str) -> Result<()> {
    let mut map = read(session);
    if map.remove(item).is_none() {
        return Ok(());
    }
    let mut block = read_block(session);
    block.insert(
        KEY.to_owned(),
        serde_json::to_value(&map).map_err(|e| AppError::Internal(e.to_string()))?,
    );
    write_block(session, &block)
}

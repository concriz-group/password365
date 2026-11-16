//! Small non-secret JSON files next to the store (settings, backup log).

use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{AppError, Result};

/// Reads `path`; a missing or unreadable file yields `None`.
pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Writes `value` atomically (temp file, then rename), creating the folder.
pub(crate) fn write<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let json =
        serde_json::to_string_pretty(value).map_err(|e| AppError::Internal(e.to_string()))?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/x.json");
        assert_eq!(read::<Vec<u8>>(&path), None);
        write(&path, &vec![1u8, 2]).unwrap();
        assert_eq!(read::<Vec<u8>>(&path), Some(vec![1, 2]));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(read::<Vec<u8>>(&path), None);
    }
}

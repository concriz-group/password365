//! One-time move from the app's former name (Quillock): its data folders are
//! copied to the new ones on first start. The old copies are left untouched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Bundle identifier used before the rename.
pub const LEGACY_IDENTIFIER: &str = "io.quillock.desktop";

/// The folder the former app used in place of `dir` (a sibling named after
/// the old identifier).
#[must_use]
pub fn legacy_dir(dir: &Path) -> Option<PathBuf> {
    Some(dir.parent()?.join(LEGACY_IDENTIFIER))
}

/// Copies the files of `old` into `new` when `new` lacks `guard` and `old`
/// has it. Files already in `new` are never replaced; subfolders (e.g. web
/// view caches) are skipped. Returns whether anything was adopted.
pub fn adopt(old: &Path, new: &Path, guard: &str) -> io::Result<bool> {
    if new.join(guard).exists() || !old.join(guard).is_file() {
        return Ok(false);
    }
    fs::create_dir_all(new)?;
    for entry in fs::read_dir(old)? {
        let entry = entry?;
        let target = new.join(entry.file_name());
        if entry.file_type()?.is_file() && !target.exists() {
            // `fs::copy` keeps the owner-only permissions.
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(true)
}

/// Adopts the former app's folder next to `dir`, if any.
pub fn adopt_beside(dir: &Path, guard: &str) -> io::Result<bool> {
    match legacy_dir(dir) {
        Some(old) => adopt(&old, dir, guard),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::write(path, text).unwrap();
    }

    #[test]
    fn the_old_folder_is_copied_and_kept() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join(LEGACY_IDENTIFIER);
        let new = root.path().join("app.password365.desktop");
        fs::create_dir_all(old.join("EBWebView")).unwrap();
        write(&old.join("default.qlvault"), "vault");
        write(&old.join("settings.json"), "{}");
        assert!(adopt_beside(&new, "default.qlvault").unwrap());
        assert_eq!(
            fs::read_to_string(new.join("default.qlvault")).unwrap(),
            "vault"
        );
        assert!(new.join("settings.json").exists());
        assert!(!new.join("EBWebView").exists(), "folders are skipped");
        assert!(old.join("default.qlvault").exists(), "the old copy stays");
    }

    #[test]
    fn an_existing_vault_is_never_replaced() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join(LEGACY_IDENTIFIER);
        let new = root.path().join("new");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        write(&old.join("default.qlvault"), "old");
        write(&new.join("default.qlvault"), "new");
        assert!(!adopt(&old, &new, "default.qlvault").unwrap());
        assert_eq!(
            fs::read_to_string(new.join("default.qlvault")).unwrap(),
            "new"
        );
    }

    #[test]
    fn files_already_present_are_kept() {
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        write(&old.join("default.qlvault"), "vault");
        write(&old.join("settings.json"), "old");
        write(&new.join("settings.json"), "new");
        assert!(adopt(&old, &new, "default.qlvault").unwrap());
        assert_eq!(
            fs::read_to_string(new.join("settings.json")).unwrap(),
            "new"
        );
    }

    #[test]
    fn nothing_to_adopt_without_an_old_folder() {
        let root = tempfile::tempdir().unwrap();
        let new = root.path().join("new");
        assert!(!adopt_beside(&new, "default.qlvault").unwrap());
        assert!(!new.exists());
        assert!(!adopt_beside(Path::new("/"), "x").unwrap(), "no parent");
    }

    #[cfg(unix)]
    #[test]
    fn owner_only_permissions_survive() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        fs::create_dir_all(&old).unwrap();
        let file = old.join("secret-key");
        write(&file, "k");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        adopt(&old, &new, "secret-key").unwrap();
        let mode = fs::metadata(new.join("secret-key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

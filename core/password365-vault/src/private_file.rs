//! Creating files readable only by the current user.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Creates `path` exclusively (fails if it exists) with owner-only
/// permissions (0600 on Unix). Missing parent directories are created (0700).
pub fn create_private_file(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_private_dir(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_file_and_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/file");
        create_private_file(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn refuses_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        create_private_file(&path).unwrap();
        let err = create_private_file(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn relative_file_without_parent_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain");
        assert!(create_private_file(&path).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/f");
        create_private_file(&path).unwrap();
        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let dir_mode = std::fs::metadata(dir.path().join("sub"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!((file_mode, dir_mode), (0o600, 0o700));
    }
}

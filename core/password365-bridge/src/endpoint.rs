//! Location of the per-user bridge socket.
//!
//! Unix: a socket file inside a directory only the user can enter (0700), in
//! the user's runtime/cache directory. Windows: a named pipe (see the threat
//! model — its access control still needs hardening before release).

use std::io;
use std::path::PathBuf;

use interprocess::local_socket::Name;
#[cfg(unix)]
use interprocess::local_socket::{GenericFilePath, ToFsName};
#[cfg(windows)]
use interprocess::local_socket::{GenericNamespaced, ToNsName};

/// Socket file name (Unix).
pub const SOCKET_FILE: &str = "bridge.sock";

/// Where the desktop app listens and the host connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    path: PathBuf,
}

impl Endpoint {
    /// The default endpoint for the current user.
    pub fn for_current_user() -> io::Result<Self> {
        let dirs = directories::ProjectDirs::from("app", "Password365", "Password365")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
        let dir = dirs.runtime_dir().unwrap_or_else(|| dirs.cache_dir());
        Ok(Self::at(dir.join(SOCKET_FILE)))
    }

    /// An endpoint at an explicit path (tests, custom setups).
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// The socket path (on Windows, used to derive the pipe name).
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Creates the private parent directory (Unix: mode 0700).
    pub fn prepare(&self) -> io::Result<()> {
        let Some(dir) = self.path.parent() else {
            return Ok(());
        };
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            builder.mode(0o700);
            builder.create(dir)?;
            // An existing directory may have looser permissions: tighten them.
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(not(unix))]
        builder.create(dir)?;
        Ok(())
    }

    /// The interprocess socket name.
    pub(crate) fn name(&self) -> io::Result<Name<'_>> {
        #[cfg(unix)]
        {
            self.path.as_path().to_fs_name::<GenericFilePath>()
        }
        #[cfg(windows)]
        {
            let user = std::env::var("USERNAME").unwrap_or_default();
            format!("password365-bridge-{user}")
                .to_ns_name::<GenericNamespaced>()
                .map(Name::into_owned)
        }
    }

    /// User id that owns the socket directory (= the app's own user), Unix only.
    #[cfg(unix)]
    pub(crate) fn owner_uid(&self) -> io::Result<u32> {
        use std::os::unix::fs::MetadataExt;
        let dir = self
            .path
            .parent()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok(std::fs::metadata(dir)?.uid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_endpoint_uses_socket_file_name() {
        let e = Endpoint::for_current_user().unwrap();
        assert!(e.path().ends_with(SOCKET_FILE));
    }

    #[cfg(unix)]
    #[test]
    fn prepare_creates_private_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("q");
        std::fs::create_dir(&sub).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let e = Endpoint::at(sub.join(SOCKET_FILE));
        e.prepare().unwrap();
        let mode = std::fs::metadata(&sub).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "existing loose permissions are tightened");
        assert!(e.name().is_ok());
        assert!(e.owner_uid().is_ok());
    }

    #[test]
    fn prepare_without_parent_is_a_no_op() {
        assert!(Endpoint::at(PathBuf::new()).prepare().is_ok());
    }
}

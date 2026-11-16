//! SQLite persistence. Stores only the header and opaque encrypted records.

use std::path::Path;

use password365_crypto::kdf::KdfParams;
use rusqlite::{Connection, OptionalExtension, params};

use crate::header::Header;
use crate::{Result, VaultError};

/// Opaque record identifier.
pub(crate) type RecordId = [u8; 16];

const SCHEMA: &str = "
CREATE TABLE header (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    format_version  INTEGER NOT NULL,
    store_id        BLOB    NOT NULL,
    kdf_memory_kib  INTEGER NOT NULL,
    kdf_iterations  INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    salt            BLOB    NOT NULL,
    wrapped_keyset  BLOB    NOT NULL
);
CREATE TABLE records (
    id   BLOB PRIMARY KEY,
    blob BLOB NOT NULL
) WITHOUT ROWID;
";

/// Encrypted attachment bytes, apart from the records (never loaded on unlock).
/// Created on open too, so stores made before attachments existed gain it.
const ATTACHMENTS: &str = "
CREATE TABLE IF NOT EXISTS attachments (
    id   BLOB PRIMARY KEY,
    blob BLOB NOT NULL
) WITHOUT ROWID;
";

/// Handle to the store database.
pub(crate) struct Database {
    conn: Connection,
}

impl Database {
    /// Creates a new store file. Fails if one already exists.
    pub(crate) fn create(path: &Path) -> Result<Self> {
        create_private_file(path)?;
        let db = Self::connect(Connection::open(path)?)?;
        db.conn.execute_batch(SCHEMA)?;
        Ok(db)
    }

    /// Opens an existing store file.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(VaultError::NotInitialized);
        }
        Self::connect(Connection::open(path)?)
    }

    /// In-memory database (tests).
    #[cfg(test)]
    pub(crate) fn in_memory() -> Result<Self> {
        let db = Self::connect(Connection::open_in_memory()?)?;
        db.conn.execute_batch(SCHEMA)?;
        Ok(db)
    }

    fn connect(conn: Connection) -> Result<Self> {
        // Overwrite deleted content instead of leaving it in free pages.
        conn.execute_batch("PRAGMA secure_delete = ON;")?;
        Ok(Self { conn })
    }

    /// Makes sure the attachments table exists (older stores lack it).
    pub(crate) fn ensure_attachments(&self) -> Result<()> {
        self.conn.execute_batch(ATTACHMENTS)?;
        Ok(())
    }

    /// Stores (or replaces) an attachment's encrypted bytes.
    pub(crate) fn put_attachment(&self, id: &[u8; 16], blob: &[u8]) -> Result<()> {
        self.ensure_attachments()?;
        self.conn.execute(
            "INSERT OR REPLACE INTO attachments (id, blob) VALUES (?1, ?2)",
            params![id.as_slice(), blob],
        )?;
        Ok(())
    }

    /// An attachment's encrypted bytes, if stored.
    pub(crate) fn attachment(&self, id: &[u8; 16]) -> Result<Option<Vec<u8>>> {
        self.ensure_attachments()?;
        Ok(self
            .conn
            .query_row(
                "SELECT blob FROM attachments WHERE id = ?1",
                params![id.as_slice()],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Whether an attachment's bytes are stored (without reading them).
    pub(crate) fn has_attachment(&self, id: &[u8; 16]) -> Result<bool> {
        self.ensure_attachments()?;
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM attachments WHERE id = ?1",
                params![id.as_slice()],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Ids of every stored attachment.
    pub(crate) fn attachment_ids(&self) -> Result<Vec<[u8; 16]>> {
        self.ensure_attachments()?;
        let mut statement = self.conn.prepare("SELECT id FROM attachments")?;
        let ids = statement
            .query_map([], |r| r.get::<_, Vec<u8>>(0))?
            .filter_map(|id| id.ok().and_then(|id| <[u8; 16]>::try_from(id).ok()))
            .collect();
        Ok(ids)
    }

    /// Deletes an attachment's bytes.
    pub(crate) fn delete_attachment(&self, id: &[u8; 16]) -> Result<()> {
        self.ensure_attachments()?;
        self.conn.execute(
            "DELETE FROM attachments WHERE id = ?1",
            params![id.as_slice()],
        )?;
        Ok(())
    }

    /// Runs `f` inside a transaction (all-or-nothing).
    pub(crate) fn in_transaction<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let tx = self.conn.unchecked_transaction()?;
        let out = f()?;
        tx.commit()?;
        Ok(out)
    }

    /// Writes a consistent copy of the whole database to `dest` (owner-only
    /// permissions), replacing any existing file atomically. Works while the
    /// store is open; the copy is exactly as encrypted as the store.
    pub(crate) fn backup_to(&self, dest: &Path) -> Result<()> {
        let mut tmp = dest.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = std::path::PathBuf::from(tmp);
        let tmp_text = tmp
            .to_str()
            .ok_or(VaultError::Corrupt("backup path is not valid UTF-8"))?;
        let _ = std::fs::remove_file(&tmp);
        // VACUUM INTO accepts an existing *empty* file: create it owner-only first.
        crate::private_file::create_private_file(&tmp)?;
        let copied = self
            .conn
            .execute("VACUUM INTO ?1", params![tmp_text])
            .map_err(VaultError::from)
            .and_then(|_| std::fs::rename(&tmp, dest).map_err(VaultError::from));
        if copied.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        copied
    }

    /// Inserts or replaces the header row.
    pub(crate) fn write_header(&self, h: &Header) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO header
             (id, format_version, store_id, kdf_memory_kib, kdf_iterations, kdf_parallelism, salt, wrapped_keyset)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                h.format_version,
                h.store_id.as_slice(),
                h.kdf.memory_kib,
                h.kdf.iterations,
                h.kdf.parallelism,
                h.salt.as_slice(),
                h.wrapped_keyset,
            ],
        )?;
        Ok(())
    }

    /// Reads the header row.
    pub(crate) fn read_header(&self) -> Result<Header> {
        let row = self
            .conn
            .query_row(
                "SELECT format_version, store_id, kdf_memory_kib, kdf_iterations, kdf_parallelism, salt, wrapped_keyset
                 FROM header WHERE id = 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, u32>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        KdfParams { memory_kib: r.get(2)?, iterations: r.get(3)?, parallelism: r.get(4)? },
                        r.get::<_, Vec<u8>>(5)?,
                        r.get::<_, Vec<u8>>(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or(VaultError::NotInitialized)?;
        let (format_version, store_id, kdf, salt, wrapped_keyset) = row;
        Ok(Header {
            format_version,
            store_id: store_id
                .try_into()
                .map_err(|_| VaultError::Corrupt("store id length"))?,
            kdf,
            salt: salt
                .try_into()
                .map_err(|_| VaultError::Corrupt("salt length"))?,
            wrapped_keyset,
        })
    }

    /// Inserts or replaces a record.
    pub(crate) fn put_record(&self, id: &RecordId, blob: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO records (id, blob) VALUES (?1, ?2)",
            params![id.as_slice(), blob],
        )?;
        Ok(())
    }

    /// Deletes a record (no error if absent).
    pub(crate) fn delete_record(&self, id: &RecordId) -> Result<()> {
        self.conn
            .execute("DELETE FROM records WHERE id = ?1", params![id.as_slice()])?;
        Ok(())
    }

    /// All records. Rows with malformed ids are skipped.
    pub(crate) fn records(&self) -> Result<Vec<(RecordId, Vec<u8>)>> {
        let mut stmt = self.conn.prepare("SELECT id, blob FROM records")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, blob) = row?;
            if let Ok(id) = RecordId::try_from(id.as_slice()) {
                out.push((id, blob));
            }
        }
        Ok(out)
    }
}

/// Creates the store file exclusively with owner-only permissions.
fn create_private_file(path: &Path) -> Result<()> {
    match crate::private_file::create_private_file(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(VaultError::AlreadyExists),
        Err(_) => Err(VaultError::NotInitialized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            format_version: 1,
            store_id: [1; 16],
            kdf: KdfParams::DESKTOP,
            salt: [2; 32],
            wrapped_keyset: vec![3; 89],
        }
    }

    #[test]
    fn header_round_trip() {
        let db = Database::in_memory().unwrap();
        db.write_header(&header()).unwrap();
        assert_eq!(db.read_header().unwrap(), header());
    }

    #[test]
    fn missing_header_is_not_initialized() {
        let db = Database::in_memory().unwrap();
        assert!(matches!(db.read_header(), Err(VaultError::NotInitialized)));
    }

    #[test]
    fn corrupt_header_lengths_are_detected() {
        let db = Database::in_memory().unwrap();
        db.write_header(&header()).unwrap();
        db.conn
            .execute("UPDATE header SET store_id = x'00'", [])
            .unwrap();
        assert!(matches!(db.read_header(), Err(VaultError::Corrupt(_))));
        db.write_header(&header()).unwrap();
        db.conn
            .execute("UPDATE header SET salt = x'00'", [])
            .unwrap();
        assert!(matches!(db.read_header(), Err(VaultError::Corrupt(_))));
    }

    #[test]
    fn records_crud() {
        let db = Database::in_memory().unwrap();
        db.put_record(&[1; 16], b"a").unwrap();
        db.put_record(&[2; 16], b"b").unwrap();
        db.put_record(&[1; 16], b"a2").unwrap();
        let mut all = db.records().unwrap();
        all.sort();
        assert_eq!(
            all,
            vec![([1; 16], b"a2".to_vec()), ([2; 16], b"b".to_vec())]
        );
        db.delete_record(&[1; 16]).unwrap();
        db.delete_record(&[9; 16]).unwrap();
        assert_eq!(db.records().unwrap().len(), 1);
    }

    #[test]
    fn records_with_bad_ids_are_skipped() {
        let db = Database::in_memory().unwrap();
        db.conn
            .execute("INSERT INTO records (id, blob) VALUES (x'01', x'02')", [])
            .unwrap();
        assert!(db.records().unwrap().is_empty());
    }

    #[test]
    fn transaction_rolls_back_on_error() {
        let db = Database::in_memory().unwrap();
        let result: Result<()> = db.in_transaction(|| {
            db.put_record(&[1; 16], b"x")?;
            Err(VaultError::Corrupt("boom"))
        });
        assert!(result.is_err());
        assert!(db.records().unwrap().is_empty());
        db.in_transaction(|| db.put_record(&[1; 16], b"x")).unwrap();
        assert_eq!(db.records().unwrap().len(), 1);
    }

    #[test]
    fn create_and_open_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.qlvault");
        assert!(matches!(
            Database::open(&path),
            Err(VaultError::NotInitialized)
        ));
        let db = Database::create(&path).unwrap();
        db.write_header(&header()).unwrap();
        drop(db);
        assert!(matches!(
            Database::create(&path),
            Err(VaultError::AlreadyExists)
        ));
        assert_eq!(
            Database::open(&path).unwrap().read_header().unwrap(),
            header()
        );
    }

    #[cfg(unix)]
    #[test]
    fn store_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.qlvault");
        Database::create(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn create_in_missing_directory_fails() {
        let path = Path::new("/nonexistent-dir-password365/x.qlvault");
        assert!(Database::create(path).is_err());
    }
}

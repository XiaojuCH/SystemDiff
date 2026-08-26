use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use systemdiff_core::{Snapshot, decode_snapshot_document};

const SESSION_PREFIX: &str = "session-v1-";
const OWNERSHIP_MARKER: &str = ".systemdiff-session-v1";
const OWNERSHIP_CONTENTS: &[u8] = b"SystemDiff desktop capture session v1\n";
const ROOT_LOCK_FILE: &str = ".systemdiff-desktop.lock";
const BEFORE_FILE: &str = "before.json";
const AFTER_FILE: &str = "after.json";
const MAX_CREATE_ATTEMPTS: usize = 128;
pub(crate) const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct SessionStorage {
    root: PathBuf,
}

#[derive(Debug)]
pub(crate) struct SessionRootLock {
    _file: File,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionFiles {
    directory: PathBuf,
    before: PathBuf,
    after: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RecoverySummary {
    pub cleaned_sessions: u64,
    pub refused_entries: u64,
}

impl SessionStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn create_session(&self) -> Result<SessionFiles, StorageError> {
        self.ensure_root()?;
        for _ in 0..MAX_CREATE_ATTEMPTS {
            let token = candidate_token();
            match self.create_session_named(&token) {
                Ok(session) => return Ok(session),
                Err(StorageError::Collision) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(StorageError::CreateAttemptsExhausted)
    }

    pub(crate) fn acquire_root_lock(&self) -> Result<SessionRootLock, StorageError> {
        self.ensure_root()?;
        let path = self.root.join(ROOT_LOCK_FILE);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (!metadata.is_file() || is_link_or_reparse(&metadata))
        {
            return Err(StorageError::UnsafePath(
                "single-instance lock is not a plain file",
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| StorageError::Io {
                operation: "open single-instance lock",
                source,
            })?;
        let metadata = file.metadata().map_err(|source| StorageError::Io {
            operation: "inspect opened single-instance lock",
            source,
        })?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(StorageError::UnsafePath(
                "opened single-instance lock is not a plain file",
            ));
        }
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => StorageError::AlreadyRunning,
            fs::TryLockError::Error(source) => StorageError::Io {
                operation: "acquire single-instance lock",
                source,
            },
        })?;
        Ok(SessionRootLock { _file: file })
    }

    pub(crate) fn write_before(
        &self,
        session: &SessionFiles,
        snapshot: &Snapshot,
    ) -> Result<(), StorageError> {
        self.write_snapshot(session, &session.before, BEFORE_FILE, snapshot)
    }

    pub(crate) fn write_after(
        &self,
        session: &SessionFiles,
        snapshot: &Snapshot,
    ) -> Result<(), StorageError> {
        self.write_snapshot(session, &session.after, AFTER_FILE, snapshot)
    }

    pub(crate) fn read_before(&self, session: &SessionFiles) -> Result<Snapshot, StorageError> {
        self.read_snapshot(session, &session.before, BEFORE_FILE)
    }

    pub(crate) fn read_after(&self, session: &SessionFiles) -> Result<Snapshot, StorageError> {
        self.read_snapshot(session, &session.after, AFTER_FILE)
    }

    pub(crate) fn remove_after_if_present(
        &self,
        session: &SessionFiles,
    ) -> Result<(), StorageError> {
        self.validate_session(session)?;
        remove_regular_file_if_present(&session.after)
    }

    pub(crate) fn cleanup_session(&self, session: &SessionFiles) -> Result<(), StorageError> {
        self.validate_session(session)?;
        validate_session_entries(&session.directory)?;

        remove_regular_file_if_present(&session.before)?;
        remove_regular_file_if_present(&session.after)?;
        remove_regular_file_if_present(&session.directory.join(OWNERSHIP_MARKER))?;
        fs::remove_dir(&session.directory).map_err(|source| StorageError::Io {
            operation: "remove session directory",
            source,
        })
    }

    pub(crate) fn recover_stale_sessions(&self) -> Result<RecoverySummary, StorageError> {
        self.ensure_root()?;
        let mut summary = RecoverySummary {
            cleaned_sessions: 0,
            refused_entries: 0,
        };
        let entries = fs::read_dir(&self.root).map_err(|source| StorageError::Io {
            operation: "enumerate session root",
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| StorageError::Io {
                operation: "read session root entry",
                source,
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                summary.refused_entries += 1;
                continue;
            };
            if name == ROOT_LOCK_FILE {
                continue;
            }
            if !valid_session_name(name) {
                summary.refused_entries += 1;
                continue;
            }

            let session = SessionFiles::for_directory(entry.path());
            match self.cleanup_session(&session) {
                Ok(()) => summary.cleaned_sessions += 1,
                Err(_) => summary.refused_entries += 1,
            }
        }
        Ok(summary)
    }

    fn ensure_root(&self) -> Result<(), StorageError> {
        fs::create_dir_all(&self.root).map_err(|source| StorageError::Io {
            operation: "create session root",
            source,
        })?;
        let metadata = fs::symlink_metadata(&self.root).map_err(|source| StorageError::Io {
            operation: "inspect session root",
            source,
        })?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(StorageError::UnsafePath(
                "session root is not a plain directory",
            ));
        }
        Ok(())
    }

    fn create_session_named(&self, token: &str) -> Result<SessionFiles, StorageError> {
        if !valid_session_token(token) {
            return Err(StorageError::UnsafePath("invalid generated session token"));
        }
        let directory = self.root.join(format!("{SESSION_PREFIX}{token}"));
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                return Err(StorageError::Collision);
            }
            Err(source) => {
                return Err(StorageError::Io {
                    operation: "create session directory",
                    source,
                });
            }
        }

        let marker = directory.join(OWNERSHIP_MARKER);
        let result = write_new_file(&marker, OWNERSHIP_CONTENTS, "create ownership marker");
        if let Err(error) = result {
            let _ = fs::remove_dir(&directory);
            return Err(error);
        }
        Ok(SessionFiles::for_directory(directory))
    }

    fn write_snapshot(
        &self,
        session: &SessionFiles,
        path: &Path,
        expected_name: &str,
        snapshot: &Snapshot,
    ) -> Result<(), StorageError> {
        self.validate_file_path(session, path, expected_name)?;
        let bytes = serialize_snapshot(snapshot)?;
        write_new_file(path, &bytes, "write Snapshot")
    }

    fn read_snapshot(
        &self,
        session: &SessionFiles,
        path: &Path,
        expected_name: &str,
    ) -> Result<Snapshot, StorageError> {
        self.validate_file_path(session, path, expected_name)?;
        let bytes = read_bounded_file(path)?;
        decode_snapshot_document(&bytes).map_err(|error| StorageError::InvalidSnapshot {
            details: error.to_string(),
        })
    }

    fn validate_file_path(
        &self,
        session: &SessionFiles,
        path: &Path,
        expected_name: &str,
    ) -> Result<(), StorageError> {
        self.validate_session(session)?;
        if path.parent() != Some(session.directory.as_path())
            || path.file_name().and_then(|name| name.to_str()) != Some(expected_name)
        {
            return Err(StorageError::UnsafePath(
                "session file escaped its allowlist",
            ));
        }
        Ok(())
    }

    fn validate_session(&self, session: &SessionFiles) -> Result<(), StorageError> {
        self.ensure_root()?;
        if session.directory.parent() != Some(self.root.as_path()) {
            return Err(StorageError::UnsafePath(
                "session directory escaped its root",
            ));
        }
        let Some(name) = session.directory.file_name().and_then(|name| name.to_str()) else {
            return Err(StorageError::UnsafePath(
                "session directory has no valid name",
            ));
        };
        if !valid_session_name(name) {
            return Err(StorageError::UnsafePath(
                "session directory name is not owned",
            ));
        }

        let root = self
            .root
            .canonicalize()
            .map_err(|source| StorageError::Io {
                operation: "resolve session root",
                source,
            })?;
        let metadata =
            fs::symlink_metadata(&session.directory).map_err(|source| StorageError::Io {
                operation: "inspect session directory",
                source,
            })?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(StorageError::UnsafePath(
                "session directory is not a plain directory",
            ));
        }
        let resolved = session
            .directory
            .canonicalize()
            .map_err(|source| StorageError::Io {
                operation: "resolve session directory",
                source,
            })?;
        if resolved.parent() != Some(root.as_path()) {
            return Err(StorageError::UnsafePath(
                "resolved session directory escaped its root",
            ));
        }
        verify_marker(&session.directory.join(OWNERSHIP_MARKER))
    }
}

impl SessionFiles {
    fn for_directory(directory: PathBuf) -> Self {
        Self {
            before: directory.join(BEFORE_FILE),
            after: directory.join(AFTER_FILE),
            directory,
        }
    }

    #[cfg(test)]
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
}

fn candidate_token() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{elapsed:x}-{counter:x}", std::process::id())
}

fn valid_session_name(name: &str) -> bool {
    name.strip_prefix(SESSION_PREFIX)
        .is_some_and(valid_session_token)
}

fn valid_session_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 96
        && token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
}

fn write_new_file(path: &Path, bytes: &[u8], operation: &'static str) -> Result<(), StorageError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| StorageError::Io { operation, source })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|source| StorageError::Io { operation, source })
}

fn read_bounded_file(path: &Path) -> Result<Vec<u8>, StorageError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| StorageError::Io {
        operation: "inspect Snapshot",
        source,
    })?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(StorageError::UnsafePath("Snapshot is not a plain file"));
    }
    if metadata.len() > MAX_SNAPSHOT_BYTES {
        return Err(StorageError::SnapshotTooLarge);
    }

    let file = File::open(path).map_err(|source| StorageError::Io {
        operation: "open Snapshot",
        source,
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| StorageError::Io {
            operation: "read Snapshot",
            source,
        })?;
    if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
        return Err(StorageError::SnapshotTooLarge);
    }
    Ok(bytes)
}

fn serialize_snapshot(snapshot: &Snapshot) -> Result<Vec<u8>, StorageError> {
    let mut writer = CappedWriter::new(MAX_SNAPSHOT_BYTES as usize);
    serde_json::to_writer_pretty(&mut writer, snapshot).map_err(|error| {
        if writer.exceeded {
            StorageError::SnapshotTooLarge
        } else {
            StorageError::Serialize(error.to_string())
        }
    })?;
    writer.write_all(b"\n").map_err(|error| {
        if writer.exceeded {
            StorageError::SnapshotTooLarge
        } else {
            StorageError::Io {
                operation: "serialize Snapshot",
                source: error,
            }
        }
    })?;
    Ok(writer.bytes)
}

struct CappedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl CappedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.maximum.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("Snapshot exceeds desktop size limit"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn verify_marker(path: &Path) -> Result<(), StorageError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| StorageError::Io {
        operation: "inspect ownership marker",
        source,
    })?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(StorageError::UnsafePath(
            "ownership marker is not a plain file",
        ));
    }
    let mut contents = Vec::with_capacity(OWNERSHIP_CONTENTS.len() + 1);
    File::open(path)
        .and_then(|file| {
            file.take((OWNERSHIP_CONTENTS.len() + 1) as u64)
                .read_to_end(&mut contents)
        })
        .map_err(|source| StorageError::Io {
            operation: "read ownership marker",
            source,
        })?;
    if contents != OWNERSHIP_CONTENTS {
        return Err(StorageError::UnsafePath(
            "ownership marker contents do not match",
        ));
    }
    Ok(())
}

fn validate_session_entries(directory: &Path) -> Result<(), StorageError> {
    for entry in fs::read_dir(directory).map_err(|source| StorageError::Io {
        operation: "enumerate session directory",
        source,
    })? {
        let entry = entry.map_err(|source| StorageError::Io {
            operation: "read session entry",
            source,
        })?;
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            return Err(StorageError::UnsafePath(
                "session contains a non-Unicode entry",
            ));
        };
        if !matches!(name.as_str(), OWNERSHIP_MARKER | BEFORE_FILE | AFTER_FILE) {
            return Err(StorageError::UnsafePath(
                "session contains an unexpected entry",
            ));
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| StorageError::Io {
            operation: "inspect session entry",
            source,
        })?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(StorageError::UnsafePath(
                "session entry is not a plain file",
            ));
        }
    }
    Ok(())
}

fn remove_regular_file_if_present(path: &Path) -> Result<(), StorageError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(StorageError::Io {
                operation: "inspect session file before cleanup",
                source,
            });
        }
    };
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(StorageError::UnsafePath(
            "cleanup target is not a plain file",
        ));
    }
    fs::remove_file(path).map_err(|source| StorageError::Io {
        operation: "remove session file",
        source,
    })
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[derive(Debug)]
pub(crate) enum StorageError {
    Collision,
    AlreadyRunning,
    CreateAttemptsExhausted,
    SnapshotTooLarge,
    InvalidSnapshot {
        details: String,
    },
    Serialize(String),
    UnsafePath(&'static str),
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Collision => formatter.write_str("session path already exists"),
            Self::AlreadyRunning => {
                formatter.write_str("another SystemDiff desktop instance owns the session root")
            }
            Self::CreateAttemptsExhausted => {
                formatter.write_str("could not allocate a collision-free session directory")
            }
            Self::SnapshotTooLarge => write!(
                formatter,
                "Snapshot exceeds the {} byte desktop limit",
                MAX_SNAPSHOT_BYTES
            ),
            Self::InvalidSnapshot { details } => {
                write!(formatter, "stored Snapshot is invalid: {details}")
            }
            Self::Serialize(details) => {
                write!(formatter, "could not serialize Snapshot: {details}")
            }
            Self::UnsafePath(details) => {
                write!(formatter, "refused unsafe session path: {details}")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "systemdiff-desktop-storage-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory must be unique");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn create_new_rejects_a_collision_without_touching_existing_data() {
        let temporary = TestDirectory::new();
        let storage = SessionStorage::new(temporary.0.join("sessions"));
        storage.ensure_root().expect("root must be created");
        let occupied = storage.root.join("session-v1-a-b-c");
        fs::create_dir(&occupied).expect("collision directory must be created");
        fs::write(occupied.join("keep.txt"), b"keep").expect("sentinel must be created");

        let error = storage
            .create_session_named("a-b-c")
            .expect_err("existing session name must collide");

        assert!(matches!(error, StorageError::Collision));
        assert_eq!(fs::read(occupied.join("keep.txt")).unwrap(), b"keep");
    }

    #[test]
    fn cleanup_refuses_unexpected_entries_and_preserves_the_whole_session() {
        let temporary = TestDirectory::new();
        let storage = SessionStorage::new(temporary.0.join("sessions"));
        storage.ensure_root().unwrap();
        let session = storage.create_session_named("1-2-3").unwrap();
        fs::write(session.directory.join("unexpected.txt"), b"do not delete").unwrap();

        let error = storage.cleanup_session(&session).unwrap_err();

        assert!(matches!(error, StorageError::UnsafePath(_)));
        assert!(session.directory.exists());
        assert!(session.directory.join("unexpected.txt").exists());
    }

    #[test]
    fn cleanup_refuses_a_session_outside_the_configured_root() {
        let temporary = TestDirectory::new();
        let storage = SessionStorage::new(temporary.0.join("sessions"));
        storage.ensure_root().unwrap();
        let outside = temporary.0.join("session-v1-a-b-c");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join(OWNERSHIP_MARKER), OWNERSHIP_CONTENTS).unwrap();
        let session = SessionFiles::for_directory(outside.clone());

        let error = storage.cleanup_session(&session).unwrap_err();

        assert!(matches!(error, StorageError::UnsafePath(_)));
        assert!(outside.exists());
    }

    #[test]
    fn recovery_deletes_only_valid_owned_sessions() {
        let temporary = TestDirectory::new();
        let storage = SessionStorage::new(temporary.0.join("sessions"));
        storage.ensure_root().unwrap();
        let owned = storage.create_session_named("a-b-c").unwrap();
        fs::write(&owned.before, b"snapshot").unwrap();
        let refused = storage.root.join("session-v1-d-e-f");
        fs::create_dir(&refused).unwrap();
        fs::write(refused.join("other"), b"unknown").unwrap();
        fs::write(storage.root.join("notes.txt"), b"unrelated").unwrap();

        let summary = storage.recover_stale_sessions().unwrap();

        assert_eq!(summary.cleaned_sessions, 1);
        assert_eq!(summary.refused_entries, 2);
        assert!(!owned.directory.exists());
        assert!(refused.exists());
        assert!(storage.root.join("notes.txt").exists());
    }

    #[test]
    fn root_lock_refuses_a_second_concurrent_owner() {
        let temporary = TestDirectory::new();
        let storage = SessionStorage::new(temporary.0.join("sessions"));
        let first = storage.acquire_root_lock().unwrap();

        let error = storage.acquire_root_lock().unwrap_err();

        assert!(matches!(error, StorageError::AlreadyRunning));
        drop(first);
        storage
            .acquire_root_lock()
            .expect("lock must be available after the first owner exits");
    }
}

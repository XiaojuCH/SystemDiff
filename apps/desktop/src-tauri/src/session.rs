use crate::storage::{
    RecoverySummary, SessionFiles, SessionRootLock, SessionStorage, StorageError,
};
use serde::Serialize;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use systemdiff_core::Snapshot;
use systemdiff_diff::{DiffOptions, diff_snapshots};
use systemdiff_report::presentation::{DesktopPresentation, build_desktop_presentation};
use systemdiff_report::render_technical;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub trait SnapshotSource: Send + Sync {
    fn capture(&self) -> Result<Snapshot, CaptureFailure>;
}

#[derive(Debug, Default)]
pub struct SystemSnapshotSource;

impl SnapshotSource for SystemSnapshotSource {
    fn capture(&self) -> Result<Snapshot, CaptureFailure> {
        let captured_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|error| CaptureFailure::new(error.to_string()))?;
        systemdiff_windows::capture_snapshot(captured_at, env!("CARGO_PKG_VERSION").to_owned())
            .map_err(|error| match error {
                systemdiff_windows::CaptureError::UnsupportedPlatform => {
                    CaptureFailure::unsupported_windows(error.to_string())
                }
                systemdiff_windows::CaptureError::InvalidSnapshot(_) => {
                    CaptureFailure::invalid_snapshot(error.to_string())
                }
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStage {
    Ready,
    Starting,
    Capturing,
    Finishing,
    Results,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppViewState {
    pub stage: SessionStage,
    pub presentation: Option<DesktopPresentation>,
    pub cleanup_pending: bool,
}

#[derive(Debug, Clone)]
struct CaptureResult {
    presentation: DesktopPresentation,
    technical_details: String,
    cleanup_pending: Option<SessionFiles>,
}

#[derive(Debug)]
enum InternalState {
    Ready,
    Starting,
    Capturing(SessionFiles),
    Finishing,
    Results(CaptureResult),
}

pub struct SessionService {
    storage: SessionStorage,
    _root_lock: SessionRootLock,
    source: Arc<dyn SnapshotSource>,
    state: Mutex<InternalState>,
    startup_recovery: RecoverySummary,
}

#[derive(Clone)]
pub struct DesktopBackendState {
    service: Result<Arc<SessionService>, AppError>,
}

impl DesktopBackendState {
    pub fn from_result(service: Result<SessionService, AppError>) -> Self {
        Self {
            service: service.map(Arc::new),
        }
    }

    pub fn failed(error: AppError) -> Self {
        Self {
            service: Err(error),
        }
    }

    pub fn service(&self) -> Result<Arc<SessionService>, AppError> {
        self.service.clone()
    }

    pub fn session_state(&self) -> Result<AppViewState, AppError> {
        self.service().map(|service| service.session_state())
    }

    pub fn shutdown_cleanup(&self) -> Result<ShutdownCleanup, AppError> {
        match &self.service {
            Ok(service) => service.shutdown_cleanup(),
            Err(_) => Ok(ShutdownCleanup::Complete),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownCleanup {
    Complete,
    DeferredBusy,
}

impl SessionService {
    pub fn production(storage: SessionStorage) -> Result<Self, AppError> {
        Self::new(storage, Arc::new(SystemSnapshotSource))
    }

    pub fn new(storage: SessionStorage, source: Arc<dyn SnapshotSource>) -> Result<Self, AppError> {
        let root_lock = storage.acquire_root_lock().map_err(|error| match error {
            StorageError::AlreadyRunning => AppError::another_instance(),
            error => AppError::bootstrap_storage(error.to_string()),
        })?;
        let startup_recovery = storage
            .recover_stale_sessions()
            .map_err(|error| AppError::bootstrap_storage(error.to_string()))?;
        Ok(Self {
            storage,
            _root_lock: root_lock,
            source,
            state: Mutex::new(InternalState::Ready),
            startup_recovery,
        })
    }

    pub fn session_state(&self) -> AppViewState {
        view_state(&self.lock_state())
    }

    pub fn technical_details(&self) -> Result<String, AppError> {
        let state = self.lock_state();
        match &*state {
            InternalState::Results(result) => Ok(result.technical_details.clone()),
            _ => Err(AppError::invalid_state(
                "get_technical_details",
                stage_of(&state),
            )),
        }
    }

    pub fn start_capture(&self) -> Result<AppViewState, AppError> {
        {
            let mut state = self.lock_state();
            match &*state {
                InternalState::Ready => {}
                _ => return Err(AppError::busy("start_capture")),
            }
            *state = InternalState::Starting;
        }

        let result = self.start_capture_work();
        let mut state = self.lock_state();
        match result {
            Ok(session) => {
                *state = InternalState::Capturing(session);
                Ok(view_state(&state))
            }
            Err(error) => {
                *state = InternalState::Ready;
                Err(error)
            }
        }
    }

    pub fn finish_capture(&self) -> Result<AppViewState, AppError> {
        let session = {
            let mut state = self.lock_state();
            let InternalState::Capturing(session) = &*state else {
                return Err(AppError::invalid_state("finish_capture", stage_of(&state)));
            };
            let session = session.clone();
            *state = InternalState::Finishing;
            session
        };

        let result = self.finish_capture_work(&session);
        let mut state = self.lock_state();
        match result {
            Ok(result) => {
                *state = InternalState::Results(result);
                Ok(view_state(&state))
            }
            Err(error) => {
                *state = InternalState::Capturing(session);
                Err(error)
            }
        }
    }

    pub fn cancel_capture(&self) -> Result<AppViewState, AppError> {
        enum CancelTarget {
            Capture(SessionFiles),
            Results {
                session: SessionFiles,
                result: Box<CaptureResult>,
            },
        }

        let target = {
            let mut state = self.lock_state();
            match &*state {
                InternalState::Results(result) if result.cleanup_pending.is_none() => {
                    *state = InternalState::Ready;
                    return Ok(view_state(&state));
                }
                InternalState::Results(result) => {
                    let result = result.clone();
                    let session = result
                        .cleanup_pending
                        .clone()
                        .expect("guarded by the preceding cleanup-pending check");
                    *state = InternalState::Finishing;
                    CancelTarget::Results {
                        session,
                        result: Box::new(result),
                    }
                }
                InternalState::Capturing(session) => {
                    let session = session.clone();
                    *state = InternalState::Finishing;
                    CancelTarget::Capture(session)
                }
                _ => {
                    return Err(AppError::invalid_state("cancel_capture", stage_of(&state)));
                }
            }
        };

        let session = match &target {
            CancelTarget::Capture(session) | CancelTarget::Results { session, .. } => session,
        };
        match self.storage.cleanup_session(session) {
            Ok(()) => {
                let mut state = self.lock_state();
                *state = InternalState::Ready;
                Ok(view_state(&state))
            }
            Err(error) => {
                let mut state = self.lock_state();
                *state = match target {
                    CancelTarget::Capture(session) => InternalState::Capturing(session),
                    CancelTarget::Results { result, .. } => InternalState::Results(*result),
                };
                Err(AppError::session_cleanup(error))
            }
        }
    }

    /// Best-effort cleanup used by the normal desktop application exit path.
    ///
    /// A stable Capturing session or a Results session with pending cleanup is
    /// removed through the same exact allowlist as explicit cancellation. An
    /// in-flight native capture/diff cannot be interrupted safely, so it is
    /// deliberately left for conservative startup recovery.
    pub fn shutdown_cleanup(&self) -> Result<ShutdownCleanup, AppError> {
        match self.session_state().stage {
            SessionStage::Ready => Ok(ShutdownCleanup::Complete),
            SessionStage::Starting | SessionStage::Finishing => Ok(ShutdownCleanup::DeferredBusy),
            SessionStage::Capturing | SessionStage::Results => {
                self.cancel_capture()?;
                Ok(ShutdownCleanup::Complete)
            }
        }
    }

    fn start_capture_work(&self) -> Result<SessionFiles, AppError> {
        let session = self
            .storage
            .create_session()
            .map_err(AppError::session_storage)?;
        let result = self
            .source
            .capture()
            .map_err(AppError::capture)
            .and_then(|snapshot| {
                self.storage
                    .write_before(&session, &snapshot)
                    .map_err(AppError::session_storage)
            });
        if let Err(error) = result {
            return match self.storage.cleanup_session(&session) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(error.with_cleanup_failure(cleanup)),
            };
        }
        Ok(session)
    }

    fn finish_capture_work(&self, session: &SessionFiles) -> Result<CaptureResult, AppError> {
        let result = (|| {
            let before = self
                .storage
                .read_before(session)
                .map_err(AppError::session_storage)?;
            let after = self.source.capture().map_err(AppError::capture)?;
            self.storage
                .write_after(session, &after)
                .map_err(AppError::session_storage)?;
            let after = self
                .storage
                .read_after(session)
                .map_err(AppError::session_storage)?;
            let diff = diff_snapshots(&before, &after, DiffOptions::default())
                .map_err(|error| AppError::diff(error.to_string()))?;
            let presentation = build_desktop_presentation(&diff);
            let technical = render_technical(&diff, &before, &after);
            let technical_details = format!(
                concat!(
                    "Desktop session\n",
                    "  startup sessions cleaned: {}\n",
                    "  startup entries refused: {}\n",
                    "  Snapshot size limit: {} bytes\n",
                    "  session evidence cleanup: required before Results\n\n",
                    "{}"
                ),
                self.startup_recovery.cleaned_sessions,
                self.startup_recovery.refused_entries,
                crate::storage::MAX_SNAPSHOT_BYTES,
                technical
            );
            Ok(CaptureResult {
                presentation,
                technical_details,
                cleanup_pending: None,
            })
        })();

        match result {
            Ok(mut result) => {
                match self.storage.cleanup_session(session) {
                    Ok(()) => result
                        .technical_details
                        .push_str("\nDesktop session evidence cleanup: complete\n"),
                    Err(error) => {
                        result.technical_details.push_str(&format!(
                            concat!(
                                "\nWARNING: desktop session evidence cleanup did not complete.\n",
                                "The owned app-local session will be considered for conservative cleanup during the next startup; suspicious entries are never deleted.\n",
                                "Cleanup error: {}\n"
                            ),
                            error
                        ));
                        result.cleanup_pending = Some(session.clone());
                    }
                }
                Ok(result)
            }
            Err(error) => match self.storage.remove_after_if_present(session) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(error.with_cleanup_failure(cleanup)),
            },
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, InternalState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn view_state(state: &InternalState) -> AppViewState {
    match state {
        InternalState::Ready => AppViewState {
            stage: SessionStage::Ready,
            presentation: None,
            cleanup_pending: false,
        },
        InternalState::Starting => AppViewState {
            stage: SessionStage::Starting,
            presentation: None,
            cleanup_pending: false,
        },
        InternalState::Capturing(_) => AppViewState {
            stage: SessionStage::Capturing,
            presentation: None,
            cleanup_pending: false,
        },
        InternalState::Finishing => AppViewState {
            stage: SessionStage::Finishing,
            presentation: None,
            cleanup_pending: false,
        },
        InternalState::Results(result) => AppViewState {
            stage: SessionStage::Results,
            presentation: Some(result.presentation.clone()),
            cleanup_pending: result.cleanup_pending.is_some(),
        },
    }
}

fn stage_of(state: &InternalState) -> SessionStage {
    view_state(state).stage
}

#[derive(Debug, Clone)]
pub struct CaptureFailure {
    kind: CaptureFailureKind,
    details: String,
}

#[derive(Debug, Clone, Copy)]
enum CaptureFailureKind {
    Generic,
    UnsupportedWindows,
    InvalidSnapshot,
}

impl CaptureFailure {
    pub fn new(details: String) -> Self {
        Self {
            kind: CaptureFailureKind::Generic,
            details,
        }
    }

    fn unsupported_windows(details: String) -> Self {
        Self {
            kind: CaptureFailureKind::UnsupportedWindows,
            details,
        }
    }

    fn invalid_snapshot(details: String) -> Self {
        Self {
            kind: CaptureFailureKind::InvalidSnapshot,
            details,
        }
    }
}

impl fmt::Display for CaptureFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.details)
    }
}

impl Error for CaptureFailure {}

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message_id: String,
    pub technical_details: String,
}

impl AppError {
    fn new(code: &'static str, message_id: &'static str, technical_details: String) -> Self {
        Self {
            code: code.to_owned(),
            message_id: message_id.to_owned(),
            technical_details,
        }
    }

    fn busy(command: &'static str) -> Self {
        Self::new(
            "session_busy",
            "error.session_busy",
            format!("{command} was refused because another session operation is active"),
        )
    }

    fn invalid_state(command: &'static str, stage: SessionStage) -> Self {
        Self::new(
            "invalid_session_state",
            "error.invalid_session_state",
            format!("{command} is not valid while the session stage is {stage:?}"),
        )
    }

    fn capture(error: CaptureFailure) -> Self {
        let (code, message_id) = match error.kind {
            CaptureFailureKind::Generic => ("capture_failed", "error.capture_failed"),
            CaptureFailureKind::UnsupportedWindows => {
                ("unsupported_windows", "error.unsupported_windows")
            }
            CaptureFailureKind::InvalidSnapshot => ("invalid_snapshot", "error.invalid_snapshot"),
        };
        Self::new(code, message_id, error.to_string())
    }

    fn session_storage(error: StorageError) -> Self {
        match error {
            StorageError::InvalidSnapshot { details } => Self::new(
                "invalid_snapshot",
                "error.invalid_snapshot",
                format!("stored Snapshot is invalid: {details}"),
            ),
            error => Self::new(
                "session_storage_failed",
                "error.session_storage_failed",
                error.to_string(),
            ),
        }
    }

    fn session_cleanup(error: StorageError) -> Self {
        Self::new(
            "session_cleanup_failed",
            "error.session_cleanup_failed",
            error.to_string(),
        )
    }

    fn another_instance() -> Self {
        Self::new(
            "another_instance_running",
            "error.another_instance_running",
            "another SystemDiff desktop instance owns the app-local session root".to_owned(),
        )
    }

    fn diff(details: String) -> Self {
        Self::new("diff_failed", "error.diff_failed", details)
    }

    pub fn background_task(details: String) -> Self {
        Self::new(
            "background_task_failed",
            "error.background_task_failed",
            details,
        )
    }

    pub fn bootstrap_storage(details: String) -> Self {
        Self::new(
            "bootstrap_storage_failed",
            "error.bootstrap_storage_failed",
            details,
        )
    }

    fn with_cleanup_failure(mut self, cleanup: StorageError) -> Self {
        self.technical_details = format!(
            "{}; cleanup also failed: {}",
            self.technical_details, cleanup
        );
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({}): {}",
            self.code, self.message_id, self.technical_details
        )
    }
}

impl Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use systemdiff_core::{
        CollectorRun, CollectorStatus, HostMetadata, PrivilegeState, RedactionMetadata,
        RedactionStatus, ScopeCoverage, Snapshot,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "systemdiff-desktop-session-test-{}-{id}",
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

    struct FakeSource {
        results: Mutex<VecDeque<Result<Snapshot, CaptureFailure>>>,
    }

    impl FakeSource {
        fn new(results: Vec<Result<Snapshot, CaptureFailure>>) -> Self {
            Self {
                results: Mutex::new(results.into()),
            }
        }
    }

    impl SnapshotSource for FakeSource {
        fn capture(&self) -> Result<Snapshot, CaptureFailure> {
            self.results
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake capture result must exist")
        }
    }

    fn snapshot(captured_at: &str) -> Snapshot {
        Snapshot {
            document_type: "systemdiff.snapshot".to_owned(),
            schema_version: 1,
            systemdiff_version: "0.0.0-test".to_owned(),
            captured_at: captured_at.to_owned(),
            host: HostMetadata {
                windows_version: None,
                windows_build: None,
                architecture: Some("x86_64".to_owned()),
            },
            privilege: PrivilegeState::StandardUser,
            enabled_collectors: vec!["test.collector".to_owned()],
            collectors: vec![CollectorRun {
                id: "test.collector".to_owned(),
                version: 1,
                status: CollectorStatus::Complete,
                coverage: vec![ScopeCoverage {
                    scope_id: "test.scope".to_owned(),
                    status: CollectorStatus::Complete,
                }],
                diagnostics: Vec::new(),
            }],
            redaction: RedactionMetadata {
                status: RedactionStatus::Unredacted,
                policy: None,
            },
            observations: Vec::new(),
        }
    }

    fn service_with(
        temporary: &TestDirectory,
        results: Vec<Result<Snapshot, CaptureFailure>>,
    ) -> SessionService {
        SessionService::new(
            SessionStorage::new(temporary.0.join("sessions")),
            Arc::new(FakeSource::new(results)),
        )
        .unwrap()
    }

    fn session_directory_count(temporary: &TestDirectory) -> usize {
        fs::read_dir(temporary.0.join("sessions"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("session-v1-"))
            })
            .count()
    }

    #[test]
    fn successful_start_finish_and_cleanup_reach_results() {
        let temporary = TestDirectory::new();
        let service = service_with(
            &temporary,
            vec![
                Ok(snapshot("2026-08-20T00:00:00Z")),
                Ok(snapshot("2026-08-20T00:05:00Z")),
            ],
        );

        assert_eq!(
            service.start_capture().unwrap().stage,
            SessionStage::Capturing
        );
        let state = service.finish_capture().unwrap();

        assert_eq!(state.stage, SessionStage::Results);
        assert!(state.presentation.is_some());
        assert!(!state.cleanup_pending);
        assert_eq!(session_directory_count(&temporary), 0);
        let technical = service.technical_details().unwrap();
        assert!(technical.contains("SystemDiff technical diff"));
        assert!(technical.contains("Desktop session evidence cleanup: complete"));
        assert_eq!(service.cancel_capture().unwrap().stage, SessionStage::Ready);
        assert!(service.technical_details().is_err());
    }

    #[test]
    fn cancel_discards_the_owned_session_and_returns_ready() {
        let temporary = TestDirectory::new();
        let service = service_with(&temporary, vec![Ok(snapshot("2026-08-20T00:00:00Z"))]);
        service.start_capture().unwrap();

        let state = service.cancel_capture().unwrap();

        assert_eq!(state.stage, SessionStage::Ready);
        assert_eq!(session_directory_count(&temporary), 0);
    }

    #[test]
    fn start_failure_restores_ready_and_cleans_the_session() {
        let temporary = TestDirectory::new();
        let service = service_with(
            &temporary,
            vec![Err(CaptureFailure::new("synthetic failure".to_owned()))],
        );

        let error = service.start_capture().unwrap_err();

        assert_eq!(error.code, "capture_failed");
        assert_eq!(service.session_state().stage, SessionStage::Ready);
        assert_eq!(session_directory_count(&temporary), 0);
    }

    #[test]
    fn finish_failure_keeps_before_evidence_available_for_retry_or_cancel() {
        let temporary = TestDirectory::new();
        let service = service_with(
            &temporary,
            vec![
                Ok(snapshot("2026-08-20T00:00:00Z")),
                Err(CaptureFailure::new("synthetic failure".to_owned())),
            ],
        );
        service.start_capture().unwrap();

        let error = service.finish_capture().unwrap_err();

        assert_eq!(error.code, "capture_failed");
        assert_eq!(service.session_state().stage, SessionStage::Capturing);
        assert_eq!(session_directory_count(&temporary), 1);
        assert_eq!(service.cancel_capture().unwrap().stage, SessionStage::Ready);
    }

    #[test]
    fn duplicate_and_out_of_order_commands_are_rejected() {
        let temporary = TestDirectory::new();
        let service = service_with(&temporary, vec![Ok(snapshot("2026-08-20T00:00:00Z"))]);

        assert_eq!(
            service.finish_capture().unwrap_err().code,
            "invalid_session_state"
        );
        service.start_capture().unwrap();
        assert_eq!(service.start_capture().unwrap_err().code, "session_busy");
    }

    #[test]
    fn completed_results_survive_a_conservative_cleanup_refusal() {
        let temporary = TestDirectory::new();
        let service = service_with(
            &temporary,
            vec![
                Ok(snapshot("2026-08-20T00:00:00Z")),
                Ok(snapshot("2026-08-20T00:05:00Z")),
            ],
        );
        service.start_capture().unwrap();
        let session = {
            let state = service.lock_state();
            let InternalState::Capturing(session) = &*state else {
                panic!("session must be capturing");
            };
            session.clone()
        };
        fs::write(
            session.directory().join("unexpected.txt"),
            b"refuse cleanup",
        )
        .unwrap();

        let state = service.finish_capture().unwrap();

        assert_eq!(state.stage, SessionStage::Results);
        assert!(state.cleanup_pending);
        assert!(
            service
                .technical_details()
                .unwrap()
                .contains("WARNING: desktop session evidence cleanup did not complete")
        );
        assert!(session.directory().exists());
        assert_eq!(
            service.cancel_capture().unwrap_err().code,
            "session_cleanup_failed"
        );
        assert_eq!(service.session_state().stage, SessionStage::Results);
        fs::remove_file(session.directory().join("unexpected.txt")).unwrap();
        assert_eq!(
            service.shutdown_cleanup().unwrap(),
            ShutdownCleanup::Complete
        );
        assert_eq!(service.session_state().stage, SessionStage::Ready);
        assert!(!session.directory().exists());
    }

    #[test]
    fn typed_capture_and_stored_snapshot_failures_keep_stable_message_ids() {
        let generic = AppError::capture(CaptureFailure::new("generic".to_owned()));
        let unsupported = AppError::capture(CaptureFailure::unsupported_windows(
            "unsupported".to_owned(),
        ));
        let captured_invalid = AppError::capture(CaptureFailure::invalid_snapshot(
            "invalid capture".to_owned(),
        ));
        let stored_invalid = AppError::session_storage(StorageError::InvalidSnapshot {
            details: "invalid stored evidence".to_owned(),
        });

        assert_eq!(
            (generic.code.as_str(), generic.message_id.as_str()),
            ("capture_failed", "error.capture_failed")
        );
        assert_eq!(
            (unsupported.code.as_str(), unsupported.message_id.as_str()),
            ("unsupported_windows", "error.unsupported_windows")
        );
        assert_eq!(
            (
                captured_invalid.code.as_str(),
                captured_invalid.message_id.as_str()
            ),
            ("invalid_snapshot", "error.invalid_snapshot")
        );
        assert_eq!(
            (
                stored_invalid.code.as_str(),
                stored_invalid.message_id.as_str()
            ),
            ("invalid_snapshot", "error.invalid_snapshot")
        );
    }

    #[test]
    fn managed_bootstrap_failure_is_returned_by_commands_without_a_service() {
        let backend = DesktopBackendState::failed(AppError::another_instance());

        let state_error = backend.session_state().unwrap_err();
        let Err(service_error) = backend.service() else {
            panic!("bootstrap failure must not expose a SessionService");
        };

        assert_eq!(state_error.code, "another_instance_running");
        assert_eq!(state_error.message_id, "error.another_instance_running");
        assert_eq!(service_error.code, "another_instance_running");
        assert_eq!(
            backend.shutdown_cleanup().unwrap(),
            ShutdownCleanup::Complete
        );
    }

    #[test]
    fn storage_initialization_failure_is_cached_as_restart_required() {
        let temporary = TestDirectory::new();
        let non_directory = temporary.0.join("not-a-directory");
        fs::write(&non_directory, b"blocks session root creation").unwrap();
        let initialization = SessionService::new(
            SessionStorage::new(non_directory.join("sessions")),
            Arc::new(FakeSource::new(Vec::new())),
        );
        let backend = DesktopBackendState::from_result(initialization);

        let state_error = backend.session_state().unwrap_err();
        let Err(service_error) = backend.service() else {
            panic!("bootstrap storage failure must not expose a SessionService");
        };

        assert_eq!(state_error.code, "bootstrap_storage_failed");
        assert_eq!(state_error.message_id, "error.bootstrap_storage_failed");
        assert_eq!(service_error.code, "bootstrap_storage_failed");
        assert_eq!(service_error.message_id, "error.bootstrap_storage_failed");
    }

    #[test]
    fn normal_shutdown_cleans_a_stable_capturing_session() {
        let temporary = TestDirectory::new();
        let service = service_with(&temporary, vec![Ok(snapshot("2026-08-20T00:00:00Z"))]);
        service.start_capture().unwrap();

        let outcome = service.shutdown_cleanup().unwrap();

        assert_eq!(outcome, ShutdownCleanup::Complete);
        assert_eq!(service.session_state().stage, SessionStage::Ready);
        assert_eq!(session_directory_count(&temporary), 0);
    }

    #[test]
    fn busy_shutdown_is_deferred_for_startup_recovery_without_claiming_cleanup() {
        let temporary = TestDirectory::new();
        let service = service_with(&temporary, Vec::new());
        *service.lock_state() = InternalState::Starting;

        assert_eq!(
            service.shutdown_cleanup().unwrap(),
            ShutdownCleanup::DeferredBusy
        );
        assert_eq!(service.session_state().stage, SessionStage::Starting);
    }
}

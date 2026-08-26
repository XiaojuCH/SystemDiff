#![forbid(unsafe_code)]

mod session;
mod storage;

pub use session::{
    AppError, AppViewState, DesktopBackendState, SessionService, SessionStage, ShutdownCleanup,
    SystemSnapshotSource,
};
pub use storage::{RecoverySummary, SessionStorage};

#[cfg(windows)]
mod tauri_app {
    use super::{AppError, AppViewState, DesktopBackendState, SessionService, SessionStorage};
    use tauri::{Manager, State};

    #[tauri::command]
    fn get_session_state(state: State<'_, DesktopBackendState>) -> Result<AppViewState, AppError> {
        state.session_state()
    }

    #[tauri::command]
    async fn start_capture(
        state: State<'_, DesktopBackendState>,
    ) -> Result<AppViewState, AppError> {
        let service = state.service()?;
        tauri::async_runtime::spawn_blocking(move || service.start_capture())
            .await
            .map_err(|error| AppError::background_task(error.to_string()))?
    }

    #[tauri::command]
    async fn finish_capture(
        state: State<'_, DesktopBackendState>,
    ) -> Result<AppViewState, AppError> {
        let service = state.service()?;
        tauri::async_runtime::spawn_blocking(move || service.finish_capture())
            .await
            .map_err(|error| AppError::background_task(error.to_string()))?
    }

    #[tauri::command]
    async fn cancel_capture(
        state: State<'_, DesktopBackendState>,
    ) -> Result<AppViewState, AppError> {
        let service = state.service()?;
        tauri::async_runtime::spawn_blocking(move || service.cancel_capture())
            .await
            .map_err(|error| AppError::background_task(error.to_string()))?
    }

    #[tauri::command]
    fn get_technical_details(state: State<'_, DesktopBackendState>) -> Result<String, AppError> {
        state.service()?.technical_details()
    }

    pub(super) fn run() {
        tauri::Builder::default()
            .setup(|app| {
                let backend = match app.path().app_local_data_dir() {
                    Ok(app_local_data) => {
                        DesktopBackendState::from_result(SessionService::production(
                            SessionStorage::new(app_local_data.join("capture-sessions-v1")),
                        ))
                    }
                    Err(error) => DesktopBackendState::failed(AppError::bootstrap_storage(
                        format!("could not resolve app-local data directory: {error}"),
                    )),
                };
                app.manage(backend);
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                get_session_state,
                start_capture,
                finish_capture,
                cancel_capture,
                get_technical_details
            ])
            .build(tauri::generate_context!())
            .expect("failed to build the SystemDiff desktop application")
            .run(|app_handle, event| {
                if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                    let backend = app_handle.state::<DesktopBackendState>();
                    let _ = backend.shutdown_cleanup();
                }
            });
    }
}

#[cfg(windows)]
pub fn run() {
    tauri_app::run();
}

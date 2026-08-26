const COMMANDS: &[&str] = &[
    "get_session_state",
    "start_capture",
    "finish_capture",
    "cancel_capture",
    "get_technical_details",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    tauri_build::try_build(attributes).expect("failed to prepare the Tauri application");
}

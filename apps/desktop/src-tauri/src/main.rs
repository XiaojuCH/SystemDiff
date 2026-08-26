#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    systemdiff_desktop::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The SystemDiff desktop application requires Windows.");
}

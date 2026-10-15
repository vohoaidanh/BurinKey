// Hides the console window in release builds; debug builds keep it so
// `eprintln!` diagnostics are visible while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> platform_win::Result<()> {
    // Autostart at sign-in passes this flag so the panel stays hidden.
    let start_in_tray = std::env::args().any(|arg| arg == platform_win::TRAY_FLAG);
    platform_win::run(start_in_tray)
}

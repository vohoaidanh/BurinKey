//! Windows integration: keyboard hook, text injection, the tray icon and the
//! settings panel.
//!
//! This crate contains every `unsafe` WinAPI call in the project. The typing
//! logic itself lives in the platform-independent `engine` crate.

mod autostart;
mod hook;
mod icon;
mod inject;
mod settings;
mod tray;

pub use autostart::TRAY_FLAG;

/// The name shown to users (window titles, tray tooltip, "Thông tin").
pub const APP_NAME: &str = "Burin Smart Key";

pub use engine::InputMode;
pub use windows::core::{Error, Result};

use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

/// Installs the keyboard hook and tray icon, opens the settings panel unless
/// `start_in_tray`, then runs the Win32 message loop until the user exits
/// (tray menu "Thoát" or `WM_DESTROY`).
pub fn run(start_in_tray: bool) -> Result<()> {
    // Crisp rendering on high-DPI screens; must precede any window creation.
    // SAFETY: a process-wide setting with no pointer arguments.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let _hook = hook::KeyboardHook::install()?;
    let _tray = tray::TrayIcon::create()?;
    if !start_in_tray {
        settings::show()?;
    }

    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a valid, exclusively-owned `MSG` for the duration
        // of this call; `None` means "any window owned by this thread."
        let has_message = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if !has_message.as_bool() {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

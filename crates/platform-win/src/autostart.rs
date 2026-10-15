//! "Khởi động cùng Windows": a value under the current user's `Run` key that
//! launches viet-ime straight into the tray at sign-in.

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: PCWSTR = w!("viet-ime");

/// Command-line flag that starts the app hidden in the tray.
pub const TRAY_FLAG: &str = "--tray";

pub fn is_enabled() -> bool {
    // SAFETY: a null data pointer only queries whether the value exists.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    status == ERROR_SUCCESS
}

pub fn set_enabled(enabled: bool) {
    // SAFETY: every pointer refers to a live, null-terminated wide string and
    // `cbdata` is that string's size in bytes, terminator included.
    unsafe {
        if !enabled {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME);
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let command = HSTRING::from(format!("\"{}\" {TRAY_FLAG}", exe.display()));
        let bytes = (command.len() + 1) * std::mem::size_of::<u16>();
        let _ = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            REG_SZ.0,
            Some(command.as_ptr().cast()),
            bytes as u32,
        );
    }
}

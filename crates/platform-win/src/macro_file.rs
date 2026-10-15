//! The gõ tắt list on disk: `%APPDATA%\BurinSmartKey\macros.txt`, one
//! `short = full text` per line. "Bảng gõ tắt…" opens it in Notepad; saving
//! it there is enough — the app notices the change and reloads it while
//! typing, no restart needed.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use engine::Macros;
use windows::core::{w, HSTRING};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::hook;

/// Written the first time, so the feature shows what it does.
const STARTER_LIST: &str = "\
# Bảng gõ tắt của Burin Smart Key
# Mỗi dòng: từ tắt = cụm từ đầy đủ. Dòng bắt đầu bằng # là ghi chú.
# Gõ từ tắt rồi dấu cách hoặc dấu câu để bung ra; viết hoa theo cách gõ
# (vn -> Việt Nam, VN -> VIỆT NAM). Lưu tệp là có hiệu lực ngay.

vn = Việt Nam
hcm = Hồ Chí Minh
tphcm = Thành phố Hồ Chí Minh
";

/// How often typing checks whether the file changed.
const CHECK_EVERY: Duration = Duration::from_secs(2);

thread_local! {
    static LOADED_AT: Cell<Option<SystemTime>> = const { Cell::new(None) };
    static LAST_CHECK: Cell<Option<Instant>> = const { Cell::new(None) };
}

fn path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("BurinSmartKey")
            .join("macros.txt"),
    )
}

fn modified(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Creates the starter list if there is none, then loads it into the engine.
pub fn load() {
    let Some(path) = path() else {
        return;
    };
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, STARTER_LIST);
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    hook::with_engine(|e| e.set_macros(Macros::parse(&text)));
    LOADED_AT.with(|t| t.set(modified(&path)));
}

/// Reloads the list if the file changed since it was loaded. Cheap enough
/// to call on every keystroke: it looks at the file at most every
/// [`CHECK_EVERY`].
pub fn reload_if_changed() {
    let now = Instant::now();
    let due = LAST_CHECK.with(|t| t.get().is_none_or(|last| now - last >= CHECK_EVERY));
    if !due {
        return;
    }
    LAST_CHECK.with(|t| t.set(Some(now)));
    let Some(path) = path() else {
        return;
    };
    if modified(&path) != LOADED_AT.with(Cell::get) {
        load();
    }
}

/// "Bảng gõ tắt…": opens the list in Notepad.
pub fn open_in_editor() {
    load(); // make sure the file exists
    let Some(path) = path() else {
        return;
    };
    let file = HSTRING::from(path.as_os_str());
    // SAFETY: all strings are live, null-terminated wide strings.
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("notepad.exe"),
            &file,
            None,
            SW_SHOWNORMAL,
        );
    }
}

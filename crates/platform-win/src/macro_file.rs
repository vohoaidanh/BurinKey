//! The gõ tắt list on disk: `%APPDATA%\BurinSmartKey\macros.txt`, one
//! `short = full text` per line. The "Bảng gõ tắt" window edits it; people
//! may also edit the file by hand — the app notices the change and reloads
//! it while typing, no restart needed.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use engine::Macros;

use crate::hook;

/// Comment lines at the top of the file.
const HEADER: &str = "\
# Bảng gõ tắt của Burin Smart Key
# Mỗi dòng: từ tắt = cụm từ đầy đủ. Dòng bắt đầu bằng # là ghi chú.
# Gõ từ tắt rồi dấu cách hoặc dấu câu để bung ra; viết hoa theo cách gõ
# (vn -> Việt Nam, VN -> VIỆT NAM).

";

/// The first list, so the feature shows what it does.
const STARTER: &[(&str, &str)] = &[
    ("vn", "Việt Nam"),
    ("hcm", "Hồ Chí Minh"),
    ("tphcm", "Thành phố Hồ Chí Minh"),
];

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

/// The saved list (the starter list if there is no file yet).
pub fn read() -> Macros {
    match path().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(text) => Macros::parse(&text),
        None => {
            let mut macros = Macros::default();
            for (key, full) in STARTER {
                macros.insert(key, full);
            }
            macros
        }
    }
}

/// Saves `macros` and puts them to work right away.
pub fn save(macros: &Macros) {
    if let Some(path) = path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Write then rename, so a crash mid-write can't leave a half file.
        let tmp = path.with_extension("txt.tmp");
        if std::fs::write(&tmp, macros.to_text(HEADER)).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        LOADED_AT.with(|t| t.set(modified(&path)));
    }
    let macros = macros.clone();
    hook::with_engine(|e| e.set_macros(macros));
}

/// Loads the list into the engine, writing the starter list on first run.
pub fn load() {
    let macros = read();
    match path() {
        Some(path) if !path.exists() => save(&macros),
        Some(path) => {
            LOADED_AT.with(|t| t.set(modified(&path)));
            hook::with_engine(|e| e.set_macros(macros));
        }
        None => hook::with_engine(|e| e.set_macros(macros)),
    }
}

/// Reloads the list if the file changed since it was loaded (hand edits).
/// Cheap enough to call on every keystroke: it looks at the file at most
/// every [`CHECK_EVERY`].
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

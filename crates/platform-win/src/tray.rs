//! System tray icon: a "V" badge in Vietnamese mode, "E" when off. It
//! holds the settings menu (settings panel, Vietnamese/English, Telex/VNI,
//! tone style, auto-restore, quit).

use std::cell::Cell;

use engine::{InputMode, TonePlacement, TypingStyle};
use windows::core::{w, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, GetCursorPos, GetSystemMetrics, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, SetMenuDefaultItem, TrackPopupMenu, HICON, HWND_DESKTOP, MENU_ITEM_FLAGS,
    MF_CHECKED, MF_SEPARATOR, MF_STRING, SM_CXSMICON, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW, WS_EX_LEFT, WS_OVERLAPPED,
};

use crate::{config, hook, icon, settings};

const WM_TRAYICON: u32 = WM_APP + 1;
const CLASS_NAME: PCWSTR = w!("VietImeTrayWindow");

const ID_TOGGLE_MODE: u32 = 1;
const ID_TELEX: u32 = 2;
const ID_VNI: u32 = 3;
const ID_TONE_MODERN: u32 = 4;
const ID_TONE_CLASSIC: u32 = 5;
const ID_AUTO_RESTORE: u32 = 6;
const ID_SIMPLE_TELEX: u32 = 7;
const ID_SETTINGS: u32 = 8;
const ID_EXIT: u32 = 99;

thread_local! {
    static TRAY_WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    /// (Vietnamese, English) icons, owned by `TrayIcon`.
    static ICONS: Cell<Option<(HICON, HICON)>> = const { Cell::new(None) };
    /// A double-click just happened; the button-up that follows it belongs
    /// to the double-click, not to a new single click.
    static SKIP_NEXT_CLICK: Cell<bool> = const { Cell::new(false) };
}

/// Owns the hidden window and the tray icon it registered.
pub struct TrayIcon {
    hwnd: HWND,
}

impl TrayIcon {
    pub fn create() -> Result<Self> {
        // SAFETY: a plain metric query.
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32;
        ICONS.with(|i| -> Result<()> {
            i.set(Some((
                icon::mode_badge(size, true)?,
                icon::mode_badge(size, false)?,
            )));
            Ok(())
        })?;
        let hwnd = create_window()?;
        add_icon(hwnd)?;
        TRAY_WINDOW.with(|w| w.set(Some(hwnd)));
        refresh();
        Ok(Self { hwnd })
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        TRAY_WINDOW.with(|w| w.set(None));
        let nid = notify_icon_data(self.hwnd);
        // SAFETY: `self.hwnd` is the window this icon was registered on.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = DestroyWindow(self.hwnd);
            if let Some((vietnamese, english)) = ICONS.with(|i| i.take()) {
                let _ = DestroyIcon(vietnamese);
                let _ = DestroyIcon(english);
            }
        }
    }
}

/// Updates the tooltip and the settings panel to match the current state and
/// saves it (called after any change, including the Ctrl+Shift hotkey).
pub fn refresh() {
    config::save();
    settings::refresh();
    let Some(hwnd) = TRAY_WINDOW.with(|w| w.get()) else {
        return;
    };
    let (mode, style) = hook::with_engine(|e| (e.mode(), e.style()));
    let state = match (mode, style) {
        (InputMode::English, _) => "English",
        (InputMode::Vietnamese, TypingStyle::Telex) => "Tiếng Việt (Telex)",
        (InputMode::Vietnamese, TypingStyle::SimpleTelex) => "Tiếng Việt (Simple Telex)",
        (InputMode::Vietnamese, TypingStyle::Vni) => "Tiếng Việt (VNI)",
    };
    let text = format!("{} — {state}", crate::APP_NAME);

    let mut nid = notify_icon_data(hwnd);
    nid.uFlags = NIF_TIP | NIF_ICON;
    let wide: Vec<u16> = text.encode_utf16().take(nid.szTip.len() - 1).collect();
    nid.szTip[..wide.len()].copy_from_slice(&wide);
    // SAFETY: `nid` describes the icon registered on `hwnd`.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

fn create_window() -> Result<HWND> {
    // SAFETY: `wnd_proc` matches `WNDPROC` and lives for the whole process.
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassW(&class);

        CreateWindowExW(
            WS_EX_LEFT,
            CLASS_NAME,
            &HSTRING::from(crate::APP_NAME),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            Some(HWND_DESKTOP),
            None,
            Some(hinstance.into()),
            None,
        )
    }
}

fn notify_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: current_icon(),
        ..Default::default()
    }
}

/// The badge matching the current mode: V for Vietnamese, E for English.
fn current_icon() -> HICON {
    let vietnamese = hook::with_engine(|e| e.mode()) == InputMode::Vietnamese;
    match ICONS.with(Cell::get) {
        Some((v, _)) if vietnamese => v,
        Some((_, e)) => e,
        None => HICON::default(),
    }
}

fn add_icon(hwnd: HWND) -> Result<()> {
    let nid = notify_icon_data(hwnd);
    // SAFETY: `nid` is fully initialized for `hwnd`.
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid).ok() }
}

fn show_context_menu(hwnd: HWND) {
    let (mode, style, tone, restore) =
        hook::with_engine(|e| (e.mode(), e.style(), e.tone_placement(), e.auto_restore()));

    // SAFETY: the menu is created, shown and destroyed within this call, and
    // every label is a null-terminated literal from `w!`.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let item = |id: u32, label: PCWSTR, checked: bool| {
            let flags = if checked {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let _ = AppendMenuW(menu, flags, id as usize, label);
        };
        let separator = || {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        };

        item(ID_SETTINGS, w!("Bảng điều khiển..."), false);
        let _ = SetMenuDefaultItem(menu, ID_SETTINGS, 0);
        separator();
        item(
            ID_TOGGLE_MODE,
            w!("Tiếng Việt (Ctrl+Shift)"),
            mode == InputMode::Vietnamese,
        );
        separator();
        item(ID_TELEX, w!("Kiểu gõ Telex"), style == TypingStyle::Telex);
        item(
            ID_SIMPLE_TELEX,
            w!("Kiểu gõ Simple Telex (w không thành ư)"),
            style == TypingStyle::SimpleTelex,
        );
        item(ID_VNI, w!("Kiểu gõ VNI"), style == TypingStyle::Vni);
        separator();
        item(
            ID_TONE_MODERN,
            w!("Đặt dấu: hòa, thúy"),
            tone == TonePlacement::Modern,
        );
        item(
            ID_TONE_CLASSIC,
            w!("Đặt dấu: hoà, thuý"),
            tone == TonePlacement::Classic,
        );
        item(ID_AUTO_RESTORE, w!("Tự khôi phục phím với từ sai"), restore);
        separator();
        let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), ID_EXIT as usize, w!("Thoát"));

        let mut cursor = Default::default();
        let _ = GetCursorPos(&mut cursor);
        // Required so the popup menu closes when the user clicks away from it.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

fn handle_command(id: u32) {
    match id {
        ID_SETTINGS => {
            let _ = settings::show();
        }
        ID_TOGGLE_MODE => hook::toggle_mode(),
        ID_TELEX => hook::with_engine(|e| e.set_style(TypingStyle::Telex)),
        ID_SIMPLE_TELEX => hook::with_engine(|e| e.set_style(TypingStyle::SimpleTelex)),
        ID_VNI => hook::with_engine(|e| e.set_style(TypingStyle::Vni)),
        ID_TONE_MODERN => hook::with_engine(|e| e.set_tone_placement(TonePlacement::Modern)),
        ID_TONE_CLASSIC => hook::with_engine(|e| e.set_tone_placement(TonePlacement::Classic)),
        ID_AUTO_RESTORE => hook::with_engine(|e| e.set_auto_restore(!e.auto_restore())),
        // SAFETY: posting WM_QUIT to our own thread's message loop.
        ID_EXIT => unsafe { PostQuitMessage(0) },
        _ => return,
    }
    refresh();
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            match lparam.0 as u32 {
                // Single click toggles Vietnamese/English at once (no delay
                // waiting for a possible double-click). A double-click
                // arrives as up, DBLCLK, up: undo the first toggle, open the
                // panel, and ignore the trailing up.
                WM_LBUTTONUP => {
                    if !SKIP_NEXT_CLICK.with(|s| s.replace(false)) {
                        hook::toggle_mode();
                        refresh();
                    }
                }
                WM_LBUTTONDBLCLK => {
                    SKIP_NEXT_CLICK.with(|s| s.set(true));
                    hook::toggle_mode();
                    refresh();
                    let _ = settings::show();
                }
                WM_RBUTTONUP => show_context_menu(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            handle_command((wparam.0 & 0xffff) as u32);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

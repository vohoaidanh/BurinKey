//! Low-level keyboard hook: the bridge between raw Windows keystrokes and the
//! platform-independent [`engine::Engine`].
//!
//! The hook callback must return almost instantly (Windows silently
//! unregisters a `WH_KEYBOARD_LL` hook that blocks too long), so it does pure,
//! allocation-light work and never touches the network or disk.

use std::cell::RefCell;
use std::panic;

use engine::{Action, Engine, InputMode, KeyEvent};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, MapVirtualKeyExW, ToUnicodeEx,
    MAPVK_VK_TO_VSC_EX, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow,
    GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW, RegisterClassW, SetWindowsHookExW,
    UnhookWindowsHookEx, GUITHREADINFO, HHOOK, HWND_MESSAGE, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WM_APP, WM_KEYDOWN, WM_SYSKEYDOWN, WNDCLASSW,
};

use crate::inject;

/// Our own message, posted to `PUMP_WINDOW` to apply a pending [`Action::Replace`].
const WM_APPLY_REPLACEMENT: u32 = WM_APP + 100;

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::new());
    /// The replacement queued by the most recent keystroke, applied once the
    /// pump window picks up `WM_APPLY_REPLACEMENT` (see `KeyboardHook`'s docs
    /// for why this can't just be done inline in the hook callback).
    static PENDING_REPLACEMENT: RefCell<Option<(u8, String)>> = const { RefCell::new(None) };
    /// The focused control as of the last keystroke, so switching to a
    /// different field, dialog, or app between keystrokes can reset the
    /// in-progress word instead of silently continuing to compose onto
    /// whatever that new control already contains.
    static LAST_FOCUS: std::cell::Cell<HWND> = const { std::cell::Cell::new(HWND(std::ptr::null_mut())) };
}

/// Owns the installed hook (and the hidden window used to defer applying its
/// replacements; see below); tears both down on drop.
///
/// Must be created on the thread that runs the Win32 message loop, and must
/// stay alive for as long as that loop runs.
///
/// A hook callback runs *ahead of* the normal message pump: when we decide to
/// swallow a keystroke and inject a replacement (backspaces + the composed
/// text) via `SendInput`, calling that `SendInput` synchronously from inside
/// the callback races with the *previous* keystroke still making its way to
/// the focused control through that same pump. Lose the race and the
/// backspaces can fire before the control has registered the letter they're
/// supposed to delete, corrupting the word. Posting a message to our own
/// window instead defers the actual `SendInput` call to the next pass through
/// the message loop, by which point the previous keystroke has landed.
pub struct KeyboardHook {
    hook: HHOOK,
    pump_window: HWND,
}

impl KeyboardHook {
    pub fn install() -> windows::core::Result<Self> {
        let pump_window = create_pump_window()?;
        // SAFETY: `hook_proc` has the signature required by `WH_KEYBOARD_LL` and
        // stays valid for as long as the process is running.
        let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) } {
            Ok(hook) => hook,
            Err(err) => {
                // SAFETY: `pump_window` was just created successfully above.
                unsafe {
                    let _ = DestroyWindow(pump_window);
                }
                return Err(err);
            }
        };
        PUMP_WINDOW.with(|w| w.set(Some(pump_window)));
        Ok(Self { hook, pump_window })
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        PUMP_WINDOW.with(|w| w.set(None));
        // SAFETY: `self.hook`/`self.pump_window` were returned by successful
        // creation calls above and have not been torn down yet.
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
            let _ = DestroyWindow(self.pump_window);
        }
    }
}

thread_local! {
    static PUMP_WINDOW: std::cell::Cell<Option<HWND>> = const { std::cell::Cell::new(None) };
}

const PUMP_WINDOW_CLASS: windows::core::PCWSTR = w!("VietImeHookPump");

fn create_pump_window() -> windows::core::Result<HWND> {
    // SAFETY: `pump_wnd_proc` matches the required `WNDPROC` signature and is
    // valid for the process's lifetime; the class/window calls are used with
    // valid, null-terminated wide strings from the `w!` macro.
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(pump_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: PUMP_WINDOW_CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);

        CreateWindowExW(
            Default::default(),
            PUMP_WINDOW_CLASS,
            windows::core::PCWSTR::null(),
            Default::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance.into()),
            None,
        )
    }
}

unsafe extern "system" fn pump_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_APPLY_REPLACEMENT {
        if let Some((backspaces, insert)) = PENDING_REPLACEMENT.with(|p| p.borrow_mut().take()) {
            inject::replace_text(backspaces, &insert);
        }
        return LRESULT(0);
    }
    // SAFETY: forwarding whatever we don't handle to the default handler is
    // always valid.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Runs `f` against the engine shared with the hook (same thread).
pub fn with_engine<R>(f: impl FnOnce(&mut Engine) -> R) -> R {
    ENGINE.with(|e| f(&mut e.borrow_mut()))
}

pub fn toggle_mode() {
    with_engine(|e| {
        let next = match e.mode() {
            InputMode::Vietnamese => InputMode::English,
            InputMode::English => InputMode::Vietnamese,
        };
        e.set_mode(next);
    });
}

thread_local! {
    /// Ctrl+Shift has been pressed together with nothing else in between;
    /// releasing either key then toggles Vietnamese/English.
    static SWITCH_ARMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn is_ctrl(vk: u32) -> bool {
    vk == VK_CONTROL.0 as u32 || vk == VK_LCONTROL.0 as u32 || vk == VK_RCONTROL.0 as u32
}

fn is_shift(vk: u32) -> bool {
    vk == VK_SHIFT.0 as u32 || vk == VK_LSHIFT.0 as u32 || vk == VK_RSHIFT.0 as u32
}

/// Tracks the Ctrl+Shift switch hotkey. Never swallows anything.
fn track_switch_hotkey(msg: u32, vk: u32) {
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let modifier = is_ctrl(vk) || is_shift(vk);
    if down {
        let armed = modifier
            && ((is_ctrl(vk) && is_key_down(VK_SHIFT.0))
                || (is_shift(vk) && is_key_down(VK_CONTROL.0)));
        if armed || !modifier {
            SWITCH_ARMED.with(|a| a.set(armed));
        }
    } else if modifier && SWITCH_ARMED.with(|a| a.replace(false)) {
        toggle_mode();
        crate::tray::refresh();
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // A panic unwinding across an `extern "system"` boundary is undefined
    // behavior; catching it also means one engine bug can never freeze the
    // user's keyboard. On panic we just let the key through untouched.
    let swallow = panic::catch_unwind(|| handle_event(wparam, lparam)).unwrap_or(false);

    if swallow {
        LRESULT(1)
    } else {
        CallNextHookEx(None, code, wparam, lparam)
    }
}

/// Returns true if the original keystroke should be swallowed (because we
/// already injected a replacement for it).
fn handle_event(wparam: WPARAM, lparam: LPARAM) -> bool {
    let msg = wparam.0 as u32;

    // SAFETY: for `WH_KEYBOARD_LL`, `lparam` always points to a valid
    // `KBDLLHOOKSTRUCT` for the duration of this call.
    let data = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

    if data.dwExtraInfo == inject::OUR_INJECTED_MARKER {
        return false;
    }

    track_switch_hotkey(msg, data.vkCode);

    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return false;
    }

    if modifier_is_down() {
        return false;
    }

    reset_on_focus_change();
    crate::macro_file::reload_if_changed();

    let event = classify_key(data.vkCode);
    let action = ENGINE.with(|e| e.borrow_mut().process(event));

    match action {
        Action::PassThrough => false,
        Action::Replace { backspaces, insert } => {
            queue_replacement(backspaces, insert);
            true
        }
    }
}

/// Queues a replacement and wakes the pump window to apply it on the next
/// pass through the message loop (see [`KeyboardHook`]'s docs for why it
/// can't just be applied here).
fn queue_replacement(backspaces: u8, insert: String) {
    PENDING_REPLACEMENT.with(|p| *p.borrow_mut() = Some((backspaces, insert)));
    if let Some(hwnd) = PUMP_WINDOW.with(|w| w.get()) {
        // SAFETY: `hwnd` is the pump window created alongside this hook and
        // destroyed only after the hook is uninstalled.
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_APPLY_REPLACEMENT, WPARAM(0), LPARAM(0));
        }
    }
}

/// Clears the in-progress word if the focused *control* changed since the
/// last keystroke — e.g. the user clicked into a different field or switched
/// apps without typing anything first, which otherwise leaves us silently
/// composing onto text we never saw. Two fields in the same dialog/page share
/// one top-level window, so this tracks the actual focused control
/// (`GUITHREADINFO::hwndFocus`) rather than just the foreground window.
fn reset_on_focus_change() {
    let current = focused_control();
    let changed = LAST_FOCUS.with(|w| w.replace(current) != current);
    if changed {
        ENGINE.with(|e| e.borrow_mut().reset());
    }
}

fn focused_control() -> HWND {
    // SAFETY: both calls have no preconditions; a null foreground window is
    // handled by `GetWindowThreadProcessId` returning 0, and `GetGUIThreadInfo`
    // failing (e.g. no window has focus yet) leaves `info.hwndFocus` at its
    // zeroed default.
    unsafe {
        let foreground = GetForegroundWindow();
        let thread_id = GetWindowThreadProcessId(foreground, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let _ = GetGUIThreadInfo(thread_id, &mut info);
        info.hwndFocus
    }
}

/// Ctrl/Alt/Win combinations are shortcuts, not text, and must never be
/// swallowed or fed into the word-composition state.
fn modifier_is_down() -> bool {
    is_key_down(VK_CONTROL.0)
        || is_key_down(VK_MENU.0)
        || is_key_down(VK_LWIN.0)
        || is_key_down(VK_RWIN.0)
}

fn is_key_down(vk: u16) -> bool {
    // SAFETY: `GetAsyncKeyState` has no preconditions beyond a valid vkey.
    let state = unsafe { GetAsyncKeyState(vk as i32) };
    (state as u16 & 0x8000) != 0
}

fn classify_key(vk_code: u32) -> KeyEvent {
    if vk_code == VK_BACK.0 as u32 {
        return KeyEvent::Backspace;
    }
    match printable_char_for(vk_code) {
        Some(c) => KeyEvent::Char(c),
        None => KeyEvent::WordBoundary,
    }
}

/// Translates a virtual-key code into the character it would type, using
/// whatever keyboard layout the currently focused app is using (which can
/// differ from this thread's own layout). `None` means the key doesn't
/// produce a character at all (arrows, function keys, ...) or is a dead key
/// (an accent waiting for the next keystroke) — dead-key layouts aren't a
/// target for Telex/VNI typing, so we don't try to track that pending state,
/// we just drop it.
fn printable_char_for(vk_code: u32) -> Option<char> {
    let layout = foreground_keyboard_layout();

    let mut key_state = [0u8; 256];
    if is_key_down(VK_SHIFT.0) {
        key_state[VK_SHIFT.0 as usize] = 0x80;
    }
    if is_toggled(VK_CAPITAL) {
        key_state[VK_CAPITAL.0 as usize] = 0x01;
    }

    // SAFETY: `vk_code` comes straight from the hook's own `KBDLLHOOKSTRUCT`,
    // and `layout` is a layout handle `GetKeyboardLayout` just returned.
    let scan_code = unsafe { MapVirtualKeyExW(vk_code, MAPVK_VK_TO_VSC_EX, Some(layout)) };

    let mut buf = [0u16; 4];
    // SAFETY: `key_state` is a full 256-byte array as required, and `buf` is
    // a valid, exclusively-owned buffer for the duration of this call. This
    // call does not change the physical keyboard state; it only mutates this
    // thread's internal dead-key composition state, which we intentionally
    // don't track (see above).
    let written = unsafe { ToUnicodeEx(vk_code, scan_code, &key_state, &mut buf, 0, Some(layout)) };

    if written == 1 {
        char::from_u32(buf[0] as u32).filter(|c| !c.is_control())
    } else {
        None
    }
}

fn is_toggled(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    // SAFETY: `GetKeyState` has no preconditions beyond a valid vkey.
    (unsafe { GetKeyState(vk.0 as i32) } & 0x0001) != 0
}

/// The keyboard layout of whichever window currently has focus, so typing
/// still composes correctly if that app uses a different layout than the
/// one this background thread would otherwise default to.
fn foreground_keyboard_layout() -> windows::Win32::UI::Input::KeyboardAndMouse::HKL {
    // SAFETY: both calls have no preconditions; a null foreground window
    // (nothing focused) is handled by `GetWindowThreadProcessId` returning 0,
    // for which `GetKeyboardLayout` falls back to the system default layout.
    unsafe {
        let foreground = GetForegroundWindow();
        let thread_id = GetWindowThreadProcessId(foreground, None);
        GetKeyboardLayout(thread_id)
    }
}

//! "Bảng gõ tắt": a window to add, change and delete gõ tắt entries,
//! drawn in the same style as the settings panel. The two text boxes are
//! native EDIT controls (so typing, selection, copy/paste and Vietnamese
//! input all just work); everything else is drawn with Direct2D.
//!
//! Every change is saved to `macros.txt` at once and put to work.

use std::cell::{Cell, RefCell};

use engine::Macros;
use windows::core::{w, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::{D2D1CreateFactory, D2D1_FACTORY_TYPE_SINGLE_THREADED};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, DeleteObject, InvalidateRect, ScreenToClient, ValidateRect, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, FW_NORMAL, HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN, VK_TAB};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetParent, GetSystemMetrics, GetWindowTextLengthW,
    GetWindowTextW, LoadCursorW, MoveWindow, PostMessageW, RegisterClassW, SendMessageW, SetCursor,
    SetForegroundWindow, SetWindowPos, SetWindowTextW, ShowWindow, HMENU, HTCAPTION, HTCLIENT,
    IDC_ARROW, IDC_HAND, SM_CXICON, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOZORDER,
    SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CHAR, WM_CLOSE, WM_DPICHANGED,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCHITTEST, WM_PAINT,
    WM_SETCURSOR, WM_SETFONT, WM_SIZE, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_EX_APPWINDOW,
    WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

use super::layout::Rect;
use super::paint::*;
use super::{create_target, lparam_point, to_dips, Target, WM_MOUSELEAVE};
use crate::{icon, macro_file};

const CLASS_NAME: PCWSTR = w!("BurinMacroEditor");
/// Posted by the text boxes when Enter is pressed.
const WM_SAVE_ENTRY: u32 = WM_APP + 10;
const ES_AUTOHSCROLL: u32 = 0x0080;
const EM_LIMITTEXT: u32 = 0x00C5;

// ---------------------------------------------------------------------------
// Layout (DIPs)
// ---------------------------------------------------------------------------

const WIDTH: f32 = 480.0;
const HEIGHT: f32 = 560.0;
const MARGIN: f32 = 20.0;
const HEADER: f32 = 64.0;

const CLOSE: Rect = Rect::new(WIDTH - 44.0, 18.0, 28.0, 28.0);
const LABEL_Y: f32 = 78.0;
const KEY_BOX: Rect = Rect::new(MARGIN, 100.0, 110.0, 36.0);
const SAVE_BUTTON: Rect = Rect::new(WIDTH - MARGIN - 110.0, 100.0, 110.0, 36.0);
const FULL_X: f32 = KEY_BOX.x + KEY_BOX.w + 10.0;
const FULL_BOX: Rect = Rect::new(FULL_X, 100.0, SAVE_BUTTON.x - 10.0 - FULL_X, 36.0);
const MESSAGE: Rect = Rect::new(MARGIN, 140.0, WIDTH - 2.0 * MARGIN, 24.0);

const LIST: Rect = Rect::new(MARGIN, 172.0, WIDTH - 2.0 * MARGIN, 318.0);
const LIST_HEAD: f32 = 30.0;
const ROW: f32 = 32.0;
const VISIBLE_ROWS: usize = 9;
const KEY_COLUMN: f32 = 120.0;

const DELETE_BUTTON: Rect = Rect::new(MARGIN, 504.0, 110.0, 36.0);
const DONE_BUTTON: Rect = Rect::new(WIDTH - MARGIN - 110.0, 504.0, 110.0, 36.0);

/// Text box height inside its rounded frame.
const EDIT_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Close,
    Save,
    Delete,
    Done,
    /// A row of the list, by index into the sorted entries.
    Row(usize),
    /// The list's empty space: clicking it clears the selection.
    List,
}

fn row_rect(slot: usize) -> Rect {
    Rect::new(
        LIST.x + 4.0,
        LIST.y + LIST_HEAD + ROW * slot as f32,
        LIST.w - 8.0,
        ROW,
    )
}

fn item_at(editor: &Editor, x: f32, y: f32) -> Option<Item> {
    let fixed = [
        (Item::Close, CLOSE),
        (Item::Save, SAVE_BUTTON),
        (Item::Delete, DELETE_BUTTON),
        (Item::Done, DONE_BUTTON),
    ];
    if let Some((item, _)) = fixed.iter().find(|(_, r)| r.contains(x, y)) {
        return Some(*item);
    }
    let rows = editor
        .macros
        .len()
        .saturating_sub(editor.scroll)
        .min(VISIBLE_ROWS);
    (0..rows)
        .find(|&slot| row_rect(slot).contains(x, y))
        .map(|slot| Item::Row(editor.scroll + slot))
        .or_else(|| LIST.contains(x, y).then_some(Item::List))
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct Editor {
    fonts: Fonts,
    d2d: windows::Win32::Graphics::Direct2D::ID2D1Factory,
    target: Option<Target>,
    hover: Option<Item>,
    macros: Macros,
    /// The short word of the entry being edited, if any.
    selected: Option<String>,
    scroll: usize,
    /// Feedback under the text boxes: (text, is_error).
    message: Option<(String, bool)>,
    key_edit: HWND,
    full_edit: HWND,
    font: Option<HFONT>,
}

thread_local! {
    static EDITOR_WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    static EDITOR: RefCell<Option<Editor>> = const { RefCell::new(None) };
}

fn with_editor<R>(f: impl FnOnce(&mut Editor) -> R) -> Option<R> {
    EDITOR.with(|cell| cell.borrow_mut().as_mut().map(f))
}

fn repaint() {
    if let Some(hwnd) = EDITOR_WINDOW.with(Cell::get) {
        // SAFETY: `hwnd` is our live editor window.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

/// Opens the window (or brings it to the front) with the saved list.
pub fn show() -> Result<()> {
    let hwnd = match EDITOR_WINDOW.with(Cell::get) {
        Some(hwnd) => hwnd,
        None => create()?,
    };
    let key_edit = with_editor(|e| {
        e.macros = macro_file::read();
        e.selected = None;
        e.message = None;
        e.scroll = 0;
        e.key_edit
    });
    clear_inputs();
    // SAFETY: `hwnd` and the text box are our live windows.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        if let Some(edit) = key_edit {
            let _ = SetFocus(Some(edit));
        }
    }
    repaint();
    Ok(())
}

fn create() -> Result<HWND> {
    let editor = Editor {
        fonts: Fonts::new()?,
        // SAFETY: plain factory creation with no borrowed arguments.
        d2d: unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? },
        target: None,
        hover: None,
        macros: Macros::default(),
        selected: None,
        scroll: 0,
        message: None,
        key_edit: HWND::default(),
        full_edit: HWND::default(),
        font: None,
    };

    // SAFETY: `wnd_proc`/`edit_proc` match their callback types and live for
    // the whole process; all strings are null-terminated literals.
    let (hwnd, key_edit, full_edit) = unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: icon::pig(GetSystemMetrics(SM_CXICON).max(32) as u32)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassW(&class);

        let scale = GetDpiForSystem() as f32 / 96.0;
        let (w, h) = ((WIDTH * scale) as i32, (HEIGHT * scale) as i32);
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 3;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            CLASS_NAME,
            w!("Bảng gõ tắt"),
            WS_POPUP | WS_CLIPCHILDREN | WS_MINIMIZEBOX | WS_SYSMENU,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        let corners = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corners as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        let edit = |id: usize, limit: usize| -> Result<HWND> {
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL),
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(id as _)),
                Some(hinstance.into()),
                None,
            )?;
            SendMessageW(edit, EM_LIMITTEXT, Some(WPARAM(limit)), None);
            let _ = SetWindowSubclass(edit, Some(edit_proc), 1, 0);
            Ok(edit)
        };
        (hwnd, edit(1, 32)?, edit(2, 500)?)
    };

    EDITOR.with(|cell| {
        *cell.borrow_mut() = Some(Editor {
            key_edit,
            full_edit,
            ..editor
        })
    });
    EDITOR_WINDOW.with(|w| w.set(Some(hwnd)));
    place_inputs(hwnd);
    Ok(hwnd)
}

/// Positions the text boxes inside their drawn frames and gives them a
/// font matching the window's DPI.
fn place_inputs(hwnd: HWND) {
    // SAFETY: `hwnd` and the text boxes are our live windows; the old font
    // is deleted only after the boxes switched to the new one.
    unsafe {
        let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
        let font = CreateFontW(
            -(14.0 * scale).round() as i32,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        );
        let Some((key_edit, full_edit, old)) =
            with_editor(|e| (e.key_edit, e.full_edit, e.font.replace(font)))
        else {
            return;
        };
        for (edit, frame) in [(key_edit, KEY_BOX), (full_edit, FULL_BOX)] {
            let px = |v: f32| (v * scale).round() as i32;
            let _ = MoveWindow(
                edit,
                px(frame.x + 10.0),
                px(frame.y + (frame.h - EDIT_HEIGHT) / 2.0),
                px(frame.w - 20.0),
                px(EDIT_HEIGHT),
                true,
            );
            SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        if let Some(old) = old {
            let _ = DeleteObject(HGDIOBJ(old.0));
        }
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

fn text_of(edit: HWND) -> String {
    // SAFETY: the buffer is sized from the control's own length.
    unsafe {
        let len = GetWindowTextLengthW(edit).max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(edit, &mut buf).max(0) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}

fn set_text(edit: HWND, text: &str) {
    // SAFETY: `edit` is our live text box.
    unsafe {
        let _ = SetWindowTextW(edit, &HSTRING::from(text));
    }
}

fn clear_inputs() {
    if let Some((key_edit, full_edit)) = with_editor(|e| (e.key_edit, e.full_edit)) {
        set_text(key_edit, "");
        set_text(full_edit, "");
    }
}

/// "Thêm" / "Cập nhật": stores what's in the text boxes.
fn save_entry() {
    let Some((key_edit, full_edit, selected, mut macros)) = with_editor(|e| {
        (
            e.key_edit,
            e.full_edit,
            e.selected.clone(),
            e.macros.clone(),
        )
    }) else {
        return;
    };
    let (key, full) = (text_of(key_edit), text_of(full_edit));
    if let Some(old) = &selected {
        if !old.eq_ignore_ascii_case(key.trim()) {
            macros.remove(old);
        }
    }
    let existed = macros.expand(key.trim()).is_some();
    if !macros.insert(&key, &full) {
        with_editor(|e| {
            e.message = Some((
                "Cần có từ tắt (không dấu cách) và cụm từ đầy đủ.".into(),
                true,
            ))
        });
        return repaint();
    }
    macro_file::save(&macros);
    let key = key.trim().to_lowercase();
    let note = if existed || selected.is_some() {
        format!("Đã cập nhật “{key}”.")
    } else {
        format!("Đã thêm “{key}”.")
    };
    with_editor(|e| {
        e.macros = macros;
        e.selected = None;
        e.message = Some((note, false));
    });
    clear_inputs();
    // SAFETY: `key_edit` is our live text box.
    unsafe {
        let _ = SetFocus(Some(key_edit));
    }
    repaint();
}

fn delete_entry() {
    let Some(Some((key, mut macros))) =
        with_editor(|e| e.selected.clone().map(|k| (k, e.macros.clone())))
    else {
        return;
    };
    macros.remove(&key);
    macro_file::save(&macros);
    with_editor(|e| {
        e.scroll = e.scroll.min(macros.len().saturating_sub(VISIBLE_ROWS));
        e.macros = macros;
        e.selected = None;
        e.message = Some((format!("Đã xoá “{key}”."), false));
    });
    clear_inputs();
    repaint();
}

fn select_row(index: usize) {
    let Some(Some((key, full, key_edit, full_edit))) = with_editor(|e| {
        let (key, full) = e
            .macros
            .entries()
            .get(index)
            .map(|(k, f)| (k.to_string(), f.to_string()))?;
        e.selected = Some(key.clone());
        e.message = None;
        Some((key, full, e.key_edit, e.full_edit))
    }) else {
        return;
    };
    set_text(key_edit, &key);
    set_text(full_edit, &full);
    repaint();
}

fn clear_selection() {
    with_editor(|e| {
        e.selected = None;
        e.message = None;
    });
    clear_inputs();
    repaint();
}

fn hide(hwnd: HWND) {
    // SAFETY: `hwnd` is our live editor window.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

fn activate(hwnd: HWND, item: Item) {
    match item {
        Item::Close | Item::Done => hide(hwnd),
        Item::Save => save_entry(),
        Item::Delete => delete_entry(),
        Item::Row(index) => select_row(index),
        Item::List => clear_selection(),
    }
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw(p: &Painter, e: &Editor) {
    let hovered = |item| e.hover == Some(item);

    let header = Rect::new(0.0, 0.0, WIDTH, HEADER);
    p.fill(header, 0.0, CORAL);
    p.clipped(header, || p.circle(WIDTH - 10.0, -20.0, 70.0, SUN));
    p.circle(MARGIN + 20.0, HEADER / 2.0, 20.0, WHITE);
    p.logo(Rect::new(MARGIN + 4.0, HEADER / 2.0 - 16.0, 32.0, 32.0));
    p.text(
        "Bảng gõ tắt",
        &e.fonts.title,
        Rect::new(72.0, 8.0, 300.0, 28.0),
        WHITE,
    );
    let hint = "Gõ từ tắt rồi dấu cách: vn → Việt Nam";
    p.text(
        hint,
        &e.fonts.subtitle,
        Rect::new(72.0, 34.0, 320.0, 20.0),
        WHITE,
    );
    if hovered(Item::Close) {
        p.fill(CLOSE, CLOSE.w / 2.0, HOVER_SHADE);
    }
    let (cx, cy, arm) = (CLOSE.x + CLOSE.w / 2.0, CLOSE.y + CLOSE.h / 2.0, 6.0);
    p.polyline(&[(cx - arm, cy - arm), (cx + arm, cy + arm)], TEXT, 1.6);
    p.polyline(&[(cx - arm, cy + arm), (cx + arm, cy - arm)], TEXT, 1.6);

    // Entry row.
    p.text(
        "TỪ TẮT",
        &e.fonts.label,
        Rect::new(KEY_BOX.x, LABEL_Y, 120.0, 20.0),
        TEXT,
    );
    p.text(
        "THAY BẰNG",
        &e.fonts.label,
        Rect::new(FULL_BOX.x, LABEL_Y, 200.0, 20.0),
        TEXT,
    );
    for frame in [KEY_BOX, FULL_BOX] {
        p.fill(frame, 8.0, WHITE);
        p.stroke(frame, 8.0, BORDER, 1.0);
    }
    let editing = e.selected.is_some();
    let save_label = if editing { "Cập nhật" } else { "Thêm" };
    button(
        p,
        &e.fonts,
        SAVE_BUTTON,
        hovered(Item::Save),
        save_label,
        |x, y| {
            if editing {
                p.polyline(
                    &[(x - 6.0, y), (x - 2.0, y + 4.5), (x + 6.5, y - 5.0)],
                    ICON_SAFE,
                    2.4,
                );
            } else {
                p.polyline(&[(x - 6.0, y), (x + 6.0, y)], ICON, ICON_STROKE);
                p.polyline(&[(x, y - 6.0), (x, y + 6.0)], ICON, ICON_STROKE);
            }
        },
    );
    if let Some((message, is_error)) = &e.message {
        let color = if *is_error { ICON } else { MUTED };
        p.text(message, &e.fonts.body, MESSAGE, color);
    }

    // The list.
    p.fill(LIST, 10.0, WHITE);
    p.stroke(LIST, 10.0, BOX_BORDER, 1.0);
    let head_y = LIST.y + 4.0;
    let key_x = LIST.x + 16.0;
    let full_x = LIST.x + KEY_COLUMN;
    p.text(
        "TỪ TẮT",
        &e.fonts.label,
        Rect::new(key_x, head_y, 100.0, 24.0),
        MUTED,
    );
    p.text(
        "THAY BẰNG",
        &e.fonts.label,
        Rect::new(full_x, head_y, 200.0, 24.0),
        MUTED,
    );
    let line_y = LIST.y + LIST_HEAD;
    p.polyline(
        &[(LIST.x + 12.0, line_y), (LIST.x + LIST.w - 12.0, line_y)],
        BOX_BORDER,
        1.0,
    );

    let entries = e.macros.entries();
    if entries.is_empty() {
        let empty = Rect::new(LIST.x, line_y, LIST.w, LIST.h - LIST_HEAD);
        p.text(
            "Chưa có từ tắt nào. Thêm ở ô phía trên.",
            &e.fonts.centered,
            empty,
            MUTED,
        );
    }
    for (slot, (key, full)) in entries.iter().skip(e.scroll).take(VISIBLE_ROWS).enumerate() {
        let row = row_rect(slot);
        let index = e.scroll + slot;
        if e.selected.as_deref() == Some(*key) {
            p.fill(row, 6.0, SELECTED);
        } else if hovered(Item::Row(index)) {
            p.fill(row, 6.0, BUTTON_HOVER);
        }
        let key_rect = Rect::new(key_x, row.y, KEY_COLUMN - 24.0, row.h);
        let full_rect = Rect::new(full_x, row.y, row.x + row.w - full_x - 8.0, row.h);
        p.clipped(key_rect, || p.text(key, &e.fonts.button, key_rect, TEXT));
        p.clipped(full_rect, || {
            p.text(full, &e.fonts.body, full_rect, TEXT_SOFT)
        });
    }
    if entries.len() > VISIBLE_ROWS {
        // Scroll position, as a thin bar on the right edge.
        let track_h = LIST.h - LIST_HEAD - 12.0;
        let thumb_h = track_h * VISIBLE_ROWS as f32 / entries.len() as f32;
        let max_scroll = (entries.len() - VISIBLE_ROWS) as f32;
        let thumb_y = line_y + 6.0 + (track_h - thumb_h) * e.scroll as f32 / max_scroll;
        p.fill(
            Rect::new(LIST.x + LIST.w - 7.0, thumb_y, 3.0, thumb_h),
            1.5,
            BORDER,
        );
    }

    // Footer.
    let can_delete = e.selected.is_some();
    button(
        p,
        &e.fonts,
        DELETE_BUTTON,
        can_delete && hovered(Item::Delete),
        "Xoá",
        |x, y| {
            let color = if can_delete { ICON } else { BORDER };
            p.polyline(
                &[(x - 6.0, y - 4.0), (x + 6.0, y - 4.0)],
                color,
                ICON_STROKE,
            );
            p.polyline(
                &[
                    (x - 4.5, y - 4.0),
                    (x - 3.5, y + 6.5),
                    (x + 3.5, y + 6.5),
                    (x + 4.5, y - 4.0),
                ],
                color,
                ICON_STROKE,
            );
            p.polyline(
                &[
                    (x - 2.0, y - 4.0),
                    (x - 2.0, y - 7.0),
                    (x + 2.0, y - 7.0),
                    (x + 2.0, y - 4.0),
                ],
                color,
                ICON_STROKE,
            );
        },
    );
    let count = format!("{} từ tắt", entries.len());
    let between = Rect::new(
        DELETE_BUTTON.x + DELETE_BUTTON.w,
        DELETE_BUTTON.y,
        DONE_BUTTON.x - DELETE_BUTTON.x - DELETE_BUTTON.w,
        DELETE_BUTTON.h,
    );
    p.text(&count, &e.fonts.centered, between, MUTED);
    button(
        p,
        &e.fonts,
        DONE_BUTTON,
        hovered(Item::Done),
        "Xong",
        |x, y| {
            p.polyline(
                &[(x - 6.0, y), (x - 2.0, y + 4.5), (x + 6.5, y - 5.0)],
                ICON_SAFE,
                2.4,
            );
        },
    );
}

fn paint(hwnd: HWND) {
    EDITOR.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(editor) = slot.as_mut() else {
            return;
        };
        if editor.target.is_none() {
            editor.target = create_target(&editor.d2d, hwnd).ok();
        }
        let Some(Target {
            target,
            brush,
            logo,
        }) = &editor.target
        else {
            return;
        };
        // SAFETY: Begin/EndDraw bracket the drawing on our own target.
        let drawn = unsafe {
            target.BeginDraw();
            target.Clear(Some(&BACKGROUND));
            draw(
                &Painter {
                    target,
                    brush,
                    logo,
                },
                editor,
            );
            target.EndDraw(None, None)
        };
        if drawn.is_err() {
            editor.target = None;
        }
    });
}

// ---------------------------------------------------------------------------
// Window procedures
// ---------------------------------------------------------------------------

/// Text boxes: Enter saves, Tab moves to the other box, Esc closes.
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let key = wparam.0 as u16;
    match msg {
        WM_KEYDOWN if key == VK_RETURN.0 || key == VK_ESCAPE.0 => {
            if let Ok(parent) = GetParent(hwnd) {
                let message = if key == VK_RETURN.0 {
                    WM_SAVE_ENTRY
                } else {
                    WM_CLOSE
                };
                let _ = PostMessageW(Some(parent), message, WPARAM(0), LPARAM(0));
            }
            LRESULT(0)
        }
        WM_KEYDOWN if key == VK_TAB.0 => {
            let other = with_editor(|e| {
                if hwnd == e.key_edit {
                    e.full_edit
                } else {
                    e.key_edit
                }
            });
            if let Some(other) = other {
                let _ = SetFocus(Some(other));
            }
            LRESULT(0)
        }
        // Swallow the characters of those keys so the box doesn't beep.
        WM_CHAR if matches!(key, 0x0D | 0x09 | 0x1B) => LRESULT(0),
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

fn set_hover(hwnd: HWND, item: Option<Item>) {
    let changed = with_editor(|e| std::mem::replace(&mut e.hover, item) != item).unwrap_or(false);
    if changed {
        repaint();
    }
    if item.is_some() {
        super::track_mouse_leave(hwnd);
    }
}

fn item_under(hwnd: HWND, lparam: LPARAM) -> Option<Item> {
    let (px, py) = lparam_point(lparam);
    let (x, y) = to_dips(hwnd, px, py);
    with_editor(|e| item_at(e, x, y)).flatten()
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            let _ = ValidateRect(Some(hwnd), None);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => {
            with_editor(|e| e.target = None);
            repaint();
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let suggested = &*(lparam.0 as *const RECT);
            let _ = SetWindowPos(
                hwnd,
                None,
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            place_inputs(hwnd);
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let hit = DefWindowProcW(hwnd, msg, wparam, lparam);
            if hit.0 != HTCLIENT as isize {
                return hit;
            }
            let (sx, sy) = lparam_point(lparam);
            let mut pt = POINT { x: sx, y: sy };
            let _ = ScreenToClient(hwnd, &mut pt);
            let (x, y) = to_dips(hwnd, pt.x, pt.y);
            if y < HEADER && !CLOSE.contains(x, y) {
                LRESULT(HTCAPTION as isize)
            } else {
                hit
            }
        }
        WM_MOUSEMOVE => {
            set_hover(hwnd, item_under(hwnd, lparam));
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            set_hover(hwnd, None);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let notches = (wparam.0 >> 16) as u16 as i16 / 120;
            with_editor(|e| {
                let max = e.macros.len().saturating_sub(VISIBLE_ROWS) as isize;
                e.scroll = (e.scroll as isize - notches as isize * 3).clamp(0, max) as usize;
            });
            repaint();
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let clickable = with_editor(|e| {
                e.hover
                    .is_some_and(|i| i != Item::List && (i != Item::Delete || e.selected.is_some()))
            })
            .unwrap_or(false);
            if (lparam.0 & 0xffff) as u32 == HTCLIENT && clickable {
                if let Ok(hand) = LoadCursorW(None, IDC_HAND) {
                    SetCursor(Some(hand));
                }
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_LBUTTONUP => {
            if let Some(item) = item_under(hwnd, lparam) {
                activate(hwnd, item);
            }
            LRESULT(0)
        }
        WM_SAVE_ENTRY => {
            save_entry();
            LRESULT(0)
        }
        WM_CLOSE => {
            hide(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

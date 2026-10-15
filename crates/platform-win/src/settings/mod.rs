//! Settings panel ("bảng điều khiển"), organised as a
//! compact view with the main controls and an expandable advanced section.
//!
//! It is a borderless window drawn with Direct2D/DirectWrite and runs on the
//! hook thread, so it reads and changes the engine directly; every change
//! goes through `tray::refresh`, which also repaints this panel. Geometry is
//! in DIPs (1/96 inch); the render target scales to the monitor's DPI.

mod layout;
mod paint;

use std::cell::{Cell, RefCell};

use engine::{Engine, TonePlacement};
use windows::core::{w, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    D2D1_BITMAP_PROPERTIES, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetSystemMetrics, LoadCursorW, MessageBoxW,
    PostQuitMessage, RegisterClassW, SetCursor, SetForegroundWindow, SetWindowPos, ShowWindow,
    HTCAPTION, HTCLIENT, IDC_ARROW, IDC_HAND, MB_ICONINFORMATION, MB_OK, SM_CXICON, SM_CXSCREEN,
    SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE, SW_SHOW, WM_CLOSE, WM_DESTROY,
    WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR,
    WM_SIZE, WNDCLASSW, WS_EX_APPWINDOW, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
};

use crate::{autostart, config, hook, icon, macro_file, tray};
use layout::{control_at, Control, HEADER, WIDTH};
use paint::{draw, Fonts, Painter, State, BACKGROUND, WHITE};

const CLASS_NAME: PCWSTR = w!("VietImeSettingsWindow");

/// Defined in `Win32_UI_Controls`, a large feature pulled in for nothing else.
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Everything tied to one Direct2D render target; rebuilt together when the
/// window is resized, changes DPI or loses its device.
struct Target {
    target: ID2D1HwndRenderTarget,
    brush: ID2D1SolidColorBrush,
    logo: ID2D1Bitmap,
}

struct Panel {
    d2d: ID2D1Factory,
    fonts: Fonts,
    target: Option<Target>,
    hover: Option<Control>,
}

thread_local! {
    static PANEL_WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
    /// Whether the advanced section is shown ("Mở rộng").
    static EXPANDED: Cell<bool> = const { Cell::new(false) };
}

/// Opens the panel, or brings it to the front if it is already open.
pub fn show() -> Result<()> {
    let hwnd = match PANEL_WINDOW.with(Cell::get) {
        Some(hwnd) => hwnd,
        None => create()?,
    };
    // SAFETY: `hwnd` is our live panel window.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
    refresh();
    Ok(())
}

/// Repaints the panel after a setting changed elsewhere (hotkey, tray).
pub fn refresh() {
    if let Some(hwnd) = PANEL_WINDOW.with(Cell::get) {
        // SAFETY: `hwnd` is our live panel window.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

fn expanded() -> bool {
    EXPANDED.with(Cell::get)
}

/// Panel size in pixels for the given DPI.
fn pixel_size(dpi: u32) -> (i32, i32) {
    let scale = dpi as f32 / 96.0;
    let height = layout::height(expanded());
    (
        (WIDTH * scale).round() as i32,
        (height * scale).round() as i32,
    )
}

fn create() -> Result<HWND> {
    let panel = Panel {
        // SAFETY: plain factory creation with no borrowed arguments.
        d2d: unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? },
        fonts: Fonts::new()?,
        target: None,
        hover: None,
    };

    // SAFETY: `wnd_proc` matches `WNDPROC` and lives for the whole process;
    // the class name and title are null-terminated literals.
    let hwnd = unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            // Lives as long as the class, i.e. the whole process.
            hIcon: icon::pig(GetSystemMetrics(SM_CXICON).max(32) as u32)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassW(&class);

        let (w, h) = pixel_size(GetDpiForSystem());
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 3;
        // WS_MINIMIZEBOX/WS_SYSMENU draw nothing on a popup but let the
        // taskbar button minimize and restore it.
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            CLASS_NAME,
            &HSTRING::from(crate::APP_NAME),
            WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;

        // Windows 11 rounds the borderless window; older systems ignore it.
        let corners = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corners as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        hwnd
    };

    PANEL.with(|p| *p.borrow_mut() = Some(panel));
    PANEL_WINDOW.with(|w| w.set(Some(hwnd)));
    Ok(hwnd)
}

fn hide(hwnd: HWND) {
    // SAFETY: `hwnd` is our live panel window.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

/// Shows or hides the advanced section, growing/shrinking the window down.
fn toggle_expanded(hwnd: HWND) {
    EXPANDED.with(|e| e.set(!e.get()));
    // SAFETY: `hwnd` is our live panel window.
    unsafe {
        let (w, h) = pixel_size(GetDpiForWindow(hwnd));
        let _ = SetWindowPos(hwnd, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER);
    }
}

fn show_about(hwnd: HWND) {
    let text = HSTRING::from(format!(
        "{} {}\n\n\
         Bộ gõ tiếng Việt mã nguồn mở cho Windows 10/11.\n\n\
         Phần mềm tự do theo giấy phép GNU GPL v3 trở lên,\n\
         KHÔNG kèm bất kỳ bảo hành nào.\n\
         Có sử dụng mã nguồn của bên thứ ba: xem tệp NOTICE\n\
         và LICENSE đi kèm phần mềm.",
        crate::APP_NAME,
        env!("CARGO_PKG_VERSION")
    ));
    // SAFETY: both strings are live, null-terminated wide strings.
    unsafe {
        MessageBoxW(
            Some(hwnd),
            &text,
            w!("Thông tin"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// "Mặc định": typing settings go back to what a fresh engine starts with.
/// The on/off mode and autostart are left alone.
fn restore_defaults() {
    let defaults = Engine::new();
    hook::with_engine(|e| {
        e.set_style(defaults.style());
        e.set_tone_placement(defaults.tone_placement());
        e.set_auto_restore(defaults.auto_restore());
        e.set_macros_enabled(defaults.macros_enabled());
    });
}

fn activate(hwnd: HWND, control: Control) {
    match control {
        Control::Close | Control::Hide => return hide(hwnd),
        // SAFETY: posting WM_QUIT to our own thread's message loop.
        Control::Quit => return unsafe { PostQuitMessage(0) },
        Control::Expand => toggle_expanded(hwnd),
        Control::About => return show_about(hwnd),
        Control::Defaults => restore_defaults(),
        Control::Style(style) => hook::with_engine(|e| e.set_style(style)),
        Control::Mode => hook::toggle_mode(),
        Control::ClassicTone => hook::with_engine(|e| {
            let next = match e.tone_placement() {
                TonePlacement::Modern => TonePlacement::Classic,
                TonePlacement::Classic => TonePlacement::Modern,
            };
            e.set_tone_placement(next);
        }),
        Control::AutoRestore => hook::with_engine(|e| e.set_auto_restore(!e.auto_restore())),
        Control::Autostart => autostart::set_enabled(!autostart::is_enabled()),
        Control::Macros => hook::with_engine(|e| e.set_macros_enabled(!e.macros_enabled())),
        Control::EditMacros => return macro_file::open_in_editor(),
        Control::ShowOnStart => {
            config::set_show_panel_on_start(!config::current().show_panel_on_start)
        }
    }
    tray::refresh();
}

fn current_state() -> State {
    let config = config::current();
    State {
        mode: config.mode,
        style: config.style,
        tone: config.tone,
        auto_restore: config.auto_restore,
        macros: config.macros_enabled,
        autostart: autostart::is_enabled(),
        show_on_start: config.show_panel_on_start,
        expanded: expanded(),
    }
}

fn paint(hwnd: HWND) {
    let state = current_state();
    PANEL.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(panel) = slot.as_mut() else {
            return;
        };
        if panel.target.is_none() {
            panel.target = create_target(&panel.d2d, hwnd).ok();
        }
        let Some(Target {
            target,
            brush,
            logo,
        }) = &panel.target
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
                &panel.fonts,
                &state,
                panel.hover,
            );
            target.EndDraw(None, None)
        };
        if drawn.is_err() {
            // Device lost (driver reset, remote desktop…): rebuild next time.
            panel.target = None;
        }
    });
}

fn create_target(d2d: &ID2D1Factory, hwnd: HWND) -> Result<Target> {
    // SAFETY: `hwnd` is our live window and the property structs are locals.
    unsafe {
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client)?;
        let dpi = GetDpiForWindow(hwnd) as f32;
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            dpiX: dpi,
            dpiY: dpi,
            ..Default::default()
        };
        let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: client.right as u32,
                height: client.bottom as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let target = d2d.CreateHwndRenderTarget(&properties, &hwnd_properties)?;
        let brush = target.CreateSolidColorBrush(&WHITE, None)?;
        let logo = logo_bitmap(&target)?;
        Ok(Target {
            target,
            brush,
            logo,
        })
    }
}

/// The pig logo as a bitmap; drawn scaled down, so it stays sharp up to
/// about 300% DPI.
fn logo_bitmap(target: &ID2D1HwndRenderTarget) -> Result<ID2D1Bitmap> {
    const SIZE: u32 = 128;
    // Direct2D wants premultiplied alpha.
    let pixels: Vec<u32> = icon::pig_pixels(SIZE)
        .into_iter()
        .map(|p| {
            let a = p >> 24;
            let premultiply = |shift: u32| (((p >> shift) & 0xff) * a / 255) << shift;
            (a << 24) | premultiply(16) | premultiply(8) | premultiply(0)
        })
        .collect();
    let properties = D2D1_BITMAP_PROPERTIES {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
    };
    // SAFETY: `pixels` holds SIZE×SIZE 32-bit pixels with a SIZE*4 pitch.
    unsafe {
        target.CreateBitmap(
            D2D_SIZE_U {
                width: SIZE,
                height: SIZE,
            },
            Some(pixels.as_ptr().cast()),
            SIZE * 4,
            &properties,
        )
    }
}

/// Drops the render target so the next paint rebuilds it at the new size/DPI.
fn reset_target() {
    PANEL.with(|cell| {
        if let Some(panel) = cell.borrow_mut().as_mut() {
            panel.target = None;
        }
    });
    refresh();
}

/// Client-area pixel coordinates → DIPs.
fn to_dips(hwnd: HWND, x: i32, y: i32) -> (f32, f32) {
    // SAFETY: `hwnd` is our live panel window.
    let scale = 96.0 / unsafe { GetDpiForWindow(hwnd) }.max(1) as f32;
    (x as f32 * scale, y as f32 * scale)
}

fn lparam_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xffff) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

fn control_under(hwnd: HWND, lparam: LPARAM) -> Option<Control> {
    let (px, py) = lparam_point(lparam);
    let (x, y) = to_dips(hwnd, px, py);
    control_at(expanded(), x, y)
}

fn set_hover(hwnd: HWND, control: Option<Control>) {
    let changed = PANEL.with(|cell| match cell.borrow_mut().as_mut() {
        Some(panel) if panel.hover != control => {
            panel.hover = control;
            true
        }
        _ => false,
    });
    if changed {
        refresh();
    }
    if control.is_some() {
        // Ask for WM_MOUSELEAVE so the hover clears when the mouse exits.
        let mut track = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        // SAFETY: `track` is a fully initialized local.
        unsafe {
            let _ = TrackMouseEvent(&mut track);
        }
    }
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
        // Direct2D paints every pixel; skipping the erase avoids flicker.
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => {
            reset_target();
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
            reset_target();
            LRESULT(0)
        }
        // The header (outside its buttons) drags the window like a title bar.
        WM_NCHITTEST => {
            let hit = DefWindowProcW(hwnd, msg, wparam, lparam);
            if hit.0 != HTCLIENT as isize {
                return hit;
            }
            let (sx, sy) = lparam_point(lparam);
            let mut pt = POINT { x: sx, y: sy };
            let _ = ScreenToClient(hwnd, &mut pt);
            let (x, y) = to_dips(hwnd, pt.x, pt.y);
            if y < HEADER && control_at(expanded(), x, y).is_none() {
                LRESULT(HTCAPTION as isize)
            } else {
                hit
            }
        }
        WM_MOUSEMOVE => {
            set_hover(hwnd, control_under(hwnd, lparam));
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            set_hover(hwnd, None);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let over_control =
                PANEL.with(|c| c.borrow().as_ref().is_some_and(|p| p.hover.is_some()));
            if (lparam.0 & 0xffff) as u32 == HTCLIENT && over_control {
                if let Ok(hand) = LoadCursorW(None, IDC_HAND) {
                    SetCursor(Some(hand));
                }
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_LBUTTONUP => {
            if let Some(control) = control_under(hwnd, lparam) {
                activate(hwnd, control);
                // The layout may have changed under the cursor (expand).
                set_hover(hwnd, control_under(hwnd, lparam));
            }
            LRESULT(0)
        }
        // Alt+F4 and the taskbar's "Close window" just hide the panel; the
        // app keeps running in the tray.
        WM_CLOSE => {
            hide(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PANEL_WINDOW.with(|w| w.set(None));
            PANEL.with(|p| p.borrow_mut().take());
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

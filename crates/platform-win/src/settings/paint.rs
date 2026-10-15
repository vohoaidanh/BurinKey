//! Drawing the panel with Direct2D/DirectWrite: colors, fonts, a small
//! painter wrapper, the widgets (buttons, segmented track, checkbox…) and their icons.

use engine::{InputMode, TonePlacement, TypingStyle};
use windows::core::{w, Result};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap, ID2D1HwndRenderTarget, ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP,
};

use super::layout::*;

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

const fn rgba(hex: u32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a,
    }
}

const fn rgb(hex: u32) -> D2D1_COLOR_F {
    rgba(hex, 1.0)
}

pub const BACKGROUND: D2D1_COLOR_F = rgb(0xF7F5F3);
pub const WHITE: D2D1_COLOR_F = rgb(0xFFFFFF);
const CORAL: D2D1_COLOR_F = rgb(0xF47C7C);
const CORAL_DARK: D2D1_COLOR_F = rgb(0xE56767);
const SUN: D2D1_COLOR_F = rgb(0xFBD38D);
const TEXT: D2D1_COLOR_F = rgb(0x1F2328);
const TEXT_SOFT: D2D1_COLOR_F = rgb(0x4A4F57);
const MUTED: D2D1_COLOR_F = rgb(0x8A8F98);
const TRACK: D2D1_COLOR_F = rgb(0xE4E5E9);
const BORDER: D2D1_COLOR_F = rgb(0xD9DADF);
const GREY: D2D1_COLOR_F = rgb(0xA6A6A6);
const GREY_DARK: D2D1_COLOR_F = rgb(0x939393);
const BOX_BORDER: D2D1_COLOR_F = rgb(0xEAE6E3);
const CHECK_BORDER: D2D1_COLOR_F = rgb(0x9A9EA6);
const BUTTON_HOVER: D2D1_COLOR_F = rgb(0xF1F2F4);
const HOVER_SHADE: D2D1_COLOR_F = rgba(0x000000, 0.08);
/// All button icons share one brand color and a thin outline style.
const ICON: D2D1_COLOR_F = rgb(0xE06666);
const ICON_STROKE: f32 = 2.0;
/// "Đóng" is the safe, everyday action, so it alone is green.
const ICON_SAFE: D2D1_COLOR_F = rgb(0x4CAF50);

// ---------------------------------------------------------------------------
// Fonts and painter
// ---------------------------------------------------------------------------

pub struct Fonts {
    title: IDWriteTextFormat,
    subtitle: IDWriteTextFormat,
    label: IDWriteTextFormat,
    body: IDWriteTextFormat,
    button: IDWriteTextFormat,
    centered: IDWriteTextFormat,
}

impl Fonts {
    pub fn new() -> Result<Self> {
        // SAFETY: plain factory creation with no borrowed arguments.
        let dw: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let font = |size, weight, align| text_format(&dw, size, weight, align);
        let (left, center) = (DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_CENTER);
        Ok(Self {
            title: font(19.0, DWRITE_FONT_WEIGHT_BOLD, left)?,
            subtitle: font(12.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, left)?,
            label: font(12.0, DWRITE_FONT_WEIGHT_BOLD, left)?,
            body: font(13.5, DWRITE_FONT_WEIGHT_NORMAL, left)?,
            button: font(13.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, left)?,
            centered: font(13.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, center)?,
        })
    }
}

fn text_format(
    dw: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    align: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    // SAFETY: the family and locale names are null-terminated literals.
    unsafe {
        let format = dw.CreateTextFormat(
            w!("Segoe UI"),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("vi-VN"),
        )?;
        format.SetTextAlignment(align)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        Ok(format)
    }
}

/// Thin wrapper over the render target with one reusable brush.
pub struct Painter<'a> {
    pub target: &'a ID2D1HwndRenderTarget,
    pub brush: &'a ID2D1SolidColorBrush,
    pub logo: &'a ID2D1Bitmap,
}

// SAFETY (all methods): the target and brush are live COM objects created
// together, and every pointer passed points to a local that outlives the call.
impl Painter<'_> {
    fn fill(&self, rect: Rect, radius: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target
                .FillRoundedRectangle(&rounded(rect, radius), self.brush);
        }
    }

    fn stroke(&self, rect: Rect, radius: f32, color: D2D1_COLOR_F, width: f32) {
        // Strokes are centred on the edge; inset half a width so they stay
        // inside `rect`.
        let rect = rect.inset(width / 2.0);
        unsafe {
            self.brush.SetColor(&color);
            self.target
                .DrawRoundedRectangle(&rounded(rect, radius), self.brush, width, None);
        }
    }

    fn circle(&self, cx: f32, cy: f32, radius: f32, color: D2D1_COLOR_F) {
        let ellipse = D2D1_ELLIPSE {
            point: point(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillEllipse(&ellipse, self.brush);
        }
    }

    fn polyline(&self, points: &[(f32, f32)], color: D2D1_COLOR_F, width: f32) {
        unsafe {
            self.brush.SetColor(&color);
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                self.target
                    .DrawLine(point(a.0, a.1), point(b.0, b.1), self.brush, width, None);
            }
        }
    }

    /// An arc of radius `r` around (cx, cy), angles in degrees clockwise
    /// from 12 o'clock.
    fn arc(&self, cx: f32, cy: f32, r: f32, degrees: (u32, u32), color: D2D1_COLOR_F) {
        let points: Vec<(f32, f32)> = (degrees.0..=degrees.1)
            .step_by(10)
            .map(|deg| {
                let a = (deg as f32).to_radians();
                (cx + r * a.sin(), cy - r * a.cos())
            })
            .collect();
        self.polyline(&points, color, ICON_STROKE);
    }

    fn text(&self, text: &str, font: &IDWriteTextFormat, rect: Rect, color: D2D1_COLOR_F) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawText(
                &wide,
                font,
                &rect.d2d(),
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    fn logo(&self, rect: Rect) {
        unsafe {
            self.target.DrawBitmap(
                self.logo,
                Some(&rect.d2d()),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
    }

    fn clipped(&self, rect: Rect, draw: impl FnOnce()) {
        unsafe {
            self.target
                .PushAxisAlignedClip(&rect.d2d(), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE)
        };
        draw();
        unsafe { self.target.PopAxisAlignedClip() };
    }
}

fn rounded(rect: Rect, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: rect.d2d(),
        radiusX: radius,
        radiusY: radius,
    }
}

fn point(x: f32, y: f32) -> windows_numerics::Vector2 {
    windows_numerics::Vector2 { X: x, Y: y }
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

/// What the panel shows; read fresh on every paint.
pub struct State {
    pub mode: InputMode,
    pub style: TypingStyle,
    pub tone: TonePlacement,
    pub auto_restore: bool,
    pub autostart: bool,
    pub expanded: bool,
}

pub fn draw(p: &Painter, fonts: &Fonts, state: &State, hover: Option<Control>) {
    let hovered = |c| hover == Some(c);
    let vietnamese = state.mode == InputMode::Vietnamese;

    // Header: coral band with a sun peeking in from the corner.
    let header = Rect::new(0.0, 0.0, WIDTH, HEADER);
    p.fill(header, 0.0, CORAL);
    p.clipped(header, || p.circle(WIDTH - 20.0, -10.0, 90.0, SUN));
    p.circle(MARGIN + 24.0, HEADER / 2.0, 24.0, WHITE);
    p.logo(Rect::new(MARGIN + 5.0, HEADER / 2.0 - 19.0, 38.0, 38.0));
    p.text(
        crate::APP_NAME,
        &fonts.title,
        Rect::new(84.0, 12.0, 240.0, 28.0),
        WHITE,
    );
    let subtitle = "Bộ gõ tiếng Việt mã nguồn mở";
    p.text(
        subtitle,
        &fonts.subtitle,
        Rect::new(84.0, 40.0, 260.0, 20.0),
        WHITE,
    );
    if hovered(Control::Close) {
        p.fill(CLOSE, CLOSE.w / 2.0, HOVER_SHADE);
    }
    let (cx, cy, arm) = (CLOSE.x + CLOSE.w / 2.0, CLOSE.y + CLOSE.h / 2.0, 6.0);
    p.polyline(&[(cx - arm, cy - arm), (cx + arm, cy + arm)], TEXT, 1.6);
    p.polyline(&[(cx - arm, cy + arm), (cx + arm, cy - arm)], TEXT, 1.6);

    // Main controls: typing style and Vietnamese/English.
    section(p, fonts, "ĐIỀU KHIỂN", CONTROL_LABEL_Y, CONTROL_BOX);
    caption(p, fonts, "Kiểu gõ:", STYLE_TRACK);
    segmented(
        p,
        fonts,
        STYLE_TRACK,
        &STYLES,
        state.style,
        hover,
        Control::Style,
    );
    caption(p, fonts, "Chế độ:", MODE_PILL);
    let pill_color = match (vietnamese, hovered(Control::Mode)) {
        (true, false) => CORAL,
        (true, true) => CORAL_DARK,
        (false, false) => GREY,
        (false, true) => GREY_DARK,
    };
    p.fill(MODE_PILL, MODE_PILL.h / 2.0, pill_color);
    let pill_label = if vietnamese {
        "TIẾNG VIỆT (V)"
    } else {
        "TIẾNG ANH (E)"
    };
    p.text(pill_label, &fonts.centered, MODE_PILL, WHITE);
    let hint = Rect::new(
        MODE_PILL.x + MODE_PILL.w + 12.0,
        MODE_PILL.y,
        110.0,
        MODE_PILL.h,
    );
    p.text("Ctrl + Shift", &fonts.body, hint, MUTED);

    // Action buttons: one outline icon set in the brand color.
    let hide = hovered(Control::Hide);
    button(p, fonts, HIDE_BUTTON, hide, "Đóng", |x, y| {
        icon_to_tray(p, x, y)
    });
    let quit = hovered(Control::Quit);
    button(p, fonts, QUIT_BUTTON, quit, "Kết thúc", |x, y| {
        icon_power(p, x, y)
    });
    let expand_label = if state.expanded {
        "Thu gọn"
    } else {
        "Mở rộng"
    };
    let expand = hovered(Control::Expand);
    button(p, fonts, EXPAND_BUTTON, expand, expand_label, |x, y| {
        icon_chevrons(p, x, y, state.expanded)
    });

    if state.expanded {
        draw_advanced(p, fonts, state, hover);
    }

    // Footer.
    let about = about_button(state.expanded);
    let about_hover = hovered(Control::About);
    button(p, fonts, about, about_hover, "Thông tin", |x, y| {
        icon_note(p, x, y)
    });
    let defaults = defaults_button(state.expanded);
    let defaults_hover = hovered(Control::Defaults);
    button(
        p,
        fonts,
        defaults,
        defaults_hover,
        "Mặc định",
        |x, y| icon_undo(p, x, y),
    );
    let version = format!("v{} · GNU GPL", env!("CARGO_PKG_VERSION"));
    let note = Rect::new(
        defaults.x + defaults.w + 16.0,
        defaults.y,
        160.0,
        defaults.h,
    );
    p.text(&version, &fonts.body, note, MUTED);
}

fn draw_advanced(p: &Painter, fonts: &Fonts, state: &State, hover: Option<Control>) {
    let hovered = |c| hover == Some(c);

    section(p, fonts, "TÙY CHỌN GÕ", OPTIONS_LABEL_Y, OPTIONS_BOX);
    checkbox(
        p,
        fonts,
        RESTORE_ROW,
        "Tự khôi phục phím với từ sai (windows, Usb…)",
        state.auto_restore,
        hovered(Control::AutoRestore),
    );
    checkbox(
        p,
        fonts,
        CLASSIC_TONE_ROW,
        "Đặt dấu kiểu cũ: hoà, thuý (thay vì hòa, thúy)",
        state.tone == TonePlacement::Classic,
        hovered(Control::ClassicTone),
    );

    section(p, fonts, "HỆ THỐNG", SYSTEM_LABEL_Y, SYSTEM_BOX);
    checkbox(
        p,
        fonts,
        AUTOSTART_ROW,
        "Khởi động cùng Windows",
        state.autostart,
        hovered(Control::Autostart),
    );
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

/// A bold heading above a white rounded box.
fn section(p: &Painter, fonts: &Fonts, title: &str, label_y: f32, rect: Rect) {
    p.text(
        title,
        &fonts.label,
        Rect::new(rect.x, label_y, 200.0, 20.0),
        TEXT,
    );
    p.fill(rect, 10.0, WHITE);
    p.stroke(rect, 10.0, BOX_BORDER, 1.0);
}

/// A row caption to the left of `control`, inside the "Điều khiển" box.
fn caption(p: &Painter, fonts: &Fonts, text: &str, control: Rect) {
    let rect = Rect::new(CAPTION_X, control.y, control.x - CAPTION_X, control.h);
    p.text(text, &fonts.body, rect, TEXT);
}

// ---------------------------------------------------------------------------
// Button icons: ~16 DIP outline glyphs centred on (x, y)
// ---------------------------------------------------------------------------

/// "Đóng": an arrow dropping into a tray (the app keeps running there).
fn icon_to_tray(p: &Painter, x: f32, y: f32) {
    let tray = [
        (x - 8.0, y + 1.0),
        (x - 8.0, y + 7.0),
        (x + 8.0, y + 7.0),
        (x + 8.0, y + 1.0),
    ];
    p.polyline(&tray, ICON_SAFE, ICON_STROKE);
    p.polyline(&[(x, y - 8.0), (x, y + 3.0)], ICON_SAFE, ICON_STROKE);
    p.polyline(
        &[(x - 4.0, y - 1.0), (x, y + 3.0), (x + 4.0, y - 1.0)],
        ICON_SAFE,
        ICON_STROKE,
    );
}

/// "Kết thúc": the power symbol.
fn icon_power(p: &Painter, x: f32, y: f32) {
    p.arc(x, y + 1.0, 7.0, (40, 320), ICON);
    p.polyline(&[(x, y - 8.0), (x, y)], ICON, ICON_STROKE);
}

/// "Mở rộng" / "Thu gọn": a double chevron pointing where the panel goes.
fn icon_chevrons(p: &Painter, x: f32, y: f32, expanded: bool) {
    let dir = if expanded { -1.0 } else { 1.0 };
    for offset in [-4.0, 2.0] {
        let (top, tip) = (y + (offset - 2.5) * dir, y + (offset + 2.5) * dir);
        p.polyline(
            &[(x - 6.0, top), (x, tip), (x + 6.0, top)],
            ICON,
            ICON_STROKE,
        );
    }
}

/// "Thông tin": a small note card with two lines of text.
fn icon_note(p: &Painter, x: f32, y: f32) {
    p.stroke(
        Rect::new(x - 8.0, y - 7.0, 16.0, 14.0),
        3.0,
        ICON,
        ICON_STROKE,
    );
    p.polyline(&[(x - 4.0, y - 2.0), (x + 4.0, y - 2.0)], ICON, ICON_STROKE);
    p.polyline(&[(x - 4.0, y + 2.5), (x + 1.5, y + 2.5)], ICON, ICON_STROKE);
}

/// "Mặc định": an undo arrow (↶) — back to the starting settings.
fn icon_undo(p: &Painter, x: f32, y: f32) {
    p.polyline(&[(x + 1.0, y - 5.0), (x - 7.0, y - 5.0)], ICON, ICON_STROKE);
    let head = [(x - 3.5, y - 8.5), (x - 7.0, y - 5.0), (x - 3.5, y - 1.5)];
    p.polyline(&head, ICON, ICON_STROKE);
    p.arc(x + 1.0, y + 1.0, 6.0, (0, 180), ICON);
    p.polyline(&[(x + 1.0, y + 7.0), (x - 4.0, y + 7.0)], ICON, ICON_STROKE);
}

/// A white button with an icon (drawn by `icon` around its centre point)
/// followed by `label`, both left-aligned.
fn button(
    p: &Painter,
    fonts: &Fonts,
    rect: Rect,
    hovered: bool,
    label: &str,
    icon: impl FnOnce(f32, f32),
) {
    p.fill(rect, 8.0, if hovered { BUTTON_HOVER } else { WHITE });
    p.stroke(rect, 8.0, if hovered { CHECK_BORDER } else { BORDER }, 1.0);
    icon(rect.x + 22.0, rect.y + rect.h / 2.0);
    let text = Rect::new(rect.x + 40.0, rect.y, rect.w - 44.0, rect.h);
    p.text(label, &fonts.button, text, TEXT);
}

fn segmented<T: Copy + PartialEq>(
    p: &Painter,
    fonts: &Fonts,
    track: Rect,
    options: &[(T, &str)],
    selected: T,
    hover: Option<Control>,
    control: fn(T) -> Control,
) {
    p.fill(track, track.h / 2.0, TRACK);
    for (i, (value, label)) in options.iter().enumerate() {
        let slot = track.segment(i, options.len());
        let color = if *value == selected {
            p.fill(slot, slot.h / 2.0, WHITE);
            p.stroke(slot, slot.h / 2.0, BORDER, 1.0);
            TEXT
        } else if hover == Some(control(*value)) {
            TEXT_SOFT
        } else {
            MUTED
        };
        p.text(label, &fonts.centered, slot, color);
    }
}

fn checkbox(p: &Painter, fonts: &Fonts, row: Rect, label: &str, checked: bool, hovered: bool) {
    let bx = Rect::new(row.x, row.y + (row.h - 20.0) / 2.0, 20.0, 20.0);
    p.fill(bx, 5.0, WHITE);
    p.stroke(bx, 5.0, mark_border(checked, hovered), 1.5);
    if checked {
        let (x, y) = (bx.x, bx.y);
        p.polyline(
            &[
                (x + 4.5, y + 10.5),
                (x + 8.5, y + 14.5),
                (x + 15.5, y + 6.0),
            ],
            CORAL,
            2.2,
        );
    }
    let text = Rect::new(row.x + 30.0, row.y, row.w - 30.0, row.h);
    p.text(label, &fonts.body, text, TEXT);
}

fn mark_border(on: bool, hovered: bool) -> D2D1_COLOR_F {
    match (on, hovered) {
        (true, _) => CORAL,
        (false, true) => TEXT_SOFT,
        (false, false) => CHECK_BORDER,
    }
}

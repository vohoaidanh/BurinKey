//! Where everything sits on the panel, in DIPs. The panel has a compact view
//! (main controls only) and an expanded view that
//! adds the advanced options above the footer.

use engine::TypingStyle;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

pub const WIDTH: f32 = 540.0;
pub const MARGIN: f32 = 20.0;
pub const HEADER: f32 = 72.0;
const PADDING: f32 = 14.0;
const FULL: f32 = WIDTH - 2.0 * MARGIN;

pub const CLOSE: Rect = Rect::new(WIDTH - 44.0, 18.0, 28.0, 28.0);

// "Điều khiển" box on the left, action buttons on the right.
pub const CONTROL_LABEL_Y: f32 = 84.0;
pub const CONTROL_BOX: Rect = Rect::new(MARGIN, 106.0, 364.0, 114.0);
pub const CAPTION_X: f32 = MARGIN + PADDING;
pub const STYLE_TRACK: Rect = Rect::new(108.0, 116.0, 264.0, 40.0);
pub const MODE_PILL: Rect = Rect::new(108.0, 168.0, 150.0, 40.0);

const SIDE_X: f32 = MARGIN + 364.0 + 16.0;
const SIDE_W: f32 = WIDTH - MARGIN - SIDE_X;
pub const HIDE_BUTTON: Rect = Rect::new(SIDE_X, 106.0, SIDE_W, 34.0);
pub const QUIT_BUTTON: Rect = Rect::new(SIDE_X, 146.0, SIDE_W, 34.0);
pub const EXPAND_BUTTON: Rect = Rect::new(SIDE_X, 186.0, SIDE_W, 34.0);

// Advanced options, only in the expanded view.
pub const OPTIONS_LABEL_Y: f32 = 238.0;
pub const OPTIONS_BOX: Rect = Rect::new(MARGIN, 260.0, FULL, 72.0);
pub const RESTORE_ROW: Rect = Rect::new(MARGIN + PADDING, 268.0, FULL - 2.0 * PADDING, 28.0);
pub const CLASSIC_TONE_ROW: Rect = Rect::new(MARGIN + PADDING, 298.0, FULL - 2.0 * PADDING, 28.0);
pub const SYSTEM_LABEL_Y: f32 = 344.0;
pub const SYSTEM_BOX: Rect = Rect::new(MARGIN, 366.0, FULL, 44.0);
pub const AUTOSTART_ROW: Rect = Rect::new(MARGIN + PADDING, 374.0, FULL - 2.0 * PADDING, 28.0);

const FOOTER_COMPACT_Y: f32 = 238.0;
const FOOTER_EXPANDED_Y: f32 = 428.0;
const FOOTER_BUTTON: (f32, f32) = (130.0, 34.0);

pub const STYLES: [(TypingStyle, &str); 3] = [
    (TypingStyle::Telex, "Telex"),
    (TypingStyle::SimpleTelex, "Simple Telex"),
    (TypingStyle::Vni, "VNI"),
];

/// Everything on the panel that reacts to the mouse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Close,
    Style(TypingStyle),
    Mode,
    Hide,
    Quit,
    Expand,
    ClassicTone,
    AutoRestore,
    Autostart,
    About,
    Defaults,
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn inset(self, d: f32) -> Self {
        Self::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }

    /// The `index`-th of `count` equal slots inside a segmented track.
    pub fn segment(self, index: usize, count: usize) -> Self {
        let inner = self.inset(4.0);
        let w = inner.w / count as f32;
        Self::new(inner.x + w * index as f32, inner.y, w, inner.h)
    }

    pub fn d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.w,
            bottom: self.y + self.h,
        }
    }
}

fn footer_y(expanded: bool) -> f32 {
    if expanded {
        FOOTER_EXPANDED_Y
    } else {
        FOOTER_COMPACT_Y
    }
}

pub fn about_button(expanded: bool) -> Rect {
    let (w, h) = FOOTER_BUTTON;
    Rect::new(MARGIN, footer_y(expanded), w, h)
}

pub fn defaults_button(expanded: bool) -> Rect {
    let (w, h) = FOOTER_BUTTON;
    Rect::new(MARGIN + w + 10.0, footer_y(expanded), w, h)
}

pub fn height(expanded: bool) -> f32 {
    footer_y(expanded) + FOOTER_BUTTON.1 + MARGIN
}

fn controls(expanded: bool) -> Vec<(Control, Rect)> {
    let mut list = vec![
        (Control::Close, CLOSE),
        (Control::Mode, MODE_PILL),
        (Control::Hide, HIDE_BUTTON),
        (Control::Quit, QUIT_BUTTON),
        (Control::Expand, EXPAND_BUTTON),
        (Control::About, about_button(expanded)),
        (Control::Defaults, defaults_button(expanded)),
    ];
    for (i, (style, _)) in STYLES.iter().enumerate() {
        list.push((Control::Style(*style), STYLE_TRACK.segment(i, STYLES.len())));
    }
    if expanded {
        list.push((Control::ClassicTone, CLASSIC_TONE_ROW));
        list.push((Control::AutoRestore, RESTORE_ROW));
        list.push((Control::Autostart, AUTOSTART_ROW));
    }
    list
}

pub fn control_at(expanded: bool, x: f32, y: f32) -> Option<Control> {
    controls(expanded)
        .into_iter()
        .find(|(_, rect)| rect.contains(x, y))
        .map(|(control, _)| control)
}

//! Icons drawn in code, so there are no image assets to ship:
//!
//! - the tray icon, a "V" (Vietnamese, coral) or "E" (English, grey) badge
//!   readable at 16 px;
//! - the logo, a pink pig, used for the window icon and the panel header.
//!
//! Each icon is a function from a point in the unit square to a color; every
//! pixel is supersampled 4×4 so edges stay smooth at any size.

use windows::core::Result;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

// ---------------------------------------------------------------------------
// Tray badge: V / E
// ---------------------------------------------------------------------------

const BADGE_VIETNAMESE: u32 = 0xF06A6A;
const BADGE_ENGLISH: u32 = 0x7D828B;
const LETTER: u32 = 0xFFFFFF;
const STROKE: f32 = 0.075; // half the letter's stroke width
type Segment = ((f32, f32), (f32, f32));

const LETTER_V: &[Segment] = &[((0.27, 0.24), (0.5, 0.77)), ((0.73, 0.24), (0.5, 0.77))];
const LETTER_E: &[Segment] = &[
    ((0.33, 0.23), (0.33, 0.77)),
    ((0.33, 0.23), (0.70, 0.23)),
    ((0.33, 0.50), (0.64, 0.50)),
    ((0.33, 0.77), (0.70, 0.77)),
];

fn distance_to_segment(x: f32, y: f32, ((ax, ay), (bx, by)): Segment) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((x - ax - t * dx).powi(2) + (y - ay - t * dy).powi(2)).sqrt()
}

/// Inside a square filling the icon, with corners rounded by `radius`.
fn in_rounded_square(x: f32, y: f32, radius: f32) -> bool {
    let qx = (x - 0.5).abs() - (0.5 - radius);
    let qy = (y - 0.5).abs() - (0.5 - radius);
    qx.max(0.0).hypot(qy.max(0.0)) <= radius
}

fn badge_color(vietnamese: bool, x: f32, y: f32) -> Option<u32> {
    if !in_rounded_square(x, y, 0.22) {
        return None;
    }
    let (letter, background) = if vietnamese {
        (LETTER_V, BADGE_VIETNAMESE)
    } else {
        (LETTER_E, BADGE_ENGLISH)
    };
    let on_letter = letter
        .iter()
        .any(|&segment| distance_to_segment(x, y, segment) <= STROKE);
    Some(if on_letter { LETTER } else { background })
}

/// The tray icon for the current mode. The caller owns it (`DestroyIcon`).
pub fn mode_badge(size: u32, vietnamese: bool) -> Result<HICON> {
    to_icon(size, &rasterize(size, |x, y| badge_color(vietnamese, x, y)))
}

// ---------------------------------------------------------------------------
// Logo: pink pig
// ---------------------------------------------------------------------------

const PINK: u32 = 0xF9A8B8;
const EAR_INSIDE: u32 = 0xF48AA2;
const OUTLINE: u32 = 0xD45A75;
const SNOUT: u32 = 0xF27B97;
const CHEEK: u32 = 0xF7899F;
const DARK: u32 = 0x4A2530;

/// An ellipse in unit coordinates, rotated by `angle` radians.
struct Ellipse {
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    angle: f32,
}

impl Ellipse {
    const fn new(cx: f32, cy: f32, rx: f32, ry: f32, angle: f32) -> Self {
        Self {
            cx,
            cy,
            rx,
            ry,
            angle,
        }
    }

    /// The same ellipse grown by `d` on every side (for outlines).
    const fn grown(&self, d: f32) -> Self {
        Self::new(self.cx, self.cy, self.rx + d, self.ry + d, self.angle)
    }

    /// The same ellipse mirrored across the vertical centre line.
    const fn mirrored(&self) -> Self {
        Self::new(1.0 - self.cx, self.cy, self.rx, self.ry, -self.angle)
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        let (dx, dy) = (x - self.cx, y - self.cy);
        let (sin, cos) = self.angle.sin_cos();
        let u = dx * cos + dy * sin;
        let v = -dx * sin + dy * cos;
        (u / self.rx).powi(2) + (v / self.ry).powi(2) <= 1.0
    }
}

const OUTLINE_WIDTH: f32 = 0.045;
const HEAD: Ellipse = Ellipse::new(0.5, 0.56, 0.42, 0.37, 0.0);
const EAR: Ellipse = Ellipse::new(0.22, 0.25, 0.12, 0.17, -0.55);
const EAR_INNER: Ellipse = Ellipse::new(0.23, 0.27, 0.06, 0.10, -0.55);
const CHEEK_LEFT: Ellipse = Ellipse::new(0.22, 0.66, 0.07, 0.05, 0.0);
const SNOUT_SHAPE: Ellipse = Ellipse::new(0.5, 0.67, 0.18, 0.125, 0.0);
const NOSTRIL: Ellipse = Ellipse::new(0.44, 0.67, 0.033, 0.05, 0.0);
const EYE: Ellipse = Ellipse::new(0.33, 0.47, 0.05, 0.055, 0.0);

/// Paint order, back to front: the last shape containing a point wins.
fn pig_layers() -> [(Ellipse, u32); 16] {
    [
        (EAR.grown(OUTLINE_WIDTH), OUTLINE),
        (EAR.mirrored().grown(OUTLINE_WIDTH), OUTLINE),
        (HEAD.grown(OUTLINE_WIDTH), OUTLINE),
        (EAR, PINK),
        (EAR.mirrored(), PINK),
        (EAR_INNER, EAR_INSIDE),
        (EAR_INNER.mirrored(), EAR_INSIDE),
        (HEAD, PINK),
        (CHEEK_LEFT, CHEEK),
        (CHEEK_LEFT.mirrored(), CHEEK),
        (SNOUT_SHAPE.grown(OUTLINE_WIDTH / 2.0), OUTLINE),
        (SNOUT_SHAPE, SNOUT),
        (NOSTRIL, DARK),
        (NOSTRIL.mirrored(), DARK),
        (EYE, DARK),
        (EYE.mirrored(), DARK),
    ]
}

/// The pig as straight-alpha `0xAARRGGBB` pixels, top row first.
pub fn pig_pixels(size: u32) -> Vec<u32> {
    let layers = pig_layers();
    rasterize(size, |x, y| {
        layers
            .iter()
            .rev()
            .find(|(shape, _)| shape.contains(x, y))
            .map(|(_, rgb)| *rgb)
    })
}

/// The pig as an icon. The caller owns it (`DestroyIcon`).
pub fn pig(size: u32) -> Result<HICON> {
    to_icon(size, &pig_pixels(size))
}

// ---------------------------------------------------------------------------
// Rasterizing
// ---------------------------------------------------------------------------

/// Samples `color_at` (unit coordinates → `0xRRGGBB`, `None` = transparent)
/// into straight-alpha `0xAARRGGBB` pixels, row-major, top row first.
fn rasterize(size: u32, color_at: impl Fn(f32, f32) -> Option<u32>) -> Vec<u32> {
    const SAMPLES: u32 = 4;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut sum, mut hits) = ([0u32; 3], 0u32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    if let Some(rgb) = color_at(x, y) {
                        sum[0] += (rgb >> 16) & 0xff;
                        sum[1] += (rgb >> 8) & 0xff;
                        sum[2] += rgb & 0xff;
                        hits += 1;
                    }
                }
            }
            pixels.push(if hits == 0 {
                0
            } else {
                let [r, g, b] = sum.map(|v| (v + hits / 2) / hits);
                let a = 255 * hits / (SAMPLES * SAMPLES);
                (a << 24) | (r << 16) | (g << 8) | b
            });
        }
    }
    pixels
}

fn to_icon(size: u32, pixels: &[u32]) -> Result<HICON> {
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size as i32,
        biHeight: -(size as i32), // negative = top-down rows
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO {
        bmiHeader: header,
        ..Default::default()
    };
    // SAFETY: the DIB section is exactly size×size 32-bit pixels, matching
    // `pixels`; both bitmaps are deleted once the icon has copied them.
    unsafe {
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
        // With a 32-bit color bitmap the alpha channel decides transparency;
        // the mask just has to exist.
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, None);
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            hbmMask: mask,
            hbmColor: color,
            ..Default::default()
        });
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        icon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(pixels: &[u32], size: u32, x: f32, y: f32) -> u32 {
        let (px, py) = ((x * size as f32) as u32, (y * size as f32) as u32);
        pixels[(py * size + px) as usize]
    }

    #[test]
    fn pig_corners_are_transparent_and_face_is_opaque() {
        let pixels = pig_pixels(32);
        assert_eq!(at(&pixels, 32, 0.0, 0.99) >> 24, 0);
        assert_eq!(at(&pixels, 32, 0.99, 0.99) >> 24, 0);
        assert_eq!(at(&pixels, 32, 0.5, 0.45) >> 24, 255);
    }

    #[test]
    fn badges_show_their_letter() {
        let color = |vietnamese, x, y| badge_color(vietnamese, x, y);
        // V: the bottom point is white; the middle of the top is background.
        assert_eq!(color(true, 0.5, 0.74), Some(LETTER));
        assert_eq!(color(true, 0.5, 0.3), Some(BADGE_VIETNAMESE));
        // E: the spine is white; the open right side is background.
        assert_eq!(color(false, 0.33, 0.6), Some(LETTER));
        assert_eq!(color(false, 0.62, 0.64), Some(BADGE_ENGLISH));
        // Rounded corners stay transparent.
        assert_eq!(color(true, 0.01, 0.01), None);
    }
}

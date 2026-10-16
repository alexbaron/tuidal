//! composite_bg.rs — Spectrum background for terminals without a z-index
//! (Sixel, iTerm2): the spectrum curve is drawn in pixels on top of the
//! background image and the whole frame is sent through ratatui-image.

/// Line colour of the spectrum curve (same as ui::ACCENT).
pub const CURVE_RGB: [u8; 3] = [99, 202, 183];
/// Alpha (0..=255) of the fill under the curve.
pub const FILL_ALPHA: u8 = 80;
/// Line thickness in pixels.
pub const LINE_PX: u32 = 2;

/// Draw `bands` (values in 0..=1, one per band, evenly spread on the x axis,
/// linearly interpolated between band centres) onto a copy of `bg`.
/// y = h * (1 - v). Fill under the curve with CURVE_RGB blended at FILL_ALPHA,
/// then the curve itself LINE_PX thick in solid CURVE_RGB (vertical segment
/// joining neighbouring columns so steep slopes have no gaps).
/// Returns `bg` unchanged (cloned) when `bands` is empty; a single band
/// draws a flat line at that value.
pub fn draw_spectrum(bg: &image::RgbaImage, bands: &[f32]) -> image::RgbaImage {
    let mut out = bg.clone();
    let (w, h) = out.dimensions();
    if bands.is_empty() || w == 0 || h == 0 {
        return out;
    }
    let n = bands.len();
    let value_at = |x: u32| -> f32 {
        if n == 1 {
            return bands[0].clamp(0.0, 1.0);
        }
        let pos = x as f32 / (w - 1).max(1) as f32 * (n - 1) as f32;
        let i = (pos.floor() as usize).min(n - 2);
        let frac = pos - i as f32;
        (bands[i] * (1.0 - frac) + bands[i + 1] * frac).clamp(0.0, 1.0)
    };

    let mut prev_top: Option<u32> = None;
    for x in 0..w {
        let v = value_at(x);
        let y = (h as f32 * (1.0 - v)).round() as i64;

        // Translucent fill under the curve.
        let fill_start = y.clamp(0, h as i64) as u32;
        for yy in fill_start..h {
            let px = out.get_pixel_mut(x, yy);
            for c in 0..3 {
                px[c] = ((px[c] as u16 * (255 - FILL_ALPHA as u16)
                    + CURVE_RGB[c] as u16 * FILL_ALPHA as u16)
                    / 255) as u8;
            }
        }

        // Solid line, LINE_PX thick, joined to the previous column so steep
        // slopes have no gaps.
        let top = y.clamp(0, (h - 1) as i64) as u32;
        let (lo, hi) = match prev_top {
            Some(p) => (top.min(p), (top + LINE_PX).max(p + LINE_PX)),
            None => (top, top + LINE_PX),
        };
        for yy in lo..=hi.min(h - 1) {
            let px = out.get_pixel_mut(x, yy);
            px[0] = CURVE_RGB[0];
            px[1] = CURVE_RGB[1];
            px[2] = CURVE_RGB[2];
            px[3] = 255;
        }
        prev_top = Some(top);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn empty_bands_returns_identical_image() {
        let bg = RgbaImage::from_pixel(40, 20, Rgba([10, 20, 30, 255]));
        let out = draw_spectrum(&bg, &[]);
        assert_eq!(out, bg);
    }

    #[test]
    fn full_bands_draw_line_at_top_and_fill() {
        let bg = RgbaImage::from_pixel(100, 50, Rgba([0, 0, 0, 255]));
        let bands = [1.0; 10];
        let out = draw_spectrum(&bg, &bands);
        let line_px = out.get_pixel(50, 2);
        assert_eq!(line_px[0], CURVE_RGB[0]);
        assert_eq!(line_px[1], CURVE_RGB[1]);
        assert_eq!(line_px[2], CURVE_RGB[2]);
        let fill_px = out.get_pixel(50, 25);
        let expected_g = 202 * FILL_ALPHA as u32 / 255;
        assert!(
            (fill_px[1] as i32 - expected_g as i32).abs() <= 3,
            "green channel {} expected ~{expected_g}",
            fill_px[1]
        );
    }

    #[test]
    fn zero_bands_leave_middle_black_and_color_bottom() {
        let bg = RgbaImage::from_pixel(100, 50, Rgba([0, 0, 0, 255]));
        let bands = [0.0; 10];
        let out = draw_spectrum(&bg, &bands);
        let mid_px = out.get_pixel(50, 10);
        assert_eq!(*mid_px, Rgba([0, 0, 0, 255]));
        let bottom_px = out.get_pixel(50, 49);
        assert_eq!(bottom_px[0], CURVE_RGB[0]);
        assert_eq!(bottom_px[1], CURVE_RGB[1]);
        assert_eq!(bottom_px[2], CURVE_RGB[2]);
    }
}

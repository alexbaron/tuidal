//! kitty_bg.rs — Background image behind the spectrum chart, emitted through
//! the kitty graphics protocol (APC sequences) directly on stdout.
//!
//! ratatui-image renders via Unicode placeholders, which cannot sit behind
//! text, so we speak the protocol ourselves: transmit the PNG once, then
//! re-place it with z = -2^30 - 1 (below both text and cell backgrounds).

use base64::Engine;
use image::{DynamicImage, GenericImageView};
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

/// Fixed kitty image id for the spectrum background.
pub const BG_IMAGE_ID: u32 = 0x7A1D;
/// RGB multiplier applied to the image so the chart stays readable.
pub const DARKEN: f32 = 0.75;
const CHUNK_SIZE: usize = 4096;
/// z value below -2^30: renders under text AND cell background colors.
const Z_INDEX: i32 = -1_073_741_825;

/// Horizontal blue→magenta gradient used as the built-in background.
fn make_gradient(w: u32, h: u32) -> DynamicImage {
    let mut img = image::RgbaImage::new(w, h);
    for (x, _y, px) in img.enumerate_pixels_mut() {
        let t = x as f32 / w.max(1) as f32;
        // blue (30,60,200) -> magenta (200,50,140)
        px[0] = (30.0 + (200.0 - 30.0) * t) as u8;
        px[1] = (60.0 + (50.0 - 60.0) * t) as u8;
        px[2] = (200.0 + (140.0 - 200.0) * t) as u8;
        px[3] = 255;
    }
    DynamicImage::ImageRgba8(img)
}

/// Built-in fallback background (1280×720 gradient), lazily generated and
/// shared so no image ever has to be decoded before something can display.
pub fn default_background() -> &'static DynamicImage {
    static DEFAULT_BG: std::sync::OnceLock<DynamicImage> = std::sync::OnceLock::new();
    DEFAULT_BG.get_or_init(|| make_gradient(1280, 720))
}

/// Append one diagnostic line to /tmp/tuidal-bg.log when TUIDAL_BG_DEBUG is
/// set in the environment; a no-op otherwise.
pub fn bg_log(msg: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let enabled = *ENABLED.get_or_init(|| std::env::var_os("TUIDAL_BG_DEBUG").is_some());
    if enabled {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/tuidal-bg.log")
        {
            use std::io::Write as _;
            let _ = writeln!(f, "{msg}");
        }
    }
}

/// Where the background image comes from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpectrumBg {
    None,
    Cover,
    Wallhaven,
}

impl SpectrumBg {
    pub fn cycle(self) -> Self {
        match self {
            SpectrumBg::None => SpectrumBg::Cover,
            SpectrumBg::Cover => SpectrumBg::Wallhaven,
            SpectrumBg::Wallhaven => SpectrumBg::None,
        }
    }
}

/// How the source image is composed into the (px_w x px_h) background.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BgLayout {
    /// Center-crop the source so it covers the whole area.
    Cover,
    /// Sharp foreground scaled to full height over a heavily blurred,
    /// darkened cover-crop backdrop (used for square album covers).
    FitHeight,
}

fn darken(px: &mut image::Rgba<u8>, factor: f32) {
    px[0] = (px[0] as f32 * factor) as u8;
    px[1] = (px[1] as f32 * factor) as u8;
    px[2] = (px[2] as f32 * factor) as u8;
}

/// Classic cover-crop fill of the whole area, darkened by DARKEN.
fn compose_cover(img: &DynamicImage, px_w: u32, px_h: u32) -> image::RgbaImage {
    let (src_w, src_h) = img.dimensions();
    let (rw, rh, cx, cy) = cover_crop_size(src_w, src_h, px_w, px_h);
    let mut cropped = img
        .resize_exact(rw, rh, image::imageops::FilterType::Triangle)
        .crop_imm(cx, cy, px_w, px_h)
        .to_rgba8();
    for px in cropped.pixels_mut() {
        darken(px, DARKEN);
    }
    cropped
}

/// Sharp full-height foreground over a blurred, darker backdrop.
fn compose_fit_height(img: &DynamicImage, px_w: u32, px_h: u32) -> image::RgbaImage {
    let (src_w, src_h) = img.dimensions();

    // Backdrop: cover-crop at quarter resolution, blur, upscale, darken hard.
    // Same look as a heavy blur at ~16x less work.
    let (rw, rh, cx, cy) = cover_crop_size(src_w, src_h, px_w / 4, px_h / 4);
    let mut bg = img
        .resize_exact(rw, rh, image::imageops::FilterType::Triangle)
        .crop_imm(cx, cy, px_w / 4, px_h / 4)
        .blur(3.0)
        .resize_exact(px_w, px_h, image::imageops::FilterType::Triangle)
        .to_rgba8();
    for px in bg.pixels_mut() {
        darken(px, 0.45);
    }

    // Foreground: scaled to full height at native aspect, no blur; if it
    // overflows the width, center-crop it.
    let mut fw = (px_h as u64 * src_w as u64 / src_h as u64).max(1) as u32;
    let mut fg = img
        .resize_exact(fw, px_h, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    if fw > px_w {
        fg = image::imageops::crop_imm(&fg, (fw - px_w) / 2, 0, px_w, px_h).to_image();
        fw = px_w;
    }
    for px in fg.pixels_mut() {
        darken(px, DARKEN);
    }

    image::imageops::overlay(&mut bg, &fg, ((px_w - fw) / 2) as i64, 0);
    bg
}

/// Compose `img` into a (px_w x px_h) background per `layout`.
pub fn compose(img: &DynamicImage, px_w: u32, px_h: u32, layout: BgLayout) -> image::RgbaImage {
    match layout {
        BgLayout::Cover => compose_cover(img, px_w, px_h),
        BgLayout::FitHeight => compose_fit_height(img, px_w, px_h),
    }
}

/// Compose `img` into a (px_w x px_h) background per `layout`, PNG-encode it
/// and return the chunked kitty transmit sequence.
pub fn transmit_sequence(
    img: &DynamicImage,
    px_w: u32,
    px_h: u32,
    layout: BgLayout,
) -> Result<String, image::ImageError> {
    let composed = compose(img, px_w, px_h, layout);
    let mut png = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png);
    composed.write_to(&mut cursor, image::ImageFormat::Png)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(chunk_payload(&b64).concat())
}

/// Cursor save + position + place + restore for `area` (0-based, as Rect).
pub fn place_sequence(area: Rect) -> String {
    format!(
        "\x1b7\x1b[{};{}H\x1b_Ga=p,i={BG_IMAGE_ID},p=1,z={Z_INDEX},c={},r={},q=2\x1b\\\x1b8",
        area.y + 1,
        area.x + 1,
        area.width,
        area.height,
    )
}

/// Delete the image data and all its placements.
pub fn delete_sequence() -> String {
    format!("\x1b_Ga=d,d=I,i={BG_IMAGE_ID},q=2\x1b\\")
}

/// Split a base64 payload into kitty APC chunks: control keys only on the
/// first chunk, `m=1` on every chunk but the last (`m=0`).
pub(crate) fn chunk_payload(b64: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut rest = b64;
    while !rest.is_empty() {
        let n = rest.len().min(CHUNK_SIZE);
        let (head, tail) = rest.split_at(n);
        let last = tail.is_empty();
        let keys = if chunks.is_empty() {
            format!("a=t,i={BG_IMAGE_ID},f=100,q=2,")
        } else {
            String::new()
        };
        chunks.push(format!(
            "\x1b_G{keys}m={};{head}\x1b\\",
            if last { 0 } else { 1 }
        ));
        rest = tail;
    }
    if chunks.is_empty() {
        chunks.push(format!("\x1b_Ga=t,i={BG_IMAGE_ID},f=100,q=2,m=0;\x1b\\"));
    }
    chunks
}

/// Cover-fit geometry: resize `src` uniformly so it covers `dst`, then crop
/// the overflow around the center. Returns (resize_w, resize_h, crop_x, crop_y).
pub(crate) fn cover_crop_size(src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> (u32, u32, u32, u32) {
    let scale = (dst_w as f64 / src_w as f64).max(dst_h as f64 / src_h as f64);
    let rw = ((src_w as f64) * scale).round() as u32;
    let rh = ((src_h as f64) * scale).round() as u32;
    let cx = rw.saturating_sub(dst_w) / 2;
    let cy = rh.saturating_sub(dst_h) / 2;
    (rw, rh, cx, cy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_split_and_flags() {
        let payload = "a".repeat(10_000);
        let chunks = chunk_payload(&payload);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].starts_with("\x1b_Ga=t,i=31261,f=100,q=2,m=1;"));
        assert!(chunks[1].starts_with("\x1b_Gm=1;"));
        assert!(chunks[2].starts_with("\x1b_Gm=0;"));
        for c in &chunks {
            assert!(c.ends_with("\x1b\\"));
            // payload part after ';' and before terminator is <= 4096
            let body = c.rsplit(';').next().unwrap().trim_end_matches("\x1b\\");
            assert!(body.len() <= CHUNK_SIZE);
        }
    }

    #[test]
    fn placement_sequence() {
        let s = place_sequence(Rect {
            x: 2,
            y: 5,
            width: 40,
            height: 10,
        });
        assert!(s.contains("\x1b[6;3H"));
        assert!(s.contains("a=p,i=31261,p=1,z=-1073741825,c=40,r=10,q=2"));
    }

    #[test]
    fn cover_crop_math() {
        assert_eq!(cover_crop_size(1000, 500, 200, 200), (400, 200, 100, 0));
    }

    #[test]
    fn transmit_produces_darkened_cover_sized_png() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 0, 0, 255])));
        let seq = transmit_sequence(&img, 32, 16, BgLayout::Cover).unwrap();
        // Reassemble the base64 payload from the chunks.
        let b64: String = seq
            .split("\x1b\\")
            .filter_map(|c| c.rsplit(';').next())
            .collect();
        let png = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!(decoded.dimensions(), (32, 16));
        let p = decoded.get_pixel(0, 0);
        assert!((p[0] as i32 - 191).abs() <= 2, "red channel {}", p[0]);
        assert!(p[1] <= 2 && p[2] <= 2);
    }

    #[test]
    fn fit_height_centers_sharp_image() {
        // Uniform green 100x100 source on a 300x100 target.
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            100,
            100,
            image::Rgba([0, 255, 0, 255]),
        ));
        let seq = transmit_sequence(&img, 300, 100, BgLayout::FitHeight).unwrap();
        let b64: String = seq
            .split("\x1b\\")
            .filter_map(|c| c.rsplit(';').next())
            .collect();
        let png = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (300, 100));
        // Center: sharp foreground at DARKEN.
        let c = decoded.get_pixel(150, 50);
        assert!((c[1] as i32 - 191).abs() <= 2, "fg green {}", c[1]);
        // Edge: blurred backdrop at 0.45.
        let e = decoded.get_pixel(10, 50);
        assert!((e[1] as i32 - 114).abs() <= 3, "bg green {}", e[1]);
    }

    /// Live wallhaven.cc API check (network). Run once with:
    ///   cargo test wallhaven -- --ignored
    #[tokio::test]
    #[ignore]
    async fn wallhaven_api_returns_image_url() {
        let client = reqwest::Client::new();
        let url = reqwest::Url::parse_with_params(
            "https://wallhaven.cc/api/v1/search",
            &[
                ("q", "abstract"),
                ("categories", "100"),
                ("purity", "100"),
                ("sorting", "random"),
                ("atleast", "1280x720"),
            ],
        )
        .unwrap();
        let json: serde_json::Value = client
            .get(url)
            .header("User-Agent", "tuidal")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let path = json["data"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|d| d["path"].as_str())
            .expect("no image path in wallhaven response");
        let bytes = client
            .get(path)
            .header("User-Agent", "tuidal")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert!(img.width() > 1280, "width {}", img.width());
    }
}

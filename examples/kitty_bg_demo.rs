//! Standalone demo: a kitty-protocol background image behind a ratatui Chart.
//!
//! Reproduces exactly what the app does in the Visualizer tab: draw the UI
//! with ratatui, then write kitty APC sequences on stdout after the draw.
//! Run inside kitty: `cargo run --example kitty_bg_demo` (q to quit).

// The demo only exercises part of the module; the rest is used by the app.
#[allow(dead_code)]
#[path = "../src/kitty_bg.rs"]
mod kitty_bg;

use std::io::{self, Write as _};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use image::{DynamicImage, RgbaImage};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph},
};

/// Square source (like an album cover): gradient + yellow disc, so the demo
/// shows the FitHeight layout — sharp center, blurred side bands.
fn demo_image(w: u32, h: u32) -> DynamicImage {
    let mut img = kitty_bg::default_background()
        .resize_exact(w, h, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let (cx, cy, r) = (w as f32 / 2.0, h as f32 / 2.0, h as f32 / 4.0);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        if (dx * dx + dy * dy).sqrt() < r {
            *px = image::Rgba([240, 210, 60, 255]);
        }
    }
    DynamicImage::ImageRgba8(RgbaImage::from(img))
}

fn draw_ui(f: &mut Frame, info: &[String]) -> Rect {
    let area = f.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" kitty_bg demo ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(info.len() as u16 + 1), Constraint::Min(0)])
        .split(inner);

    let lines: Vec<Line> = info
        .iter()
        .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(Color::White))))
        .collect();
    f.render_widget(Paragraph::new(lines), chunks[0]);

    // Fixed sine curve, no background style: the image shows through.
    let data: Vec<(f64, f64)> = (0..=200)
        .map(|i| {
            let x = i as f64 / 200.0 * 4.0 * std::f64::consts::PI;
            (x, (x.sin() + 1.0) / 2.0)
        })
        .collect();
    let dataset = Dataset::default()
        .graph_type(GraphType::Line)
        .marker(Marker::Braille)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .data(&data);
    let chart = Chart::new(vec![dataset])
        .x_axis(Axis::default().bounds([0.0, 4.0 * std::f64::consts::PI]))
        .y_axis(Axis::default().bounds([0.0, 1.0]));
    f.render_widget(chart, chunks[1]);

    chunks[1]
}

fn main() -> io::Result<()> {
    let picker = ratatui_image::picker::Picker::from_query_stdio().ok();
    let (fw, fh) = picker.as_ref().map(|p| p.font_size()).unwrap_or((10, 20));
    let proto = picker
        .as_ref()
        .map(|p| format!("{:?}", p.protocol_type()))
        .unwrap_or_else(|| "none".to_string());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let img = demo_image(600, 600);
    let mut transmit: Option<String> = None;
    let mut frame = 0u32;

    loop {
        let mut inner = Rect::ZERO;
        terminal.draw(|f| {
            let seq_len = transmit.as_ref().map(|t| t.len()).unwrap_or(0);
            let info = vec![
                format!("protocol: {proto}   font: {fw}x{fh}px"),
                format!("image: 600x600 (FitHeight)   transmit seq: {seq_len} bytes"),
                "press q to quit".to_string(),
            ];
            inner = draw_ui(f, &info);
        })?;

        // After the ratatui draw/flush: transmit once, then re-place every
        // ~10 frames (ratatui clears the screen on resize).
        if transmit.is_none() {
            let px_w = inner.width as u32 * fw as u32;
            let px_h = inner.height as u32 * fh as u32;
            let seq = kitty_bg::transmit_sequence(&img, px_w, px_h, kitty_bg::BgLayout::FitHeight)
                .unwrap_or_default()
                + &kitty_bg::place_sequence(inner);
            io::stdout().write_all(seq.as_bytes())?;
            io::stdout().flush()?;
            transmit = Some(seq);
        } else if frame % 10 == 0 {
            io::stdout().write_all(kitty_bg::place_sequence(inner).as_bytes())?;
            io::stdout().flush()?;
        }
        frame = frame.wrapping_add(1);

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    io::stdout().write_all(kitty_bg::delete_sequence().as_bytes())?;
    io::stdout().flush()?;
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

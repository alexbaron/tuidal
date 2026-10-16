//use crate::app::{App, InputMode, Tab};
use crate::app::{App, CollectionView, InputMode, Tab};
use crate::player::PlayerState;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    symbols::Marker,
    widgets::{
        Axis, Bar, BarChart, BarGroup, Block, BorderType, Borders, Cell, Chart, Clear, Dataset,
        GraphType, Padding, Paragraph, Row, Table, Wrap,
    },
};
//use ratatui_image::{picker::Picker, StatefulImage, Resize};
use ratatui_image::{Resize, StatefulImage};

const BG: Color = Color::Rgb(10, 10, 14);
const BG2: Color = Color::Rgb(18, 18, 26);
const BG3: Color = Color::Rgb(28, 28, 40);
const ACCENT: Color = Color::Rgb(99, 202, 183);
const ACCENT2: Color = Color::Rgb(180, 110, 255);
const GOLD: Color = Color::Rgb(255, 200, 80);
const TEXT: Color = Color::Rgb(210, 208, 220);
const MUTED: Color = Color::Rgb(110, 108, 130);
const DIM: Color = Color::Rgb(60, 58, 80);
const RED: Color = Color::Rgb(255, 85, 100);

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_player(f, app, chunks[2]);
    draw_status(f, app, chunks[3]);

    if app.device_code.is_some() {
        draw_login_overlay(f, app, area);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Min(0),
            Constraint::Length(30),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "◈ ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "TIDAL",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" TUI", Style::default().fg(MUTED)),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::RIGHT)
                .border_style(Style::default().fg(DIM))
                .padding(Padding::horizontal(1)),
        ),
        cols[0],
    );

    let s = app.lang.strings();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            tab_span(s.tab_search, Tab::Search, &app.active_tab),
            Span::styled("  ", Style::default()),
            tab_span(s.tab_queue, Tab::Queue, &app.active_tab),
            Span::styled("  ", Style::default()),
            tab_span(s.tab_now, Tab::Now, &app.active_tab),
            Span::styled("  ", Style::default()),
            tab_span(s.tab_visualizer, Tab::Visualizer, &app.active_tab),
            Span::styled("  ", Style::default()),
            tab_span(s.tab_effects, Tab::Effects, &app.active_tab),
            Span::styled("  ", Style::default()),
            tab_span(s.tab_library, Tab::Library, &app.active_tab),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(DIM))
                .padding(Padding::new(1, 0, 1, 0)),
        ),
        cols[1],
    );

    let auth_icon = if app.authenticated {
        Span::styled("● ", Style::default().fg(ACCENT))
    } else {
        Span::styled("○ ", Style::default().fg(RED))
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("[{}]  ", app.lang.label()),
                Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
            ),
            auth_icon,
            Span::styled(
                app.quality.label(),
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::LEFT)
                .border_style(Style::default().fg(DIM))
                .padding(Padding::horizontal(1)),
        ),
        cols[2],
    );
}

fn tab_span<'a>(label: &'a str, tab: Tab, active: &Tab) -> Span<'a> {
    if &tab == active {
        Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(BG)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" {label} "), Style::default().fg(MUTED))
    }
}

fn draw_body(f: &mut Frame, app: &mut App, area: Rect) {
    // Only the Visualizer tab fills this in; reset it for other tabs.
    app.spectrum_area = None;
    match app.active_tab {
        Tab::Search => draw_search_tab(f, app, area),
        Tab::Queue => draw_queue_tab(f, app, area),
        Tab::Now => draw_now_tab(f, app, area),
        Tab::Visualizer => draw_visualizer_tab(f, app, area),
        Tab::Effects => draw_effects_tab(f, app, area),
        Tab::Library => draw_library_tab(f, app, area),
    }
}

/// Fuzzy-filter bar: `f ` prefix, query text and a right-aligned
/// "shown/total" counter. ACCENT while typing, muted when a filter stays
/// applied in Normal mode.
fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect, total: usize) {
    let active = app.input_mode == InputMode::Filter;
    let shown = app.filter_matches.as_ref().map_or(total, |m| m.len());
    let counter = format!("{shown}/{total}");
    // Inner width: 2 borders + 2 horizontal padding.
    let inner_w = area.width.saturating_sub(4) as usize;
    let left_len = 2
        + app.filter_input.chars().count()
        + usize::from(active)
        + if app.filter_input.is_empty() {
            app.lang.strings().filter_placeholder.chars().count()
        } else {
            0
        };
    let pad = inner_w.saturating_sub(left_len + counter.chars().count());
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "f ",
                Style::default()
                    .fg(if active { ACCENT } else { MUTED })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(app.filter_input.as_str(), Style::default().fg(TEXT)),
            if active {
                Span::styled(
                    "▎",
                    Style::default()
                        .fg(ACCENT)
                        .add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
            if app.filter_input.is_empty() {
                Span::styled(
                    app.lang.strings().filter_placeholder,
                    Style::default().fg(DIM),
                )
            } else {
                Span::raw("")
            },
            Span::raw(" ".repeat(pad)),
            Span::styled(counter, Style::default().fg(MUTED)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if active { ACCENT } else { DIM }))
                .padding(Padding::horizontal(1)),
        ),
        area,
    );
}

/// Build the filtered view of `tracks` (visible order + per-item match
/// indices) when a filter is applied.
fn filtered_tracks(
    app: &App,
    tracks: &[crate::tidal::Track],
) -> (Vec<crate::tidal::Track>, Option<Vec<Vec<u32>>>) {
    match &app.filter_matches {
        Some(m) => (
            m.iter()
                .filter_map(|x| tracks.get(x.index).cloned())
                .collect(),
            Some(m.iter().map(|x| x.indices.clone()).collect()),
        ),
        None => (tracks.to_vec(), None),
    }
}

fn draw_search_tab(f: &mut Frame, app: &App, area: Rect) {
    let show_filter = app.input_mode == InputMode::Filter || app.filter_matches.is_some();
    let constraints: Vec<Constraint> = if show_filter {
        vec![
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ]
    } else {
        vec![Constraint::Length(3), Constraint::Min(0)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    let list_area = *chunks.last().unwrap();
    if show_filter {
        draw_filter_bar(f, app, chunks[1], app.search_results.len());
    }

    let is_searching = app.input_mode == InputMode::Search;
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "/ ",
                Style::default()
                    .fg(if is_searching { ACCENT } else { MUTED })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(app.search_input.as_str(), Style::default().fg(TEXT)),
            if is_searching {
                Span::styled(
                    "▎",
                    Style::default()
                        .fg(ACCENT)
                        .add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
            if is_searching && app.live_search_in_flight {
                Span::styled(" ⟳", Style::default().fg(MUTED))
            } else {
                Span::raw("")
            },
            if app.search_input.is_empty() && !is_searching {
                Span::styled(
                    app.lang.strings().search_placeholder,
                    Style::default().fg(DIM),
                )
            } else {
                Span::raw("")
            },
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if is_searching { ACCENT } else { DIM }))
                .padding(Padding::horizontal(1)),
        ),
        chunks[0],
    );

    let (tracks, highlights) = filtered_tracks(app, &app.search_results);
    draw_track_list(
        f,
        app,
        list_area,
        &tracks,
        app.selected,
        app.lang.strings().search_results_title,
        highlights.as_deref(),
    );
}

fn draw_queue_tab(f: &mut Frame, app: &App, area: Rect) {
    let show_filter = app.input_mode == InputMode::Filter || app.filter_matches.is_some();
    let list_area = if show_filter {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);
        draw_filter_bar(f, app, chunks[0], app.queue.len());
        chunks[1]
    } else {
        area
    };
    let (tracks, highlights) = filtered_tracks(app, &app.queue);
    draw_track_list(
        f,
        app,
        list_area,
        &tracks,
        app.selected,
        app.lang.strings().queue_title,
        highlights.as_deref(),
    );
}

fn draw_now_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let s = app.lang.strings();
    if app.player.current.is_none() {
        f.render_widget(
            Paragraph::new(format!("\n\n  {}", s.now_playing_empty))
                .style(Style::default().fg(MUTED))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(DIM))
                        .title(Span::styled(
                            format!(" {} ", s.now_playing_title),
                            Style::default().fg(MUTED),
                        )),
                ),
            area,
        );
        return;
    }

    let img_cols = area.height.saturating_sub(2); // environ carré en cellules
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(img_cols), // carré
            Constraint::Min(0),           // reste pour info + paroles
        ])
        .split(area);

    // ── Colonne gauche : pochette ─────────────────────────────────────────
    let img_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .style(Style::default().bg(BG2));
    let img_inner = img_block.inner(cols[0]);
    f.render_widget(img_block, cols[0]);

    let area_size = (img_inner.width, img_inner.height);
    if app.cover_proto.is_none() || app.last_img_area != Some(area_size) {
        app.last_img_area = Some(area_size);
        app.cover_proto = None;
        if let (Some(picker), Some(img)) = (&app.picker, &app.cover_image) {
            let (cols, rows) = (img_inner.width as u32, img_inner.height as u32);
            let font_size = picker.font_size();
            let target_w = cols * font_size.0 as u32;
            let target_h = rows * font_size.1 as u32;
            let scaled = img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3);
            app.cover_proto = Some(picker.new_resize_protocol(scaled));
        }
    }

    if let Some(ref mut proto) = app.cover_proto {
        let widget = StatefulImage::new().resize(Resize::Fit(None));
        f.render_stateful_widget(widget, img_inner, proto);
    } else {
        f.render_widget(
            Paragraph::new(format!("\n\n  {}", app.lang.strings().loading_image))
                .alignment(Alignment::Center)
                .style(Style::default().fg(MUTED)),
            img_inner,
        );
    }

    // ── Colonne droite : info (haut) + paroles (bas) ────────────────────
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(cols[1]);

    // Info
    let info_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" {} ", s.now_playing_title),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(BG2));
    let info_inner = info_block.inner(right_chunks[0]);
    f.render_widget(info_block, right_chunks[0]);

    if let Some(ref track) = app.player.current {
        let max_w = (info_inner.width as usize).saturating_sub(4);
        let progress = app.player.progress();
        let bar_width = info_inner.width.saturating_sub(14) as usize;
        let filled = (progress * bar_width as f64) as usize;
        let rest = bar_width.saturating_sub(filled);
        let duration_str = format!("{}:{:02}", track.duration / 60, track.duration % 60);

        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {}", truncate(&track.title, max_w)),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {}", truncate(&track.artist, max_w)),
                    Style::default().fg(ACCENT),
                )),
                Line::from(Span::styled(
                    format!("  {}", truncate(&track.album, max_w)),
                    Style::default().fg(MUTED),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    format!(
                        "  {}bit / {}kHz  {}",
                        track.bit_depth,
                        track.sample_rate / 1000,
                        track.codec.to_uppercase()
                    ),
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        format!("  {} ", app.player.elapsed_str()),
                        Style::default().fg(MUTED),
                    ),
                    Span::styled("━".repeat(filled), Style::default().fg(ACCENT)),
                    Span::styled("╌".repeat(rest), Style::default().fg(DIM)),
                    Span::styled(format!(" {duration_str}"), Style::default().fg(MUTED)),
                ]),
            ]),
            info_inner,
        );
    }

    // ── Paroles ──────────────────────────────────────────────────────────────
    draw_lyrics(f, app, right_chunks[1]);
}

fn draw_lyrics(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 2 {
        return;
    }
    let Some(ref lyrics) = app.lyrics else {
        if app.player.current.is_some() {
            let s = app.lang.strings();
            f.render_widget(
                Paragraph::new(s.loading)
                    .style(Style::default().fg(DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(DIM))
                            .title(Span::styled(
                                format!(" {} ", s.now_lyrics),
                                Style::default().fg(MUTED),
                            )),
                    ),
                area,
            );
        }
        return;
    };

    let plain_empty = lyrics.plain.trim().is_empty();
    if plain_empty {
        let s = app.lang.strings();
        f.render_widget(
            Paragraph::new(format!("\n  {}", s.now_no_lyrics))
                .style(Style::default().fg(DIM))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(DIM))
                        .title(Span::styled(
                            format!(" {} ", s.now_lyrics),
                            Style::default().fg(MUTED),
                        )),
                ),
            area,
        );
        return;
    }

    let lyrics_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" {} ", app.lang.strings().now_lyrics),
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(BG2));
    let inner = lyrics_block.inner(area);
    f.render_widget(lyrics_block, area);

    if inner.height < 2 {
        return;
    }

    let elapsed = app.player.elapsed.as_secs();
    let max_lines = inner.height as usize;

    let lyric_lines: Vec<Line> = if lyrics.has_sync && !lyrics.lines.is_empty() {
        let current = lyrics.current_line(elapsed);
        let half = max_lines / 2;
        let start = if current > half { current - half } else { 0 };
        let end = (start + max_lines).min(lyrics.lines.len());

        lyrics.lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, (_ts, text))| {
                let idx = start + i;
                if idx == current {
                    Line::from(Span::styled(
                        text.as_str(),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Line::from(Span::styled(text.as_str(), Style::default().fg(TEXT)))
                }
            })
            .collect()
    } else {
        lyrics
            .plain
            .lines()
            .take(max_lines)
            .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(TEXT))))
            .collect()
    };

    f.render_widget(
        Paragraph::new(lyric_lines).style(Style::default().bg(BG2)),
        inner,
    );
}

/// Split `text` into spans, highlighting the char positions listed in
/// `indices` (global haystack offsets — `base` is where `text` starts
/// inside that haystack). Hits render GOLD + BOLD.
fn highlight_spans(
    text: &str,
    indices: &[u32],
    base: usize,
    normal: Style,
    hit: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut run_hit = false;
    for (i, ch) in text.chars().enumerate() {
        let is_hit = indices.contains(&((base + i) as u32));
        if is_hit != run_hit && !run.is_empty() {
            spans.push(Span::styled(
                std::mem::take(&mut run),
                if run_hit { hit } else { normal },
            ));
        }
        run_hit = is_hit;
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, if run_hit { hit } else { normal }));
    }
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}

/// Style for fuzzy-matched characters.
fn hit_style() -> Style {
    Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
}

fn draw_track_list(
    f: &mut Frame,
    app: &App,
    area: Rect,
    tracks: &[crate::tidal::Track],
    selected: usize,
    title: &str,
    highlights: Option<&[Vec<u32>]>,
) {
    let s = app.lang.strings();
    if tracks.is_empty() {
        let msg = if app.loading {
            s.loading
        } else if !app.authenticated {
            s.not_authenticated
        } else {
            s.no_results_hint
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(MUTED)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM))
                    .title(Span::styled(
                        format!(" {title} "),
                        Style::default().fg(MUTED),
                    ))
                    .padding(Padding::new(1, 1, 1, 0)),
            ),
            area,
        );
        return;
    }

    let now_playing_id = app.queue_index.and_then(|i| app.queue.get(i)).map(|t| t.id);

    let rows: Vec<Row> = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_selected = i == selected;
            let is_now_playing = Some(t.id) == now_playing_id;
            let row_bg = if is_selected {
                BG3
            } else if i % 2 == 0 {
                BG
            } else {
                BG2
            };

            let title_style = Style::default()
                .fg(if is_now_playing { ACCENT } else { TEXT })
                .add_modifier(if is_now_playing {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            let hl = highlights.and_then(|h| h.get(i));
            // Haystack is "{title} {artists} {album}": map global match
            // offsets back to each cell.
            let title_len = t.title.chars().count();
            let artist = t.artist_names();
            let artist_len = artist.chars().count();
            let title_spans = match hl {
                Some(idx) => highlight_spans(
                    &truncate(&t.title, 36),
                    idx,
                    0,
                    title_style,
                    hit_style(),
                ),
                None => vec![Span::styled(truncate(&t.title, 36), title_style)],
            };
            let artist_spans = match hl {
                Some(idx) => highlight_spans(
                    &truncate(&artist, 24),
                    idx,
                    title_len + 1,
                    Style::default().fg(MUTED),
                    hit_style(),
                ),
                None => vec![Span::styled(
                    truncate(&artist, 24),
                    Style::default().fg(MUTED),
                )],
            };
            let album_spans = match hl {
                Some(idx) => highlight_spans(
                    &truncate(&t.album.title, 22),
                    idx,
                    title_len + 1 + artist_len + 1,
                    Style::default().fg(DIM),
                    hit_style(),
                ),
                None => vec![Span::styled(
                    truncate(&t.album.title, 22),
                    Style::default().fg(DIM),
                )],
            };

            let mut title_line = vec![
                if is_now_playing {
                    Span::styled(
                        "▶ ",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled("  ", Style::default())
                },
                Span::styled(
                    format!("{} ", t.quality_icon()),
                    Style::default().fg(match t.audio_quality.as_deref() {
                        Some("HI_RES_LOSSLESS") => GOLD,
                        Some("LOSSLESS") => ACCENT,
                        _ => MUTED,
                    }),
                ),
            ];
            title_line.extend(title_spans);

            Row::new(vec![
                Cell::from(Span::styled(
                    format!("{:>3}", i + 1),
                    Style::default().fg(MUTED),
                )),
                Cell::from(Line::from(title_line)),
                Cell::from(Line::from(artist_spans)),
                Cell::from(Line::from(album_spans)),
                Cell::from(Span::styled(t.duration_str(), Style::default().fg(MUTED))),
            ])
            .style(Style::default().bg(row_bg))
            .height(1)
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled(
            "  #",
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_title,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_artist,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_album,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_duration,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
    ])
    .style(Style::default().bg(BG2))
    .height(1);

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(selected));
    f.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Min(30),
                Constraint::Length(24),
                Constraint::Length(22),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(
                    format!(" {title} ({}) ", tracks.len()),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ))
                .padding(Padding::ZERO),
        )
        .row_highlight_style(Style::default().bg(BG3))
        .highlight_symbol("  "),
        area,
        &mut state,
    );
}

fn draw_player(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(BG2)),
        area,
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let (state_icon, state_color) = match app.player.state {
        PlayerState::Playing => ("▶", ACCENT),
        PlayerState::Paused => ("⏸", GOLD),
        PlayerState::Stopped => ("■", MUTED),
    };

    f.render_widget(
        Paragraph::new(if let Some(info) = &app.player.current {
            Line::from(vec![
                Span::styled(
                    format!(" {state_icon} "),
                    Style::default()
                        .fg(state_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    truncate(&info.title, 30),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  —  ", Style::default().fg(DIM)),
                Span::styled(truncate(&info.artist, 24), Style::default().fg(MUTED)),
                Span::styled("   ", Style::default()),
                Span::styled(
                    format!(
                        "{}bit/{}kHz {}",
                        info.bit_depth,
                        info.sample_rate / 1000,
                        info.codec.to_uppercase()
                    ),
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "   {}  vol {}%",
                        app.player.elapsed_str(),
                        app.player.volume
                    ),
                    Style::default().fg(MUTED),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!(" {state_icon} "), Style::default().fg(MUTED)),
                Span::styled(app.lang.strings().player_stopped, Style::default().fg(DIM)),
            ])
        }),
        inner[0],
    );

    let progress = app.player.progress();
    let duration_str = app
        .player
        .current
        .as_ref()
        .map(|t| format!("{}:{:02}", t.duration / 60, t.duration % 60))
        .unwrap_or_else(|| "0:00".to_string());
    let bar_width = inner[1].width.saturating_sub(12) as usize;
    let filled = (progress * bar_width as f64) as usize;
    let rest = bar_width.saturating_sub(filled);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{}  ", app.player.elapsed_str()),
                Style::default().fg(MUTED),
            ),
            Span::styled("━".repeat(filled), Style::default().fg(ACCENT)),
            Span::styled("╌".repeat(rest), Style::default().fg(DIM)),
            Span::styled(format!("  {duration_str}"), Style::default().fg(MUTED)),
        ])),
        inner[1],
    );

    let s = app.lang.strings();
    if app.active_tab == Tab::Visualizer {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {}", s.visualizer_hints),
                Style::default().fg(DIM),
            ))),
            inner[2],
        );
        return;
    }
    if app.active_tab == Tab::Effects {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {}", s.fx_hints),
                Style::default().fg(DIM),
            ))),
            inner[2],
        );
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(vec![
            hint_key("Enter", s.hint_play),
            hint_key("a", s.hint_add_queue),
            hint_key("Space", s.hint_pause),
            hint_key("n/p", s.hint_next_prev),
            hint_key("←/→", s.hint_seek),
            hint_key("+/-", s.hint_volume),
            hint_key("Tab", s.hint_view),
            hint_key("1/2/3", s.hint_quality),
            hint_key("q", s.hint_quit),
            hint_key("i", s.hint_library),
            hint_key("g", s.hint_theme),
            hint_key("f", s.hint_filter),
            hint_key("F", s.hint_fav_tracks),
            hint_key("A", s.hint_fav_albums),
            hint_key("Alt+L", s.hint_lang),
        ])),
        inner[2],
    );
}

fn hint_key<'a>(key: &'a str, desc: &'a str) -> Span<'a> {
    Span::styled(format!("  [{key}]{desc}"), Style::default().fg(DIM))
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let (icon, color) = if app.loading {
        ("◌", GOLD)
    } else if app.status_msg.starts_with("✓") || app.status_msg.starts_with("▶") {
        ("●", ACCENT)
    } else if app.status_msg.starts_with("✗") {
        ("●", RED)
    } else {
        ("○", MUTED)
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {icon} "), Style::default().fg(color)),
            Span::styled(app.status_msg.as_str(), Style::default().fg(TEXT)),
        ]))
        .style(Style::default().bg(BG)),
        area,
    );
}

fn draw_login_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(60, 12, area);
    f.render_widget(Clear, popup);

    let user_code = app.user_code.as_deref().unwrap_or("...");
    let auth_url = app.auth_url.as_deref().unwrap_or("...");
    let s = app.lang.strings();

    f.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                s.login_title_text,
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(s.login_open_url, Style::default().fg(MUTED))),
            Line::from(Span::styled(
                format!("     {auth_url}"),
                Style::default().fg(ACCENT),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(s.login_code_prefix, Style::default().fg(MUTED)),
                Span::styled(
                    user_code,
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(s.login_waiting, Style::default().fg(DIM))),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(ACCENT))
                .title(Span::styled(
                    s.login_overlay_title,
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ))
                .style(Style::default().bg(BG2)),
        )
        .wrap(Wrap { trim: false }),
        popup,
    );
}

/// Short EQ band labels: 31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k.
const EQ_LABELS: [&str; 10] = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

/// 21-cell ASCII slider for a value in [min, max], knob at position.
fn slider_str(value: i32, min: i32, max: i32) -> String {
    const W: usize = 21;
    let pos = ((value - min) as f64 / (max - min) as f64 * (W - 1) as f64).round() as usize;
    let mut s = String::with_capacity(W);
    for i in 0..W {
        s.push(if i == pos { '◆' } else { '─' });
    }
    s
}

fn draw_effects_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let s = app.lang.strings();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(area);

    // ── EQ block ──────────────────────────────────────────────────────────
    let eq_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" {} ", s.fx_eq_title),
            Style::default().fg(ACCENT),
        ))
        .style(Style::default().bg(BG2));
    let cursor_control = crate::audio_fx::FxControl::ALL[app.fx_cursor];
    let cursor_eq = match cursor_control {
        crate::audio_fx::FxControl::Eq(i) => Some(i),
        _ => None,
    };
    let bars: Vec<Bar> = app
        .audio_fx
        .eq
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let selected = cursor_eq == Some(i);
            Bar::default()
                .value((i32::from(*g) + 12) as u64)
                .text_value(format!("{g:+}"))
                .label(EQ_LABELS[i].into())
                .style(Style::default().fg(if selected { GOLD } else { ACCENT }))
                .value_style(
                    Style::default()
                        .fg(if selected { GOLD } else { TEXT })
                        .add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                )
        })
        .collect();
    let chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .max(24)
        .bar_width(5)
        .bar_gap(2);
    f.render_widget(chart.block(eq_block), chunks[0]);

    // ── Controls block ────────────────────────────────────────────────────
    let title = match app.audio_fx.matching_preset() {
        Some(i) => format!(
            " {} — {} ",
            s.fx_title,
            crate::audio_fx::PRESETS[i].0
        ),
        None => format!(" {} ", s.fx_title),
    };
    let ctrl_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(title, Style::default().fg(ACCENT)))
        .style(Style::default().bg(BG2));
    let ctrl_inner = ctrl_block.inner(chunks[1]);
    f.render_widget(ctrl_block, chunks[1]);

    use crate::audio_fx::FxControl as C;
    let no_mpv = !app.player.mpv_available || app.player.backend == crate::player::Backend::Ffplay;
    let (warn_area, table_area) = if no_mpv {
        let sp = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(ctrl_inner);
        (Some(sp[0]), sp[1])
    } else {
        (None, ctrl_inner)
    };
    if let Some(w) = warn_area {
        f.render_widget(
            Paragraph::new(Span::styled(s.fx_no_mpv, Style::default().fg(RED))),
            w,
        );
    }

    let bool_box = |on: bool| if on { "[x]" } else { "[ ]" }.to_string();
    let on_off = |on: bool| if on { s.fx_on } else { s.fx_off };

    let rows: Vec<Row> = C::ALL
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let fx = &app.audio_fx;
            let (name, slider, value): (String, String, String) = match *c {
                C::Enabled => (
                    s.fx_enabled.to_string(),
                    bool_box(fx.enabled),
                    on_off(fx.enabled).to_string(),
                ),
                C::Preset => {
                    let name = fx
                        .matching_preset()
                        .map(|p| crate::audio_fx::PRESETS[p].0)
                        .unwrap_or("Custom");
                    (s.fx_preset.to_string(), format!("‹ {name} ›"), String::new())
                }
                C::Balance => (
                    s.fx_balance.to_string(),
                    slider_str(fx.balance as i32, -100, 100),
                    match fx.balance.cmp(&0) {
                        std::cmp::Ordering::Less => format!("L {}", -fx.balance),
                        std::cmp::Ordering::Greater => format!("R {}", fx.balance),
                        std::cmp::Ordering::Equal => s.fx_center.to_string(),
                    },
                ),
                C::Bass => (
                    s.fx_bass.to_string(),
                    slider_str(fx.bass as i32, -15, 15),
                    format!("{:+} dB", fx.bass),
                ),
                C::Treble => (
                    s.fx_treble.to_string(),
                    slider_str(fx.treble as i32, -15, 15),
                    format!("{:+} dB", fx.treble),
                ),
                C::Eq(b) => (
                    format!("  {}", EQ_LABELS[b]),
                    slider_str(fx.eq[b] as i32, -12, 12),
                    format!("{:+} dB", fx.eq[b]),
                ),
                C::StereoWidth => (
                    s.fx_stereo_width.to_string(),
                    slider_str(fx.stereo_width as i32, 0, 200),
                    format!("{} %", fx.stereo_width),
                ),
                C::Crossfeed => (
                    s.fx_crossfeed.to_string(),
                    bool_box(fx.crossfeed),
                    on_off(fx.crossfeed).to_string(),
                ),
                C::Echo => (
                    s.fx_echo.to_string(),
                    bool_box(fx.echo),
                    on_off(fx.echo).to_string(),
                ),
                C::Compressor => (
                    s.fx_compressor.to_string(),
                    bool_box(fx.compressor),
                    on_off(fx.compressor).to_string(),
                ),
                C::Normalize => (
                    s.fx_normalize.to_string(),
                    bool_box(fx.normalize),
                    on_off(fx.normalize).to_string(),
                ),
            };
            let sel = i == app.fx_cursor;
            Row::new(vec![
                Cell::from(Span::styled(
                    name,
                    Style::default()
                        .fg(if sel { ACCENT } else { TEXT })
                        .add_modifier(if sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                )),
                Cell::from(Span::styled(slider, Style::default().fg(if sel { ACCENT } else { DIM }))),
                Cell::from(Span::styled(value, Style::default().fg(MUTED))),
            ])
            .style(Style::default().bg(if sel {
                BG3
            } else if i % 2 == 0 {
                BG
            } else {
                BG2
            }))
        })
        .collect();

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.fx_cursor));
    f.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(26),
                Constraint::Length(23),
                Constraint::Min(8),
            ],
        )
        .row_highlight_style(Style::default().bg(BG3)),
        table_area,
        &mut state,
    );
}

/// Theme-search prompt shown at the top of the Library tab while typing a
/// theme (`InputMode::Theme`) and kept afterwards — muted — while the list
/// displays results for `app.theme_query`.
fn draw_theme_prompt(f: &mut Frame, app: &App, area: Rect) {
    let active = app.input_mode == InputMode::Theme;
    let text = if active {
        app.search_input.as_str()
    } else {
        app.theme_query.as_deref().unwrap_or("")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "g ",
                Style::default()
                    .fg(if active { ACCENT } else { MUTED })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(text, Style::default().fg(TEXT)),
            if active {
                Span::styled(
                    "▎",
                    Style::default()
                        .fg(ACCENT)
                        .add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
            if active && app.live_search_in_flight {
                Span::styled(" ⟳", Style::default().fg(MUTED))
            } else {
                Span::raw("")
            },
            if text.is_empty() {
                Span::styled(
                    app.lang.strings().theme_placeholder,
                    Style::default().fg(DIM),
                )
            } else {
                Span::raw("")
            },
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if active { ACCENT } else { DIM }))
                .padding(Padding::horizontal(1)),
        ),
        area,
    );
}

fn draw_library_tab(f: &mut Frame, app: &App, area: Rect) {
    // Split off the theme prompt when typing a theme or when the list below
    // shows the results of a previous theme search, plus the fuzzy-filter
    // bar while a filter is active or applied.
    let show_prompt = app.input_mode == InputMode::Theme || app.theme_query.is_some();
    let show_filter = app.input_mode == InputMode::Filter || app.filter_matches.is_some();
    let total_items = if app.collection_view == CollectionView::Albums {
        app.fav_albums.len()
    } else {
        app.playlists.len() + app.mixes.len()
    };
    let bars = usize::from(show_prompt) + usize::from(show_filter);
    let area = if bars > 0 {
        let mut constraints = vec![Constraint::Length(3); bars];
        constraints.push(Constraint::Min(0));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);
        let mut i = 0;
        if show_prompt {
            draw_theme_prompt(f, app, chunks[i]);
            i += 1;
        }
        if show_filter {
            draw_filter_bar(f, app, chunks[i], total_items);
        }
        *chunks.last().unwrap()
    } else {
        area
    };

    // Si on est en vue albums favoris
    //if app.collection_view == app::CollectionView::Albums {
    if app.collection_view == CollectionView::Albums {
        draw_fav_albums(f, app, area);
        return;
    }
    let s = app.lang.strings();
    let total = app.playlists.len() + app.mixes.len();
    if total == 0 {
        f.render_widget(
            Paragraph::new(if app.loading {
                s.library_loading
            } else {
                s.library_hint
            })
            .style(Style::default().fg(MUTED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM))
                    .title(Span::styled(
                        format!(" {} ", s.library_title),
                        Style::default().fg(MUTED),
                    )),
            ),
            area,
        );
        return;
    }

    // Visible rows: filtered order when a filter is applied, else 0..total.
    let visible: Vec<(usize, Option<&Vec<u32>>)> = match &app.filter_matches {
        Some(m) => m.iter().map(|x| (x.index, Some(&x.indices))).collect(),
        None => (0..app.playlists.len() + app.mixes.len())
            .map(|i| (i, None))
            .collect(),
    };

    let items: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, &(idx, hl))| {
            let is_sel = i == app.library_selected;
            let row_bg = if is_sel {
                BG3
            } else if i % 2 == 0 {
                BG
            } else {
                BG2
            };
            if idx < app.playlists.len() {
                let p = &app.playlists[idx];
                let title_style = Style::default()
                    .fg(if is_sel { ACCENT } else { TEXT })
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
                // Haystack for playlists is the bare title.
                let title_cell = match hl {
                    Some(ix) => Cell::from(Line::from(highlight_spans(
                        &truncate(&p.title, 40),
                        ix,
                        0,
                        title_style,
                        hit_style(),
                    ))),
                    None => Cell::from(Span::styled(truncate(&p.title, 40), title_style)),
                };
                Row::new(vec![
                    Cell::from(Span::styled("≡ ", Style::default().fg(ACCENT2))),
                    title_cell,
                    Cell::from(Span::styled(
                        app.lang.tracks_count(p.number_of_tracks),
                        Style::default().fg(MUTED),
                    )),
                    Cell::from(Span::styled("Playlist", Style::default().fg(DIM))),
                ])
                .style(Style::default().bg(row_bg))
            } else {
                let m = &app.mixes[idx - app.playlists.len()];
                let is_sel_fg = if is_sel { GOLD } else { TEXT };
                let title_style = Style::default()
                    .fg(is_sel_fg)
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
                // Haystack for mixes is "{title} {sub_title}".
                let title_cell = match hl {
                    Some(ix) => Cell::from(Line::from(highlight_spans(
                        &truncate(&m.title, 40),
                        ix,
                        0,
                        title_style,
                        hit_style(),
                    ))),
                    None => Cell::from(Span::styled(truncate(&m.title, 40), title_style)),
                };
                Row::new(vec![
                    Cell::from(Span::styled("⊛ ", Style::default().fg(GOLD))),
                    title_cell,
                    Cell::from(Span::styled(
                        m.sub_title.as_deref().unwrap_or("").to_string(),
                        Style::default().fg(MUTED),
                    )),
                    Cell::from(Span::styled("Mix", Style::default().fg(GOLD))),
                ])
                .style(Style::default().bg(row_bg))
            }
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(Span::styled(
            s.col_name,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_info,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_type,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
    ])
    .style(Style::default().bg(BG2));

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.library_selected));

    f.render_stateful_widget(
        Table::new(
            items,
            [
                Constraint::Length(3),
                Constraint::Min(30),
                Constraint::Length(20),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(
                    match &app.theme_query {
                        Some(q) => app.lang.theme_results_title(q),
                        None => app
                            .lang
                            .library_title_with_counts(app.playlists.len(), app.mixes.len()),
                    },
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
        )
        .row_highlight_style(Style::default().bg(BG3)),
        area,
        &mut state,
    );
}

fn draw_fav_albums(f: &mut Frame, app: &App, area: Rect) {
    let s = app.lang.strings();
    if app.fav_albums.is_empty() {
        f.render_widget(
            Paragraph::new(if app.loading {
                s.loading
            } else {
                s.fav_albums_empty
            })
            .style(Style::default().fg(MUTED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM))
                    .title(Span::styled(
                        format!(" {} ", s.fav_albums_title),
                        Style::default().fg(MUTED),
                    )),
            ),
            area,
        );
        return;
    }

    // Visible rows: filtered order when a filter is applied, else 0..len.
    let visible: Vec<(usize, Option<&Vec<u32>>)> = match &app.filter_matches {
        Some(m) => m.iter().map(|x| (x.index, Some(&x.indices))).collect(),
        None => (0..app.fav_albums.len()).map(|i| (i, None)).collect(),
    };

    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, &(idx, hl))| {
            let a = &app.fav_albums[idx];
            let is_sel = i == app.fav_album_selected;
            let title_style = Style::default()
                .fg(if is_sel { ACCENT } else { TEXT })
                .add_modifier(if is_sel {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            // Haystack is "{title} {artists}".
            let title_len = a.title.chars().count();
            let artist = a.artist_names();
            let title_cell = match hl {
                Some(ix) => Cell::from(Line::from(highlight_spans(
                    &truncate(&a.title, 40),
                    ix,
                    0,
                    title_style,
                    hit_style(),
                ))),
                None => Cell::from(Span::styled(truncate(&a.title, 40), title_style)),
            };
            let artist_cell = match hl {
                Some(ix) => Cell::from(Line::from(highlight_spans(
                    &truncate(&artist, 28),
                    ix,
                    title_len + 1,
                    Style::default().fg(MUTED),
                    hit_style(),
                ))),
                None => Cell::from(Span::styled(
                    truncate(&artist, 28),
                    Style::default().fg(MUTED),
                )),
            };
            Row::new(vec![
                Cell::from(Span::styled("◆ ", Style::default().fg(ACCENT))),
                title_cell,
                artist_cell,
                Cell::from(Span::styled(
                    app.lang.tracks_count(a.number_of_tracks),
                    Style::default().fg(DIM),
                )),
            ])
            .style(Style::default().bg(if is_sel {
                BG3
            } else if i % 2 == 0 {
                BG
            } else {
                BG2
            }))
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(Span::styled(
            s.col_album,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_artist,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            s.col_tracks,
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )),
    ])
    .style(Style::default().bg(BG2));

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.fav_album_selected));

    f.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Min(30),
                Constraint::Length(28),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(
                    app.lang.fav_albums_title_with_count(app.fav_albums.len()),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
        )
        .row_highlight_style(Style::default().bg(BG3)),
        area,
        &mut state,
    );
}

/// Format a frequency as "30 Hz" / "1.0 kHz".
fn fmt_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

fn draw_visualizer_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let s = app.lang.strings();
    let bg_tag = format!(
        "[b: {}]",
        app.lang.bg_label(app.spectrum_bg)
    );
    let title = if let Some(info) = &app.player.current {
        format!(
            " {} — {} – {}  {} ",
            s.visualizer_title, info.artist, info.title, bg_tag
        )
    } else {
        format!(" {}  {} ", s.visualizer_title, bg_tag)
    };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(title, Style::default().fg(ACCENT)));
    // Keep the chart area transparent when a graphics background is shown.
    if app.spectrum_bg == crate::kitty_bg::SpectrumBg::None
        || app.bg_mode() == crate::app::BgMode::Unsupported
    {
        block = block.style(Style::default().bg(BG2));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width < 24 || inner.height < 6 {
        return;
    }

    // The band count follows the renderer and chart width; the main loop
    // reads this hint. Bars take 2 columns each (bar + gap).
    let n_bands = match app.spectrum_style {
        crate::app::SpectrumStyle::Bars => (inner.width as usize / 2).clamp(16, 256),
        crate::app::SpectrumStyle::Curve => {
            (inner.width.saturating_sub(8) as usize).clamp(16, 256)
        }
    };
    app.spectrum_bands_hint = n_bands;

    let centered_msg = |msg: &str| {
        Paragraph::new(format!("\n\n  {msg}"))
            .alignment(Alignment::Center)
            .style(Style::default().fg(MUTED))
    };

    if app.player.current.is_none() {
        f.render_widget(centered_msg(s.visualizer_empty), inner);
        return;
    }
    let failed = app
        .analysis
        .as_ref()
        .and_then(|a| a.lock().ok().map(|b| matches!(b.state, crate::spectrum::AnalysisState::Failed(_))))
        .unwrap_or(false);
    if failed {
        f.render_widget(centered_msg(s.visualizer_unavailable), inner);
        return;
    }
    if app.spectrum.bands.is_empty() {
        f.render_widget(centered_msg(s.visualizer_analyzing), inner);
        return;
    }

    // Composite mode (Sixel/iTerm2): the spectrum is drawn inside the image;
    // only a thin label row stays as text below it.
    if app.bg_mode() == crate::app::BgMode::Composite
        && app.spectrum_bg != crate::kitty_bg::SpectrumBg::None
    {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner);
        let chart_area = rows[0];
        let labels_row = rows[1];
        app.spectrum_area = Some(chart_area);

        // 5 frequency labels evenly spaced along the x axis.
        let spans = spectrum_x_labels(&app.spectrum.band_freqs, labels_row.width as usize);
        f.render_widget(Paragraph::new(Line::from(spans)), labels_row);

        match app.composite_frame() {
            Some(proto) => {
                f.render_widget(ratatui_image::Image::new(proto), chart_area);
            }
            None => render_spectrum(f, app, chart_area),
        }
        return;
    }

    // Chart is about to render: expose its area to the background placer.
    app.spectrum_area = Some(inner);
    render_spectrum(f, app, inner);
}

/// 5 frequency labels evenly spread over `width` columns (MUTED).
fn spectrum_x_labels(band_freqs: &[f32], width: usize) -> Vec<Span<'static>> {
    let n = band_freqs.len().max(1);
    let mut spans: Vec<Span> = Vec::new();
    let mut cursor = 0usize;
    for i in 0..5 {
        let idx = i * (n - 1) / 4;
        let hz = band_freqs.get(idx).copied().unwrap_or(0.0);
        let label = fmt_hz(hz);
        let label_w = label.chars().count();
        let col = match i {
            0 => 0,
            4 => width.saturating_sub(label_w),
            _ => width.saturating_sub(label_w) * i / 4,
        };
        if col >= cursor && col + label_w <= width {
            if col > cursor {
                spans.push(Span::raw(" ".repeat(col - cursor)));
            }
            spans.push(Span::styled(label, Style::default().fg(MUTED)));
            cursor = col + label_w;
        }
    }
    spans
}

/// Dispatch to the curve or bars renderer depending on the configured style.
fn render_spectrum(f: &mut Frame, app: &App, area: Rect) {
    match app.spectrum_style {
        crate::app::SpectrumStyle::Bars => render_spectrum_bars(f, app, area),
        crate::app::SpectrumStyle::Curve => render_spectrum_chart(f, app, area),
    }
}

/// Cava-style vertical bars: eighth-block fractional top cell, gradient
/// (bottom third ACCENT, middle GOLD, top RED) and a ▔ peak marker.
///
/// Cells are written straight into the frame buffer and blank cells are
/// skipped (like ratatui's Canvas does): the kitty background image sits
/// below the default cell background and only shows through cells the UI
/// never touches.
fn render_spectrum_bars(f: &mut Frame, app: &App, area: Rect) {
    if app.spectrum.bands.is_empty() || area.height < 2 {
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    // Like ratatui's Canvas: reset the cell background over the whole area so
    // the kitty image (placed below the default background) shows through
    // the base BG fill painted by `draw`.
    f.buffer_mut()
        .set_style(rows[0], Style::default().bg(ratatui::style::Color::Reset));
    paint_spectrum_bars(
        f.buffer_mut(),
        rows[0],
        &app.spectrum.bands,
        &app.spectrum.peaks,
    );
    f.render_widget(
        Paragraph::new(Line::from(spectrum_x_labels(
            &app.spectrum.band_freqs,
            rows[1].width as usize,
        ))),
        rows[1],
    );
}

/// Eighth-block glyphs indexed by 0..=8 (fraction of a cell).
const EIGHTHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Paint the bars into `buf` within `area`, touching only non-blank cells.
fn paint_spectrum_bars(buf: &mut ratatui::buffer::Buffer, area: Rect, bands: &[f32], peaks: &[f32]) {
    let n = bands.len();
    let (w, h) = (area.width as usize, area.height as usize);
    if n == 0 || w == 0 || h == 0 {
        return;
    }
    // One column of gap between bars when the width allows it.
    let col_w = if w >= n * 2 { 2 } else { 1 };
    let third = h.div_ceil(3);
    let level_color = |level: usize| {
        if level <= third {
            ACCENT
        } else if level <= 2 * third {
            GOLD
        } else {
            RED
        }
    };

    for (i, v) in bands.iter().enumerate() {
        let col = i * col_w;
        if col >= w {
            break;
        }
        let x = area.x + col as u16;
        let hv = v.clamp(0.0, 1.0) * h as f32;
        let peak_level =
            (peaks.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) * h as f32).round() as usize;
        for r in 0..h {
            // Level 1 = bottom row, level h = top row.
            let level = h - r;
            let frac = hv - (level as f32 - 1.0);
            let (ch, color) = if peak_level > 0 && level == peak_level && peak_level as f32 - hv > 0.4 {
                ('▔', MUTED)
            } else if frac >= 1.0 {
                ('█', level_color(level))
            } else if frac > 0.0 {
                (EIGHTHS[((frac * 8.0).round() as usize).clamp(1, 8)], level_color(level))
            } else {
                continue;
            };
            let y = area.y + r as u16;
            buf[(x, y)].set_char(ch).set_fg(color);
        }
    }
}

fn render_spectrum_chart(f: &mut Frame, app: &App, area: Rect) {
    let n = app.spectrum.bands.len();
    let data: Vec<(f64, f64)> = app
        .spectrum
        .bands
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64, *v as f64))
        .collect();

    // 5 frequency labels evenly spaced along the x axis.
    let mut x_labels = Vec::new();
    for i in 0..5 {
        let idx = i * (n - 1) / 4;
        let hz = app.spectrum.band_freqs.get(idx).copied().unwrap_or(0.0);
        x_labels.push(Span::styled(fmt_hz(hz), Style::default().fg(MUTED)));
    }
    let dataset = Dataset::default()
        .graph_type(GraphType::Line)
        .marker(Marker::Braille)
        .style(Style::default().fg(ACCENT))
        .data(&data);
    let chart = Chart::new(vec![dataset])
        .x_axis(
            Axis::default()
                .bounds([0.0, (n - 1).max(1) as f64])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, 1.0])
                .labels(vec![
                    Span::styled("-60 dB", Style::default().fg(MUTED)),
                    Span::styled("-30 dB", Style::default().fg(MUTED)),
                    Span::styled("  0 dB", Style::default().fg(MUTED)),
                ]),
        );
    f.render_widget(chart, area);
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            chars[..max.saturating_sub(1)].iter().collect::<String>()
        )
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = area.width * percent_x / 100;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(height)) / 2;
    Rect::new(popup_x, popup_y, popup_width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render the cava bars into a TestBackend: a 0..1 ramp must produce
    /// eighth-block glyphs and full blocks, and the label row must show
    /// "Hz"/"kHz" axis labels.
    #[test]
    fn spectrum_bars_render_glyphs_and_labels() {
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let bands: Vec<f32> = (0..40).map(|i| i as f32 / 39.0).collect();
        let peaks = vec![0.0f32; 40];
        let freqs = [40.0, 100.0, 400.0, 2000.0, 16000.0];
        term.draw(|f| {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(f.area());
            paint_spectrum_bars(f.buffer_mut(), rows[0], &bands, &peaks);
            f.render_widget(
                Paragraph::new(Line::from(spectrum_x_labels(&freqs, rows[1].width as usize))),
                rows[1],
            );
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let mut text = String::new();
        for y in 0..20u16 {
            for x in 0..80u16 {
                text.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(text.contains('█'), "expected a full block glyph");
        assert!(
            "▁▂▃▄▅▆▇".chars().any(|c| text.contains(c)),
            "expected an eighth-block glyph"
        );
        assert!(text.contains("Hz"), "expected Hz label in {text:?}");
        assert!(text.contains("kHz"), "expected kHz label in {text:?}");
    }
}

mod export;
mod api;
mod app;
mod audio_fx;
mod cavacore;
mod composite_bg;
mod fuzzy;
mod i18n;
mod kitty_bg;
#[cfg(target_os = "macos")]
mod media_macos;
mod mpris;
mod player;
mod spectrum;
mod tidal;
mod ui;

use anyhow::Result;
use app::{ApiStatus, App, AppEvent, InputMode, Tab};
use crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use tokio::time::interval;

struct TerminalRestorer;

impl Drop for TerminalRestorer {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        let _ = execute!(stdout, crossterm::cursor::Show);
    }
}

/// Build the image protocol picker: query the terminal, force kitty when we
/// provably run inside it, then let TUIDAL_IMAGE_PROTOCOL override everything.
fn build_picker() -> Option<ratatui_image::picker::Picker> {
    let mut picker = match ratatui_image::picker::Picker::from_query_stdio() {
        Ok(p) => Some(p),
        Err(e) => {
            kitty_bg::bg_log(&format!("picker query failed: {e:?}"));
            None
        }
    };
    // ratatui-image trusts an inherited TERM_PROGRAM (e.g. "vscode" when kitty
    // is launched from VS Code) over the terminal's own capability reply and
    // would pick the iTerm2 protocol, which kitty rejects. Force kitty when
    // we are demonstrably running inside it.
    let in_kitty = std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM").is_ok_and(|t| t.contains("kitty"));
    if let (Some(p), true) = (picker.as_mut(), in_kitty) {
        p.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
    }
    // TUIDAL_IMAGE_PROTOCOL=kitty|sixel|iterm2|halfblocks wins over detection,
    // including when the stdio query failed entirely.
    if let Ok(raw) = std::env::var("TUIDAL_IMAGE_PROTOCOL") {
        let forced = match raw.to_lowercase().as_str() {
            "kitty" => Some(ratatui_image::picker::ProtocolType::Kitty),
            "sixel" => Some(ratatui_image::picker::ProtocolType::Sixel),
            "iterm2" => Some(ratatui_image::picker::ProtocolType::Iterm2),
            "halfblocks" => Some(ratatui_image::picker::ProtocolType::Halfblocks),
            _ => {
                kitty_bg::bg_log(&format!("unknown TUIDAL_IMAGE_PROTOCOL={raw:?} — ignored"));
                None
            }
        };
        if let Some(t) = forced {
            picker
                .get_or_insert_with(|| {
                    ratatui_image::picker::Picker::from_fontsize((10, 20))
                })
                .set_protocol_type(t);
        }
    }
    kitty_bg::bg_log(&format!(
        "picker: in_kitty={in_kitty} protocol={:?} font_size={:?}",
        picker.as_ref().map(|p| p.protocol_type()),
        picker.as_ref().map(|p| p.font_size())
    ));
    picker
}

fn main() -> Result<()> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let api_status = Arc::new(RwLock::new(ApiStatus::default()));

    // On macOS, Now Playing / media keys require an AppKit run loop on the
    // main thread, so the tokio app moves to a spawned thread and the main
    // thread pumps NSApp until it exits.
    #[cfg(target_os = "macos")]
    {
        // Hidden flag: fake Now Playing session for media-keys testing.
        if std::env::args().any(|a| a == "--media-probe") {
            return media_macos::run_probe();
        }

        // Flips `done` even if the app thread panics.
        struct DoneGuard(Arc<AtomicBool>);
        impl Drop for DoneGuard {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let done = Arc::new(AtomicBool::new(false));
        let app_status = api_status.clone();
        let app_tx = event_tx.clone();
        let done_thread = done.clone();
        let handle = std::thread::Builder::new()
            .name("tuidal-app".to_string())
            .spawn(move || -> Result<()> {
                let _guard = DoneGuard(done_thread);
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(async_main(app_tx, event_rx, app_status))
            })?;
        media_macos::run_main_thread_loop(api_status, event_tx, done);
        handle
            .join()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("app thread panicked")))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async_main(event_tx, event_rx, api_status))
    }
}

async fn async_main(
    event_tx: mpsc::UnboundedSender<AppEvent>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    api_status: Arc<RwLock<ApiStatus>>,
) -> Result<()> {
    let _restorer = TerminalRestorer;
    let picker = build_picker();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialiser le daemon Python persistant
    let script_path = tidal::TidalClient::default_script_path();
    let python_path = tidal::TidalClient::default_python_path(&script_path);
    let tidal = tidal::TidalDaemonClient::spawn(&script_path, &python_path, "LOSSLESS")
        .await
        .unwrap_or_else(|e| {
            // Restore the terminal before exiting: process::exit skips Drop,
            // and raw mode was already enabled above.
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
            eprintln!("Erreur au démarrage du daemon Python : {e}");
            std::process::exit(1);
        });

    let mut app = App::new(tidal.clone());
    app.picker = picker;

    app.load_settings();
    // Synchroniser la qualité locale avec le daemon
    let _ = tidal.set_quality(app.quality.as_api_str()).await;

    app.status_msg = app.lang.strings().status_session_loading.to_string();
    if tidal.poll_device_token().await.unwrap_or(false) {
        app.status_msg = app.lang.strings().status_session_active.to_string();
        app.authenticated = true;
    } else {
        app.status_msg = app.lang.strings().status_press_l.to_string();
    }

    app.event_tx = Some(event_tx.clone());

    let api_handle = tokio::spawn(api::start_server(
        event_tx.clone(),
        api_status.clone(),
        tidal.clone(),
    ));

    let mpris_handle = tokio::spawn(mpris::start_mpris_server(
        api_status.clone(),
        event_tx.clone(),
    ));

    let result = run_app(&mut terminal, &mut app, event_rx, api_status).await;

    // Delete the spectrum background image before the terminal is restored.
    if matches!(
        app.picker.as_ref().map(|p| p.protocol_type()),
        Some(ratatui_image::picker::ProtocolType::Kitty)
    ) {
        use std::io::Write as _;
        let _ = write!(io::stdout(), "{}", kitty_bg::delete_sequence());
        let _ = io::stdout().flush();
    }

    // Graceful shutdown
    tidal.shutdown().await;
    api_handle.abort();
    mpris_handle.abort();

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    mut rx: mpsc::UnboundedReceiver<AppEvent>,
    api_status: Arc<RwLock<ApiStatus>>,
) -> Result<()> {
    let mut ui_tick = interval(Duration::from_millis(50));
    let mut auth_tick = interval(Duration::from_secs(5));
    auth_tick.reset();

    loop {
        if app.should_quit {
            break;
        }

        terminal.draw(|f| ui::draw(f, app))?;
        if let Some(seq) = app.bg_tick() {
            use std::io::Write as _;
            let mut out = io::stdout();
            out.write_all(seq.as_bytes())?;
            out.flush()?;
        }

        tokio::select! {
            Some(event) = rx.recv() => {
                app.handle_event(event);
            }

            _ = ui_tick.tick() => {
                app.player.tick();
                app.live_search_tick();
                app.update_spectrum(app.spectrum_bands_hint);
                if let Ok(mut s) = api_status.write() {
                    *s = app.api_status_snapshot();
                }

                if app.player.state == player::PlayerState::Stopped
                    && app.queue_index.is_some()
                    && !app.queue.is_empty()
                    && app.auto_advance
                {
                    app.auto_advance = false;
                    app.play_next_bg();
                }

                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        if key.modifiers == KeyModifiers::CONTROL
                            && key.code == KeyCode::Char('c')
                        {
                            app.player.stop();
                            app.should_quit = true;
                            continue;
                        }
                        if key.code == KeyCode::Char('l') && key.modifiers == KeyModifiers::ALT
                        {
                            app.cycle_lang();
                        } else {
                            match app.input_mode {
                                InputMode::Normal => handle_normal(key.code, app),
                                InputMode::Search => {
                                    handle_text_input(key.code, app, App::do_search_bg)
                                }
                                InputMode::Theme => {
                                    handle_text_input(key.code, app, App::do_theme_search_bg)
                                }
                                InputMode::Filter => handle_filter(key, app),
                                InputMode::Export => handle_export(key.code, app),
                            }
                        }
                    }
                }
            }

            _ = auth_tick.tick() => {
                if app.device_code.is_some() && !app.authenticated {
                    app.poll_auth_bg();
                }
            }
        }
    }
    Ok(())
}

fn handle_normal(key: KeyCode, app: &mut App) {
    // Effects tab: navigation/adjustment keys are intercepted; everything
    // else falls through to the global bindings below.
    if app.active_tab == Tab::Visualizer {
        match key {
            KeyCode::Char('b') => return app.bg_cycle_source(),
            KeyCode::Char('B') => {
                if app.spectrum_bg == kitty_bg::SpectrumBg::Wallhaven {
                    app.fetch_wallhaven_bg();
                }
                return;
            }
            KeyCode::Char('v') => return app.cycle_spectrum_style(),
            _ => {}
        }
    }
    if app.active_tab == Tab::Effects {
        match key {
            KeyCode::Up | KeyCode::Char('k') => return app.fx_move(-1),
            KeyCode::Down | KeyCode::Char('j') => return app.fx_move(1),
            KeyCode::Left => return app.fx_adjust(-1),
            KeyCode::Right => return app.fx_adjust(1),
            KeyCode::Enter => return app.fx_toggle(),
            KeyCode::Char('R') => return app.fx_reset_all(),
            KeyCode::Char('[') => return app.fx_cycle_preset(-1),
            KeyCode::Char(']') => return app.fx_cycle_preset(1),
            _ => {}
        }
    }
    match key {
        KeyCode::Char('q') => {
            app.player.stop();
            app.should_quit = true;
        }
        KeyCode::Char('/') | KeyCode::Char('s') => {
            app.input_mode = InputMode::Search;
            app.search_input.clear();
        }
        KeyCode::Char('g') => {
            app.input_mode = InputMode::Theme;
            app.search_input.clear();
            // The filter indexes the previous tab's list; drop it before
            // showing the Library.
            app.filter_clear();
            app.active_tab = Tab::Library;
        }
        KeyCode::Char('f') => {
            // Fuzzy filter only applies to list tabs.
            if matches!(app.active_tab, Tab::Search | Tab::Queue | Tab::Library) {
                app.input_mode = InputMode::Filter;
                app.filter_input.clear();
                app.filter_recompute();
            }
        }
        KeyCode::Esc => {
            // Esc closes an applied filter before anything else.
            if app.filter_matches.is_some() {
                app.filter_clear();
            }
        }
        KeyCode::Char('h') => {
            if app.split_active(app.body_width) {
                app.active_tab = Tab::Search;
                app.filter_clear();
            }
        }
        KeyCode::Char('l') => {
            if app.split_active(app.body_width) {
                app.active_tab = Tab::Queue;
                app.filter_clear();
            } else if !app.authenticated {
                app.start_login_bg();
            }
        }
        KeyCode::Char('L') => {
            if !app.authenticated {
                app.start_login_bg();
            }
        }
        KeyCode::Char('S') => app.cycle_layout(),
        KeyCode::Char('e') => app.open_export_menu(),
        KeyCode::Char('r') => app.start_track_radio_bg(),
        KeyCode::Char('R') => app.start_artist_radio_bg(),
        KeyCode::Char('i') => {
            if app.authenticated {
                app.load_library_bg();
            }
        }
        KeyCode::Char('F') => {
            if app.authenticated {
                app.load_fav_tracks_bg();
            }
        }
        KeyCode::Char('A') => {
            if app.authenticated {
                app.load_fav_albums_bg();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => app.selection_down(),
        KeyCode::Up | KeyCode::Char('k') => app.selection_up(),
        KeyCode::Enter => {
            if app.active_tab == Tab::Library {
                app.library_select_enter();
            } else {
                app.play_selected_bg();
            }
        }
        KeyCode::Char('a') => app.add_selected_to_queue(),
        KeyCode::Char(' ') => app.player.toggle_pause(),
        KeyCode::Char('n') => app.play_next_bg(),
        KeyCode::Char('p') => app.play_prev_bg(),
        KeyCode::Right => app.player.seek_forward(),
        KeyCode::Left => app.player.seek_backward(),
        KeyCode::Char('+') | KeyCode::Char('=') => app.volume_up(),
        KeyCode::Char('-') => app.volume_down(),
        KeyCode::Tab => app.next_tab(),
        KeyCode::Char('1') => app.set_quality(tidal::Quality::HiResLossless),
        KeyCode::Char('2') => app.set_quality(tidal::Quality::Lossless),
        KeyCode::Char('3') => app.set_quality(tidal::Quality::High),
        _ => {}
    }
}

/// Format chooser popup (InputMode::Export): 1/m M3U8, 2/c CSV, 3/j JSON.
fn handle_export(key: crossterm::event::KeyCode, app: &mut App) {
    match key {
        crossterm::event::KeyCode::Char('1') | crossterm::event::KeyCode::Char('m') => {
            app.export_as(crate::export::ExportDestination::File(
                crate::export::ExportFormat::M3u8,
            ))
        }
        crossterm::event::KeyCode::Char('2') | crossterm::event::KeyCode::Char('c') => {
            app.export_as(crate::export::ExportDestination::File(
                crate::export::ExportFormat::Csv,
            ))
        }
        crossterm::event::KeyCode::Char('3') | crossterm::event::KeyCode::Char('j') => {
            app.export_as(crate::export::ExportDestination::File(
                crate::export::ExportFormat::Json,
            ))
        }
        crossterm::event::KeyCode::Char('4') | crossterm::event::KeyCode::Char('t') => {
            app.export_as(crate::export::ExportDestination::Tidal)
        }
        crossterm::event::KeyCode::Char('5') | crossterm::event::KeyCode::Char('u') => {
            if app.export_upload_url.is_some() {
                app.export_as(crate::export::ExportDestination::Upload);
            }
        }
        crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
            app.input_mode = InputMode::Normal;
            app.export_target = None;
        }
        _ => {}
    }
}

/// Shared text-input handling for `InputMode::Search` and `InputMode::Theme`;
/// `on_enter` is the background task to launch on Enter. Every edit marks the
/// input dirty so `live_search_tick` can fire a debounced query.
fn handle_text_input(key: KeyCode, app: &mut App, on_enter: fn(&mut App)) {
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            // Keep an in-flight request: its result is still useful.
            app.live_search_dirty_at = None;
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            // Skip the query only when the shown input was already searched
            // (no pending edits and the last accepted result matches it).
            let stale = app.live_search_dirty_at.is_some()
                || match app.live_search_mode {
                    InputMode::Search => {
                        app.last_search_query.as_deref() != Some(app.search_input.as_str())
                    }
                    InputMode::Theme => {
                        app.theme_query.as_deref() != Some(app.search_input.as_str())
                    }
                    _ => true,
                };
            if stale {
                on_enter(app);
            }
        }
        KeyCode::Backspace => {
            app.search_input.pop();
            app.live_search_dirty_at = Some(Instant::now());
            app.live_search_mode = app.input_mode.clone();
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
            app.live_search_dirty_at = Some(Instant::now());
            app.live_search_mode = app.input_mode.clone();
        }
        _ => {}
    }
}

/// Input handling while `InputMode::Filter` is active: edits recompute the
/// fuzzy match, arrows/Ctrl-j/Ctrl-k move the selection, Enter performs the
/// tab's default action and keeps the filter applied, Esc cancels it.
fn handle_filter(key: event::KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.filter_clear();
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            if app.active_tab == Tab::Library {
                app.library_select_enter();
            } else {
                app.play_selected_bg();
            }
        }
        KeyCode::Down => app.selection_down(),
        KeyCode::Up => app.selection_up(),
        KeyCode::Char('j') if key.modifiers == KeyModifiers::CONTROL => app.selection_down(),
        KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => app.selection_up(),
        KeyCode::Backspace => {
            app.filter_input.pop();
            app.filter_recompute();
        }
        KeyCode::Char(c) => {
            app.filter_input.push(c);
            app.filter_recompute();
        }
        _ => {}
    }
}

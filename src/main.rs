mod api;
mod app;
mod audio_fx;
mod composite_bg;
mod i18n;
mod kitty_bg;
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
    time::Duration,
};
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

#[tokio::main]
async fn main() -> Result<()> {
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

    let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
    app.event_tx = Some(event_tx.clone());

    let api_status = Arc::new(RwLock::new(ApiStatus::default()));
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
                                InputMode::Search => handle_search(key.code, app),
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
        KeyCode::Char('l') | KeyCode::Char('L') => {
            if !app.authenticated {
                app.start_login_bg();
            }
        }
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
        KeyCode::Down | KeyCode::Char('j') => {
            if app.active_tab == Tab::Library {
                if app.collection_view == app::CollectionView::Albums {
                    let max = app.fav_albums.len();
                    if max > 0 {
                        app.fav_album_selected = (app.fav_album_selected + 1) % max;
                    }
                } else {
                    let max = app.playlists.len() + app.mixes.len();
                    if max > 0 {
                        app.library_selected = (app.library_selected + 1) % max;
                    }
                }
            } else {
                app.next_track();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.active_tab == Tab::Library {
                if app.collection_view == app::CollectionView::Albums {
                    let max = app.fav_albums.len();
                    if max > 0 {
                        app.fav_album_selected = if app.fav_album_selected == 0 {
                            max - 1
                        } else {
                            app.fav_album_selected - 1
                        };
                    }
                } else {
                    let max = app.playlists.len() + app.mixes.len();
                    if max > 0 {
                        app.library_selected = if app.library_selected == 0 {
                            max - 1
                        } else {
                            app.library_selected - 1
                        };
                    }
                }
            } else {
                app.prev_track();
            }
        }
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

fn handle_search(key: KeyCode, app: &mut App) {
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.do_search_bg();
        }
        KeyCode::Backspace => {
            app.search_input.pop();
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
        }
        _ => {}
    }
}

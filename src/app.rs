use crate::audio_fx::{AudioFx, FxControl, PRESETS};
use crate::composite_bg;
use crate::fuzzy::{self, FuzzyMatch};
use crate::i18n::Lang;
use crate::kitty_bg::{self, BgLayout, SpectrumBg};
use crate::spectrum::{AnalysisBuffer, SpectrumAnalyzer};
use crate::player::{Player, TrackInfo};
use crate::tidal::{
    Album, Artist, CoverInfo, FavAlbum, Lyrics, Mix, Playlist, Quality, StreamInfo,
    TidalDaemonClient, Track,
};
use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::{Protocol, StatefulProtocol};
use serde::Deserialize as DeserializeAttr;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Theme,
    /// fzf-style local fuzzy filter on the active list.
    Filter,
}

/// Fuzzy haystack for a track row: "title artists album".
fn track_haystack(t: &Track) -> String {
    format!("{} {} {}", t.title, t.artist_names(), t.album.title)
}

/// Spectrum tab rendering style.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpectrumStyle {
    Curve,
    #[default]
    Bars,
}

/// Main-area layout: side-by-side Search|Queue panes, or one tab at a time.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    #[default]
    Split,
    Tabs,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Search,
    Queue,
    Now,
    Visualizer,
    Effects,
    Library,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollectionView {
    Tracks,
    Albums,
}

pub enum AppEvent {
    /// Track-search result; `query` is the request text, `generation` lets
    /// the app drop results superseded by a newer search.
    SearchDone {
        generation: u64,
        query: String,
        result: Result<Vec<Track>, String>,
    },
    ThemeSearchDone {
        generation: u64,
        query: String,
        result: Result<Vec<Playlist>, String>,
    },
    StreamReady {
        track: Track,
        info: StreamInfo,
        queue_index: usize,
        generation: u64,
    },
    StreamError {
        error: String,
        generation: u64,
    },
    AuthStarted {
        url: String,
        code: String,
        device_code: String,
    },
    AuthDone,
    AuthError(String),
    StatusMsg(String),
    CoverReady {
        info: CoverInfo,
        image: DynamicImage,
    },
    CoverError,
    LibraryLoaded {
        playlists: Vec<Playlist>,
        mixes: Vec<Mix>,
    },
    PlaylistTracksLoaded(Vec<Track>),
    FavTracksLoaded(Vec<Track>),
    FavAlbumsLoaded(Vec<FavAlbum>),
    ApiCmd(ApiCommand),
    LyricsReady(Lyrics),
    LyricsError,
    WallhavenReady(DynamicImage),
    WallhavenError(String),
    BackgroundEncoded { key: BgKey, sequence: String },
    BackgroundComposed {
        key: BgKey,
        image: Option<Arc<image::RgbaImage>>,
    },
    SpectrumFrameEncoded {
        key: BgKey,
        area: ratatui::layout::Rect,
        proto: Option<Protocol>,
    },
}

/// Identifies which image/geometry the background currently targets.
#[derive(Clone, PartialEq)]
pub struct BgKey {
    source: SpectrumBg,
    image_tag: u64,
    px_w: u32,
    px_h: u32,
}

/// How the spectrum background is produced, depending on the terminal's
/// graphics protocol.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BgMode {
    /// kitty protocol: the image is placed under the text via z-index.
    Kitty,
    /// Sixel / iTerm2: the spectrum is drawn in pixels on the background and
    /// the composite is sent through ratatui-image each frame.
    Composite,
    /// Halfblocks or no picker: no image background.
    Unsupported,
}

/// Private bookkeeping for the kitty background image lifecycle.
#[derive(Default)]
struct BgState {
    transmitted_key: Option<BgKey>,
    pending_key: Option<BgKey>,
    placed_area: Option<ratatui::layout::Rect>,
    sequence_to_write: String,
    frame_counter: u8,
    logged_unsupported: bool,
    /// Key whose encode failed (panic or encode error): never retried.
    failed_key: Option<BgKey>,
    /// Composite mode: cached composed background for the current key.
    composed: Option<(BgKey, Arc<image::RgbaImage>)>,
    /// Composite mode: at most one frame encode in flight.
    frame_in_flight: bool,
    /// Composite mode: latest encoded frame, rendered by the ui.
    composite_proto: Option<Protocol>,
    composite_area: Option<ratatui::layout::Rect>,
}

pub enum ApiCommand {
    PlayPause,
    Next,
    Prev,
    VolumeUp,
    VolumeDown,
    VolumeSet(u8),
    SeekForward,
    SeekBackward,
    PlayTrack(ApiTrack),
    ToggleShuffle,
    CycleRepeat,
    Stop,
    Seek(i64),
    SetPosition(u64),
}

#[derive(Debug, Clone, DeserializeAttr)]
pub struct ApiTrack {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    One,
    All,
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::All
    }
}

#[derive(Clone, Default, Serialize)]
pub struct ApiStatus {
    pub state: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<u64>,
    pub elapsed: u64,
    pub volume: u8,
    pub progress: f64,
    pub bit_depth: Option<u32>,
    pub sample_rate: Option<u32>,
    pub codec: Option<String>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub authenticated: bool,
    pub track_id: Option<u64>,
    pub queue: Vec<Track>,
    pub queue_index: Option<usize>,
    pub cover_url: Option<String>,
}

pub struct App {
    pub tidal: Arc<TidalDaemonClient>,
    pub player: Player,
    pub input_mode: InputMode,
    pub active_tab: Tab,

    pub search_input: String,
    pub search_results: Vec<Track>,
    pub queue: Vec<Track>,

    /// Selection in the Search results list.
    pub search_selected: usize,
    /// Selection in the Queue list.
    pub queue_selected: usize,
    /// Main layout (split panes vs tabs).
    pub layout: LayoutMode,
    /// Body width written by draw_body each frame (for split_active()).
    pub body_width: u16,
    pub queue_index: Option<usize>,

    pub authenticated: bool,
    pub status_msg: String,
    pub loading: bool,
    pub auto_advance: bool,
    pub should_quit: bool,

    pub device_code: Option<String>,
    pub user_code: Option<String>,
    pub auth_url: Option<String>,

    pub event_tx: Option<UnboundedSender<AppEvent>>,

    pub cover_info: Option<CoverInfo>,
    pub cover_image: Option<DynamicImage>,
    pub cover_proto: Option<StatefulProtocol>,
    pub picker: Option<Picker>,
    pub last_img_area: Option<(u16, u16)>,
    pub current_track_id: Option<u64>,
    pub stream_generation: u64,

    pub playlists: Vec<Playlist>,
    pub mixes: Vec<Mix>,
    pub library_selected: usize,
    pub fav_albums: Vec<FavAlbum>,
    pub fav_album_selected: usize,
    pub collection_view: CollectionView,
    /// Query behind the last theme search; `Some` while `self.playlists`
    /// holds theme search results (cleared on library reload).
    pub theme_query: Option<String>,

    pub lang: Lang,
    pub quality: Quality,
    pub shuffle: bool,
    pub repeat: RepeatMode,

    pub lyrics: Option<Lyrics>,

    /// Parallel stream analysis for the spectrum tab (downloads the same URL).
    pub analysis: Option<Arc<std::sync::Mutex<AnalysisBuffer>>>,
    analysis_cancel: Option<Arc<AtomicBool>>,
    pub spectrum: SpectrumAnalyzer,
    /// Band count requested by the UI (depends on widget width).
    pub spectrum_bands_hint: usize,
    /// Audio effects applied live through the mpv af filter chain.
    pub audio_fx: AudioFx,
    /// Selected row in the Effects tab (index into FxControl::ALL).
    pub fx_cursor: usize,
    /// Background image source for the spectrum tab.
    pub spectrum_bg: SpectrumBg,
    /// Spectrum rendering style (curve or cava-style bars).
    pub spectrum_style: SpectrumStyle,
    /// wallhaven.cc search query (persisted in settings).
    pub wallhaven_query: String,
    /// Last wallpaper downloaded from wallhaven.
    pub wallhaven_image: Option<DynamicImage>,
    /// Max number of results requested in track/playlist searches (settings).
    pub search_limit: usize,
    /// Live search-as-you-type: last keystroke time awaiting the debounce.
    pub live_search_dirty_at: Option<std::time::Instant>,
    /// A search request is currently running (daemon answers sequentially).
    pub live_search_in_flight: bool,
    /// Input mode that was active when the dirty flag was set.
    pub live_search_mode: InputMode,
    /// Bumped for each spawned search; stale results are dropped.
    search_generation: u64,
    /// Query text of the last accepted track-search result.
    pub last_search_query: Option<String>,
    /// Current fuzzy-filter text (`InputMode::Filter`).
    pub filter_input: String,
    /// Filtered view of the active list (indices + match positions);
    /// `None` means no filter is applied.
    pub filter_matches: Option<Vec<FuzzyMatch>>,
    /// Chart area written by the UI each frame (None when not on the tab).
    pub spectrum_area: Option<ratatui::layout::Rect>,
    /// Monotonic tag bumped each time a wallhaven image arrives.
    wallhaven_seq: u64,
    bg_state: BgState,
}

impl App {
    pub fn new(tidal: Arc<TidalDaemonClient>) -> Self {
        Self {
            tidal,
            player: Player::new(),
            input_mode: InputMode::Normal,
            active_tab: Tab::Search,
            search_input: String::new(),
            search_results: Vec::new(),
            queue: Vec::new(),
            search_selected: 0,
            queue_selected: 0,
            layout: LayoutMode::default(),
            body_width: 0,
            queue_index: None,
            authenticated: false,
            status_msg: String::new(),
            loading: false,
            auto_advance: false,
            should_quit: false,
            device_code: None,
            user_code: None,
            auth_url: None,
            event_tx: None,
            cover_info: None,
            cover_image: None,
            cover_proto: None,
            picker: None,
            last_img_area: None,
            current_track_id: None,
            stream_generation: 0,
            playlists: Vec::new(),
            mixes: Vec::new(),
            library_selected: 0,
            fav_albums: Vec::new(),
            fav_album_selected: 0,
            collection_view: CollectionView::Tracks,
            theme_query: None,
            lang: Lang::Fr,
            quality: Quality::Lossless,
            shuffle: false,
            repeat: RepeatMode::All,
            lyrics: None,
            analysis: None,
            analysis_cancel: None,
            spectrum: SpectrumAnalyzer::new(),
            spectrum_bands_hint: 96,
            audio_fx: AudioFx::default(),
            fx_cursor: 0,
            spectrum_bg: SpectrumBg::Cover,
            spectrum_style: SpectrumStyle::default(),
            wallhaven_query: "abstract".to_string(),
            wallhaven_image: None,
            search_limit: 50,
            live_search_dirty_at: None,
            live_search_in_flight: false,
            live_search_mode: InputMode::Normal,
            search_generation: 0,
            last_search_query: None,
            filter_input: String::new(),
            filter_matches: None,
            spectrum_area: None,
            wallhaven_seq: 0,
            bg_state: BgState::default(),
        }
    }

    pub fn cycle_lang(&mut self) {
        self.lang = self.lang.cycle();
        self.status_msg = self.lang.lang_changed();
        self.save_settings();
    }

    fn tx(&self) -> UnboundedSender<AppEvent> {
        self.event_tx.clone().expect("event_tx non initialisé")
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SearchDone {
                generation,
                query,
                result,
            } => {
                if generation != self.search_generation {
                    return;
                }
                self.live_search_in_flight = false;
                match result {
                    Ok(results) => {
                        self.status_msg = if results.is_empty() {
                            self.lang.strings().status_no_results.to_string()
                        } else {
                            self.lang.results_count(results.len())
                        };
                        self.last_search_query = Some(query);
                        self.search_results = results;
                        self.search_selected = 0;
                        self.active_tab = Tab::Search;
                        self.loading = false;
                        self.filter_clear();
                    }
                    Err(e) => {
                        self.status_msg = self.lang.search_error(&e);
                        self.loading = false;
                    }
                }
            }
            AppEvent::ThemeSearchDone {
                generation,
                query,
                result,
            } => {
                if generation != self.search_generation {
                    return;
                }
                self.live_search_in_flight = false;
                match result {
                    Ok(playlists) => {
                        self.status_msg = if playlists.is_empty() {
                            self.lang.strings().status_no_results.to_string()
                        } else {
                            self.lang.theme_results_count(playlists.len(), &query)
                        };
                        self.theme_query = Some(query);
                        self.playlists = playlists;
                        self.mixes.clear();
                        self.library_selected = 0;
                        self.collection_view = CollectionView::Tracks;
                        self.active_tab = Tab::Library;
                        self.loading = false;
                        self.filter_clear();
                    }
                    Err(e) => {
                        self.status_msg = self.lang.search_error(&e);
                        self.loading = false;
                    }
                }
            }
            AppEvent::StreamReady {
                track,
                info,
                queue_index,
                generation,
            } => {
                if generation != self.stream_generation {
                    return;
                }
                self.queue_index = Some(queue_index);
                self.current_track_id = Some(track.id);
                let ti = TrackInfo {
                    title: track.title.clone(),
                    artist: track.artist_names(),
                    album: track.album.title.clone(),
                    duration: track.duration,
                    bit_depth: info.bit_depth,
                    sample_rate: info.sample_rate,
                    codec: info.codec.clone(),
                };
                self.status_msg = format!(
                    "▶ {} — {} | {}/{} {}",
                    track.artist_names(),
                    track.title,
                    info.bit_depth,
                    info.sample_rate,
                    info.codec.to_uppercase()
                );
                self.player.play(&info.url, ti);
                self.start_analysis(&info.url);
                // Fresh random wallpaper per track when that source is active.
                if self.spectrum_bg == SpectrumBg::Wallhaven {
                    self.fetch_wallhaven_bg();
                }
                self.loading = false;
                self.auto_advance = true;
                self.load_cover_bg(track.id);
                self.load_lyrics_bg(track.id);
            }
            AppEvent::StreamError { error, generation } => {
                if generation != 0 && generation != self.stream_generation {
                    return;
                }
                self.status_msg = self.lang.stream_error(&error);
                self.loading = false;
                if generation != 0 {
                    if let Some(i) = self.queue_index {
                        let next = i + 1;
                        if next < self.queue.len() {
                            let track = self.queue[next].clone();
                            self.stream_track_bg(track, next);
                        }
                    }
                }
            }
            AppEvent::AuthStarted {
                url,
                code,
                device_code,
            } => {
                self.device_code = Some(device_code);
                self.user_code = Some(code.clone());
                self.auth_url = Some(url.clone());
                let url_to_open = if url.starts_with("http://") || url.starts_with("https://") {
                    url.clone()
                } else {
                    format!("https://{}", url)
                };
                if let Err(e) = open::that(&url_to_open) {
                    self.status_msg = self.lang.browser_failed(&e.to_string(), &url);
                } else {
                    self.status_msg = self.lang.browser_opened(&code);
                }
                self.loading = false;
            }
            AppEvent::AuthDone => {
                self.authenticated = true;
                self.device_code = None;
                self.user_code = None;
                self.auth_url = None;
                self.status_msg = self.lang.strings().status_auth_done.to_string();
                self.loading = false;
            }
            AppEvent::AuthError(e) => {
                self.status_msg = self.lang.auth_error(&e);
                self.loading = false;
            }
            AppEvent::StatusMsg(msg) => {
                self.status_msg = msg;
            }
            AppEvent::CoverReady { info, image } => {
                self.cover_info = Some(info);
                self.cover_image = Some(image);
                self.cover_proto = None;
                self.last_img_area = None;
            }
            AppEvent::CoverError => {
                self.cover_image = None;
                self.cover_proto = None;
            }
            AppEvent::LyricsReady(lyrics) => {
                self.lyrics = Some(lyrics);
            }
            AppEvent::LyricsError => {
                self.lyrics = None;
            }
            AppEvent::WallhavenReady(img) => {
                self.wallhaven_seq += 1;
                self.wallhaven_image = Some(img);
            }
            AppEvent::WallhavenError(e) => {
                self.status_msg = self.lang.bg_error(&e);
            }
            AppEvent::BackgroundEncoded { key, sequence } => {
                if self.bg_state.pending_key.as_ref() == Some(&key) {
                    self.bg_state.pending_key = None;
                    if sequence.is_empty() {
                        kitty_bg::bg_log("encode failed");
                        self.bg_state.failed_key = Some(key);
                    } else {
                        kitty_bg::bg_log(&format!("encoded: len={}", sequence.len()));
                        self.bg_state.sequence_to_write =
                            kitty_bg::delete_sequence() + &sequence;
                        self.bg_state.transmitted_key = Some(key);
                        self.bg_state.placed_area = None;
                    }
                }
            }
            AppEvent::BackgroundComposed { key, image } => {
                if self.bg_state.pending_key.as_ref() == Some(&key) {
                    self.bg_state.pending_key = None;
                    match image {
                        None => {
                            kitty_bg::bg_log("compose failed");
                            self.bg_state.failed_key = Some(key);
                        }
                        Some(img) => {
                            kitty_bg::bg_log(&format!(
                                "composed: {}x{}",
                                img.width(),
                                img.height()
                            ));
                            self.bg_state.composed = Some((key, img));
                            self.bg_state.composite_proto = None;
                        }
                    }
                }
            }
            AppEvent::SpectrumFrameEncoded { key, area, proto } => {
                self.bg_state.frame_in_flight = false;
                if self
                    .bg_state
                    .composed
                    .as_ref()
                    .map(|(k, _)| k == &key)
                    .unwrap_or(false)
                {
                    match proto {
                        Some(p) => {
                            self.bg_state.composite_proto = Some(p);
                            self.bg_state.composite_area = Some(area);
                        }
                        None => {
                            kitty_bg::bg_log("frame encode failed");
                            self.bg_state.failed_key = Some(key);
                        }
                    }
                }
            }
            AppEvent::LibraryLoaded { playlists, mixes } => {
                self.playlists = playlists;
                self.mixes = mixes;
                self.active_tab = Tab::Library;
                self.loading = false;
                self.status_msg = self
                    .lang
                    .library_loaded(self.playlists.len(), self.mixes.len());
                self.filter_clear();
            }
            AppEvent::PlaylistTracksLoaded(tracks) => {
                self.queue = tracks;
                self.queue_index = None;
                self.queue_selected = 0;
                self.active_tab = Tab::Queue;
                self.loading = false;
                self.status_msg = self.lang.tracks_loaded(self.queue.len());
                self.filter_clear();
                self.clamp_selections();
            }
            AppEvent::FavTracksLoaded(tracks) => {
                self.queue = tracks;
                self.queue_index = None;
                self.queue_selected = 0;
                self.active_tab = Tab::Queue;
                self.loading = false;
                self.status_msg = self.lang.fav_tracks_loaded(self.queue.len());
                self.filter_clear();
                self.clamp_selections();
            }
            AppEvent::FavAlbumsLoaded(albums) => {
                self.fav_albums = albums;
                self.fav_album_selected = 0;
                self.collection_view = CollectionView::Albums;
                self.active_tab = Tab::Library;
                self.loading = false;
                self.status_msg = self.lang.fav_albums_loaded(self.fav_albums.len());
                self.filter_clear();
            }
            AppEvent::ApiCmd(cmd) => match cmd {
                ApiCommand::PlayPause => {
                    self.player.toggle_pause();
                }
                ApiCommand::Next => {
                    self.play_next_bg();
                }
                ApiCommand::Prev => {
                    self.play_prev_bg();
                }
                ApiCommand::VolumeUp => {
                    self.volume_up();
                }
                ApiCommand::VolumeDown => {
                    self.volume_down();
                }
                ApiCommand::VolumeSet(v) => {
                    self.set_volume(v);
                }
                ApiCommand::SeekForward => {
                    self.player.seek_forward();
                }
                ApiCommand::SeekBackward => {
                    self.player.seek_backward();
                }
                ApiCommand::ToggleShuffle => {
                    self.shuffle = !self.shuffle;
                    self.status_msg = if self.shuffle {
                        "Shuffle: on".into()
                    } else {
                        "Shuffle: off".into()
                    };
                }
                ApiCommand::CycleRepeat => {
                    self.repeat = match self.repeat {
                        RepeatMode::All => RepeatMode::One,
                        RepeatMode::One => RepeatMode::Off,
                        RepeatMode::Off => RepeatMode::All,
                    };
                    self.status_msg = format!(
                        "Repeat: {}",
                        match self.repeat {
                            RepeatMode::All => "all",
                            RepeatMode::One => "one",
                            RepeatMode::Off => "off",
                        }
                    );
                }
                ApiCommand::Stop => {
                    self.player.stop();
                    self.stop_analysis();
                }
                ApiCommand::Seek(secs) => {
                    self.player.seek_relative(secs);
                }
                ApiCommand::SetPosition(secs) => {
                    self.player.seek_absolute(secs);
                }
                ApiCommand::PlayTrack(api_track) => {
                    let track = Track {
                        id: api_track.id,
                        title: api_track.title.clone(),
                        duration: api_track.duration,
                        track_number: None,
                        artists: vec![Artist {
                            id: 0,
                            name: api_track.artist.clone(),
                        }],
                        album: Album {
                            id: 0,
                            title: api_track.album.clone(),
                        },
                        audio_quality: None,
                        explicit: None,
                    };
                    if !self.queue.iter().any(|t| t.id == track.id) {
                        self.queue.push(track.clone());
                    }
                    let qi = self
                        .queue
                        .iter()
                        .position(|t| t.id == track.id)
                        .unwrap_or(0);
                    self.stream_track_bg(track, qi);
                }
            },
        }
    }

    // ── Background tasks (simplifiés — utilisent Arc<TidalDaemonClient>) ────

    pub fn do_search_bg(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required.to_string();
            return;
        }
        if self.search_input.is_empty() {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.searching(&self.search_input);
        self.search_generation += 1;
        let generation = self.search_generation;
        self.live_search_in_flight = true;
        self.live_search_dirty_at = None;
        let tx = self.tx();
        let query = self.search_input.clone();
        let limit = self.search_limit;
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            let result = tidal
                .search(&query, limit)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::SearchDone {
                generation,
                query,
                result,
            });
        });
    }

    /// Search editorial playlists matching a musical theme/genre ("trip hop",
    /// "classical"...). Results land in `self.playlists` on the Library tab.
    pub fn do_theme_search_bg(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required.to_string();
            return;
        }
        if self.search_input.is_empty() {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.searching_theme(&self.search_input);
        self.search_generation += 1;
        let generation = self.search_generation;
        self.live_search_in_flight = true;
        self.live_search_dirty_at = None;
        let tx = self.tx();
        let query = self.search_input.clone();
        let limit = self.search_limit;
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            let result = tidal
                .search_playlists(&query, limit)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::ThemeSearchDone {
                generation,
                query,
                result,
            });
        });
    }

    /// Live search-as-you-type: called from the UI tick. Fires a search for
    /// the current `search_input` 300 ms after the last keystroke, at most
    /// one request in flight (the Python daemon answers sequentially). When
    /// a result arrives while the flag is dirty again, the next tick fires
    /// the follow-up query automatically.
    pub fn live_search_tick(&mut self) {
        let Some(dirty) = self.live_search_dirty_at else {
            return;
        };
        if dirty.elapsed() < Duration::from_millis(300) || self.live_search_in_flight {
            return;
        }
        if self.search_input.chars().count() < 3 {
            return;
        }
        match self.live_search_mode {
            InputMode::Search => self.do_search_bg(),
            InputMode::Theme => self.do_theme_search_bg(),
            InputMode::Normal | InputMode::Filter => {}
        }
    }

    pub fn add_selected_to_queue(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required_short.to_string();
            return;
        }
        let Some(idx) = self.resolve_index(self.selected()) else {
            return;
        };
        let track = match self.active_tab {
            Tab::Search => self.search_results.get(idx).cloned(),
            Tab::Queue => self.queue.get(idx).cloned(),
            Tab::Now | Tab::Visualizer | Tab::Effects | Tab::Library => return,
        };
        let Some(track) = track else { return };
        if self.queue.iter().any(|t| t.id == track.id) {
            self.status_msg = self.lang.strings().status_added_to_queue.to_string();
            return;
        }
        self.queue.push(track);
        self.status_msg = self.lang.strings().status_added_to_queue.to_string();
    }

    pub fn play_selected_bg(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required_short.to_string();
            return;
        }
        let Some(idx) = self.resolve_index(self.selected()) else {
            return;
        };
        let track = match self.active_tab {
            Tab::Search => self.search_results.get(idx).cloned(),
            Tab::Queue => self.queue.get(idx).cloned(),
            Tab::Now | Tab::Visualizer | Tab::Effects | Tab::Library => return,
        };
        let Some(track) = track else { return };
        let queue_index = if self.active_tab == Tab::Search {
            if !self.queue.iter().any(|t| t.id == track.id) {
                self.queue.push(track.clone());
            }
            self.queue
                .iter()
                .position(|t| t.id == track.id)
                .unwrap_or(0)
        } else {
            idx
        };
        self.stream_track_bg(track, queue_index);
    }

    pub fn play_next_bg(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let next = if self.repeat == RepeatMode::One {
            self.queue_index.unwrap_or(0)
        } else if self.shuffle {
            self.random_queue_index()
        } else {
            match self.queue_index {
                Some(i) if i + 1 < self.queue.len() => i + 1,
                _ => match self.repeat {
                    RepeatMode::Off => return,
                    _ => 0,
                },
            }
        };
        let track = self.queue[next].clone();
        self.stream_track_bg(track, next);
    }

    fn random_queue_index(&self) -> usize {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        let seed = nanos ^ (self.player.elapsed.as_nanos() as usize);
        if self.queue.len() <= 1 {
            return 0;
        }
        let candidate = seed % self.queue.len();
        if Some(candidate) == self.queue_index {
            (candidate + 1) % self.queue.len()
        } else {
            candidate
        }
    }

    pub fn play_prev_bg(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let prev = match self.queue_index {
            Some(i) if i > 0 => i - 1,
            _ => self.queue.len().saturating_sub(1),
        };
        let track = self.queue[prev].clone();
        self.stream_track_bg(track, prev);
    }

    fn stream_track_bg(&mut self, track: Track, queue_index: usize) {
        self.auto_advance = false;
        self.stream_generation += 1;
        let generation = self.stream_generation;
        self.queue_index = Some(queue_index);
        self.loading = true;
        self.status_msg = self.lang.loading_stream(&track.title);
        self.cover_image = None;
        self.cover_info = None;
        self.cover_proto = None;
        self.lyrics = None;
        self.player.stop();
        self.stop_analysis();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        let quality = self.quality.as_api_str().to_string();
        tokio::spawn(async move {
            match tidal.get_stream_info(track.id, &quality).await {
                Ok(info) => {
                    let _ = tx.send(AppEvent::StreamReady {
                        track,
                        info,
                        queue_index,
                        generation,
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        error: e.to_string(),
                        generation,
                    });
                }
            }
        });
    }

    fn load_lyrics_bg(&mut self, track_id: u64) {
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.get_lyrics(track_id).await {
                Ok(l) => {
                    let _ = tx.send(AppEvent::LyricsReady(l));
                }
                Err(_) => {
                    let _ = tx.send(AppEvent::LyricsError);
                }
            }
        });
    }

    fn load_cover_bg(&mut self, track_id: u64) {
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            let cover_info = match tidal.get_cover(track_id).await {
                Ok(c) => c,
                Err(_) => {
                    let _ = tx.send(AppEvent::CoverError);
                    return;
                }
            };
            let image_bytes = match reqwest::get(&cover_info.url).await {
                Ok(r) => match r.bytes().await {
                    Ok(b) => b,
                    Err(_) => {
                        let _ = tx.send(AppEvent::CoverError);
                        return;
                    }
                },
                Err(_) => {
                    let _ = tx.send(AppEvent::CoverError);
                    return;
                }
            };
            match image::load_from_memory(&image_bytes) {
                Ok(img) => {
                    let _ = tx.send(AppEvent::CoverReady {
                        info: cover_info,
                        image: img,
                    });
                }
                Err(_) => {
                    let _ = tx.send(AppEvent::CoverError);
                }
            }
        });
    }

    pub fn start_login_bg(&mut self) {
        self.loading = true;
        self.status_msg = self.lang.strings().status_starting_login.to_string();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.start_device_auth().await {
                Ok((device_code, user_code, url)) => {
                    let _ = tx.send(AppEvent::AuthStarted {
                        url,
                        code: user_code,
                        device_code,
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::AuthError(e.to_string()));
                }
            }
        });
    }

    pub fn poll_auth_bg(&mut self) {
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.poll_device_token().await {
                Ok(true) => {
                    let _ = tx.send(AppEvent::AuthDone);
                }
                Ok(false) => {}
                Err(e) => {
                    let _ = tx.send(AppEvent::StatusMsg(format!("Error poll: {e}")));
                }
            }
        });
    }

    pub fn load_library_bg(&mut self) {
        if !self.authenticated {
            return;
        }
        self.theme_query = None;
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_lib.to_string();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            let playlists = tidal.get_user_playlists().await.unwrap_or_default();
            let mixes = tidal.get_user_mixes().await.unwrap_or_default();
            let _ = tx.send(AppEvent::LibraryLoaded { playlists, mixes });
        });
    }

    pub fn load_playlist_tracks_bg(&mut self, uuid: String) {
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_playlist.to_string();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.get_playlist_tracks(&uuid).await {
                Ok(tracks) => {
                    let _ = tx.send(AppEvent::PlaylistTracksLoaded(tracks));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        error: e.to_string(),
                        generation: 0,
                    });
                }
            }
        });
    }

    pub fn load_mix_tracks_bg(&mut self, mix_id: String) {
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_mix.to_string();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.get_mix_tracks(&mix_id).await {
                Ok(tracks) => {
                    let _ = tx.send(AppEvent::PlaylistTracksLoaded(tracks));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        error: e.to_string(),
                        generation: 0,
                    });
                }
            }
        });
    }

    pub fn library_select_enter(&mut self) {
        if self.collection_view == CollectionView::Albums {
            let Some(idx) = self.resolve_index(self.fav_album_selected) else {
                return;
            };
            if let Some(album) = self.fav_albums.get(idx) {
                let id = album.id;
                let title = album.title.clone();
                self.load_album_tracks_bg(id, title);
            }
            return;
        }
        let Some(idx) = self.resolve_index(self.library_selected) else {
            return;
        };
        let total_playlists = self.playlists.len();
        if idx < total_playlists {
            let uuid = self.playlists[idx].uuid.clone();
            self.load_playlist_tracks_bg(uuid);
        } else {
            let mix_idx = idx - total_playlists;
            if let Some(mix) = self.mixes.get(mix_idx) {
                let mix_id = mix.id.clone();
                self.load_mix_tracks_bg(mix_id);
            }
        }
    }

    pub fn set_quality(&mut self, q: Quality) {
        self.quality = q;
        self.status_msg = self.lang.quality_changed(q.label());
        let tidal = self.tidal.clone();
        let label = q.as_api_str().to_string();
        tokio::spawn(async move {
            let _ = tidal.set_quality(&label).await;
        });
        self.save_settings();
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Search => Tab::Queue,
            Tab::Queue => Tab::Now,
            Tab::Now => Tab::Visualizer,
            Tab::Visualizer => Tab::Effects,
            Tab::Effects => Tab::Library,
            Tab::Library => {
                self.collection_view = CollectionView::Tracks;
                Tab::Search
            }
        };
        // Keep the per-pane selections: switching tabs/panes no longer
        // resets them.
        self.filter_clear();
    }

    /// Selection index of the list for the active tab.
    pub fn selected(&self) -> usize {
        match self.active_tab {
            Tab::Queue => self.queue_selected,
            _ => self.search_selected,
        }
    }

    /// Mutable access to the selection of the active tab.
    pub fn selected_mut(&mut self) -> &mut usize {
        match self.active_tab {
            Tab::Queue => &mut self.queue_selected,
            _ => &mut self.search_selected,
        }
    }

    /// Keep both selections inside their lists after a mutation.
    fn clamp_selections(&mut self) {
        if self.search_selected >= self.search_results.len() {
            self.search_selected = self.search_results.len().saturating_sub(1);
        }
        if self.queue_selected >= self.queue.len() {
            self.queue_selected = self.queue.len().saturating_sub(1);
        }
    }

    /// Split view is active on Search/Queue when the terminal is wide enough.
    pub fn split_active(&self, width: u16) -> bool {
        self.layout == LayoutMode::Split
            && width >= 100
            && matches!(self.active_tab, Tab::Search | Tab::Queue)
    }

    /// Toggle split panes vs single-tab layout (key `S`).
    pub fn cycle_layout(&mut self) {
        self.layout = match self.layout {
            LayoutMode::Split => LayoutMode::Tabs,
            LayoutMode::Tabs => LayoutMode::Split,
        };
        self.status_msg = self.lang.layout_changed(self.lang.layout_label(self.layout));
        self.save_settings();
    }

    pub fn current_list_len(&self) -> usize {
        if let Some(m) = &self.filter_matches {
            return m.len();
        }
        match self.active_tab {
            Tab::Search => self.search_results.len(),
            Tab::Queue => self.queue.len(),
            Tab::Now | Tab::Visualizer | Tab::Effects => 0,
            Tab::Library => self.playlists.len() + self.mixes.len(),
        }
    }

    /// Items in the Library playlists+mixes view, honouring the filter.
    pub fn library_len(&self) -> usize {
        match &self.filter_matches {
            Some(m) => m.len(),
            None => self.playlists.len() + self.mixes.len(),
        }
    }

    /// Items in the Library favourite-albums view, honouring the filter.
    pub fn fav_albums_len(&self) -> usize {
        match &self.filter_matches {
            Some(m) => m.len(),
            None => self.fav_albums.len(),
        }
    }

    /// Move the selection down one row in the active list (wraps around).
    pub fn selection_down(&mut self) {
        if self.active_tab == Tab::Library {
            if self.collection_view == CollectionView::Albums {
                let max = self.fav_albums_len();
                if max > 0 {
                    self.fav_album_selected = (self.fav_album_selected + 1) % max;
                }
            } else {
                let max = self.library_len();
                if max > 0 {
                    self.library_selected = (self.library_selected + 1) % max;
                }
            }
        } else {
            self.next_track();
        }
    }

    /// Move the selection up one row in the active list (wraps around).
    pub fn selection_up(&mut self) {
        if self.active_tab == Tab::Library {
            if self.collection_view == CollectionView::Albums {
                let max = self.fav_albums_len();
                if max > 0 {
                    self.fav_album_selected = if self.fav_album_selected == 0 {
                        max - 1
                    } else {
                        self.fav_album_selected - 1
                    };
                }
            } else {
                let max = self.library_len();
                if max > 0 {
                    self.library_selected = if self.library_selected == 0 {
                        max - 1
                    } else {
                        self.library_selected - 1
                    };
                }
            }
        } else {
            self.prev_track();
        }
    }

    // ── Fuzzy filter (InputMode::Filter) ─────────────────────────────────

    /// Rebuild the filtered view of the active list from `filter_input`.
    pub fn filter_recompute(&mut self) {
        let haystack: Vec<String> = match self.active_tab {
            Tab::Search => self.search_results.iter().map(track_haystack).collect(),
            Tab::Queue => self.queue.iter().map(track_haystack).collect(),
            Tab::Library => match self.collection_view {
                CollectionView::Albums => self
                    .fav_albums
                    .iter()
                    .map(|a| format!("{} {}", a.title, a.artist_names()))
                    .collect(),
                CollectionView::Tracks => self
                    .playlists
                    .iter()
                    .map(|p| p.title.clone())
                    .chain(self.mixes.iter().map(|m| {
                        format!("{} {}", m.title, m.sub_title.as_deref().unwrap_or(""))
                    }))
                    .collect(),
            },
            Tab::Now | Tab::Visualizer | Tab::Effects => {
                self.filter_matches = None;
                return;
            }
        };
        self.filter_matches = Some(fuzzy::rank(
            &self.filter_input,
            haystack.iter().map(String::as_str),
        ));
        *self.selected_mut() = 0;
        self.library_selected = 0;
        self.fav_album_selected = 0;
    }

    pub fn filter_clear(&mut self) {
        self.filter_input.clear();
        self.filter_matches = None;
    }

    /// Map a visible row to its index in the underlying (unfiltered) list.
    pub fn resolve_index(&self, visible: usize) -> Option<usize> {
        self.filter_matches
            .as_ref()
            .map_or(Some(visible), |m| m.get(visible).map(|x| x.index))
    }

    pub fn next_track(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            *self.selected_mut() = (self.selected() + 1) % len;
        }
    }

    pub fn prev_track(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            *self.selected_mut() = if self.selected() == 0 {
                len - 1
            } else {
                self.selected() - 1
            };
        }
    }

    pub fn load_fav_tracks_bg(&mut self) {
        if !self.authenticated {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_fav_tracks.to_string();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.get_favorite_tracks().await {
                Ok(t) => {
                    let _ = tx.send(AppEvent::FavTracksLoaded(t));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        error: e.to_string(),
                        generation: 0,
                    });
                }
            }
        });
    }

    pub fn load_fav_albums_bg(&mut self) {
        if !self.authenticated {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_fav_albums.to_string();
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.get_favorite_albums().await {
                Ok(a) => {
                    let _ = tx.send(AppEvent::FavAlbumsLoaded(a));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        error: e.to_string(),
                        generation: 0,
                    });
                }
            }
        });
    }

    pub fn api_status_snapshot(&self) -> ApiStatus {
        use crate::player::PlayerState;
        let state = match self.player.state {
            PlayerState::Playing => "playing",
            PlayerState::Paused => "paused",
            PlayerState::Stopped => "stopped",
        }
        .to_string();
        let (title, artist, album, duration, bit_depth, sample_rate, codec) = self
            .player
            .current
            .as_ref()
            .map(|ti| {
                (
                    Some(ti.title.clone()),
                    Some(ti.artist.clone()),
                    Some(ti.album.clone()),
                    Some(ti.duration),
                    Some(ti.bit_depth),
                    Some(ti.sample_rate),
                    Some(ti.codec.clone()),
                )
            })
            .unwrap_or_default();
        ApiStatus {
            state,
            title,
            artist,
            album,
            duration,
            elapsed: self.player.elapsed.as_secs(),
            volume: self.player.volume,
            progress: self.player.progress(),
            bit_depth,
            sample_rate,
            codec,
            track_id: self.current_track_id,
            shuffle: self.shuffle,
            repeat: self.repeat.clone(),
            authenticated: self.authenticated,
            queue: self.queue.clone(),
            queue_index: self.queue_index,
            cover_url: self.cover_info.as_ref().map(|c| c.url.clone()),
        }
    }

    pub fn load_album_tracks_bg(&mut self, album_id: u64, album_title: String) {
        self.loading = true;
        self.status_msg = self.lang.loading_album(&album_title);
        let tx = self.tx();
        let tidal = self.tidal.clone();
        tokio::spawn(async move {
            match tidal.get_album_tracks(album_id).await {
                Ok(tracks) => {
                    let _ = tx.send(AppEvent::PlaylistTracksLoaded(tracks));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::StreamError {
                        error: e.to_string(),
                        generation: 0,
                    });
                }
            }
        });
    }

    /// Start downloading + decoding `url` for spectrum analysis, replacing
    /// any analysis already in flight.
    fn start_analysis(&mut self, url: &str) {
        self.stop_analysis();
        let cancel = Arc::new(AtomicBool::new(false));
        self.analysis = Some(crate::spectrum::start_analysis(url.to_string(), cancel.clone()));
        self.analysis_cancel = Some(cancel);
    }

    fn stop_analysis(&mut self) {
        if let Some(c) = self.analysis_cancel.take() {
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.analysis = None;
        self.spectrum.reset();
    }

    pub fn update_spectrum(&mut self, n_bands: usize) {
        if self.active_tab != Tab::Visualizer
            || self.player.state != crate::player::PlayerState::Playing
        {
            return;
        }
        let Some(analysis) = &self.analysis else { return };
        let Ok(buf) = analysis.lock() else { return };
        self.spectrum.update(&buf, self.player.elapsed, n_bands);
    }

    // ── Audio effects ─────────────────────────────────────────────────────

    /// Move the Effects tab cursor by `delta` rows (wrapping).
    pub fn fx_move(&mut self, delta: i32) {
        let n = FxControl::ALL.len() as i32;
        self.fx_cursor = ((self.fx_cursor as i32 + delta).rem_euclid(n)) as usize;
    }

    /// Cycle the equalizer preset regardless of the cursor position.
    pub fn fx_cycle_preset(&mut self, dir: i32) {
        let len = PRESETS.len() as i32;
        let next = match self.audio_fx.matching_preset() {
            Some(i) => (i as i32 + dir).rem_euclid(len) as usize,
            None => {
                if dir >= 0 {
                    0
                } else {
                    (len - 1) as usize
                }
            }
        };
        self.audio_fx.apply_preset(next);
        self.apply_audio_fx();
    }

    fn fx_toggle_bool(&mut self, c: FxControl) {
        match c {
            FxControl::Enabled => self.audio_fx.enabled = !self.audio_fx.enabled,
            FxControl::Crossfeed => self.audio_fx.crossfeed = !self.audio_fx.crossfeed,
            FxControl::Echo => self.audio_fx.echo = !self.audio_fx.echo,
            FxControl::Compressor => self.audio_fx.compressor = !self.audio_fx.compressor,
            FxControl::Normalize => self.audio_fx.normalize = !self.audio_fx.normalize,
            _ => {}
        }
    }

    /// Adjust the control under the cursor by `dir` steps.
    pub fn fx_adjust(&mut self, dir: i32) {
        match FxControl::ALL[self.fx_cursor] {
            c @ (FxControl::Enabled
            | FxControl::Crossfeed
            | FxControl::Echo
            | FxControl::Compressor
            | FxControl::Normalize) => {
                let _ = dir;
                self.fx_toggle_bool(c);
            }
            FxControl::Preset => {
                self.fx_cycle_preset(dir);
                return; // fx_cycle_preset already applies
            }
            FxControl::Balance => {
                self.audio_fx.balance = (self.audio_fx.balance + 5 * dir as i8).clamp(-100, 100)
            }
            FxControl::Bass => {
                self.audio_fx.bass = (self.audio_fx.bass + dir as i8).clamp(-15, 15)
            }
            FxControl::Treble => {
                self.audio_fx.treble = (self.audio_fx.treble + dir as i8).clamp(-15, 15)
            }
            FxControl::Eq(i) => {
                self.audio_fx.eq[i] = (self.audio_fx.eq[i] + dir as i8).clamp(-12, 12)
            }
            FxControl::StereoWidth => {
                self.audio_fx.stereo_width =
                    (self.audio_fx.stereo_width as i32 + 10 * dir).clamp(0, 200) as u8
            }
        }
        self.apply_audio_fx();
    }

    /// Enter on the Effects tab: toggle booleans, cycle presets, or reset the
    /// slider under the cursor to its neutral value.
    pub fn fx_toggle(&mut self) {
        match FxControl::ALL[self.fx_cursor] {
            c @ (FxControl::Enabled
            | FxControl::Crossfeed
            | FxControl::Echo
            | FxControl::Compressor
            | FxControl::Normalize) => self.fx_toggle_bool(c),
            FxControl::Preset => {
                self.fx_cycle_preset(1);
                return;
            }
            FxControl::Balance => self.audio_fx.balance = 0,
            FxControl::Bass => self.audio_fx.bass = 0,
            FxControl::Treble => self.audio_fx.treble = 0,
            FxControl::Eq(i) => self.audio_fx.eq[i] = 0,
            FxControl::StereoWidth => self.audio_fx.stereo_width = 100,
        }
        self.apply_audio_fx();
    }

    pub fn fx_reset_all(&mut self) {
        self.audio_fx.reset();
        self.apply_audio_fx();
    }

    /// Push the current filter chain to mpv and persist it.
    fn apply_audio_fx(&mut self) {
        let af = self.audio_fx.to_mpv_af();
        let n = if af.is_empty() {
            0
        } else {
            self.audio_fx.active_filters().len()
        };
        self.player.set_audio_filters(&af);
        self.status_msg = self.lang.fx_status(n);
        self.save_settings();
    }

    // ── Spectrum background (kitty / Sixel / iTerm2 graphics) ─────────────

    pub fn bg_mode(&self) -> BgMode {
        match self.picker.as_ref().map(|p| p.protocol_type()) {
            Some(ratatui_image::picker::ProtocolType::Kitty) => BgMode::Kitty,
            Some(ratatui_image::picker::ProtocolType::Sixel)
            | Some(ratatui_image::picker::ProtocolType::Iterm2) => BgMode::Composite,
            _ => BgMode::Unsupported,
        }
    }

    /// Latest composite frame ready to render (Composite mode only).
    pub fn composite_frame(&self) -> Option<&Protocol> {
        self.bg_state.composite_proto.as_ref()
    }

    /// Cycle the background source (key `b` on the spectrum tab).
    pub fn bg_cycle_source(&mut self) {
        if self.bg_mode() == BgMode::Unsupported {
            self.status_msg = self.lang.strings().bg_requires_graphics.to_string();
            return;
        }
        self.spectrum_bg = self.spectrum_bg.cycle();
        self.status_msg = self
            .lang
            .bg_source_changed(self.lang.bg_label(self.spectrum_bg));
        if self.spectrum_bg == SpectrumBg::Wallhaven && self.wallhaven_image.is_none() {
            self.fetch_wallhaven_bg();
        }
        self.save_settings();
    }

    /// Toggle the spectrum rendering style (key `v` on the Visualizer tab).
    pub fn cycle_spectrum_style(&mut self) {
        self.spectrum_style = match self.spectrum_style {
            SpectrumStyle::Curve => SpectrumStyle::Bars,
            SpectrumStyle::Bars => SpectrumStyle::Curve,
        };
        self.status_msg = self
            .lang
            .spectrum_style_changed(self.lang.spectrum_style_label(self.spectrum_style));
        self.save_settings();
    }

    /// Download a random wallpaper from wallhaven.cc in the background.
    pub fn fetch_wallhaven_bg(&mut self) {
        self.status_msg = self.lang.strings().bg_fetching.to_string();
        let tx = self.tx();
        let query = self.wallhaven_query.clone();
        tokio::spawn(async move {
            let result: anyhow::Result<DynamicImage> = async {
                let client = reqwest::Client::new();
                let api = reqwest::Url::parse_with_params(
                    "https://wallhaven.cc/api/v1/search",
                    &[
                        ("q", query.as_str()),
                        ("categories", "100"),
                        ("purity", "100"),
                        ("sorting", "random"),
                        ("atleast", "1280x720"),
                    ],
                )?;
                let json: serde_json::Value = client
                    .get(api)
                    .header("User-Agent", "tuidal")
                    .send()
                    .await?
                    .json()
                    .await?;
                let path = json["data"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|d| d["path"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("wallhaven: empty result"))?
                    .to_string();
                let bytes = client
                    .get(&path)
                    .header("User-Agent", "tuidal")
                    .send()
                    .await?
                    .bytes()
                    .await?;
                Ok(image::load_from_memory(&bytes)?)
            }
            .await;
            let _ = match result {
                Ok(img) => tx.send(AppEvent::WallhavenReady(img)),
                Err(e) => tx.send(AppEvent::WallhavenError(e.to_string())),
            };
        });
    }

    /// Called right after each `terminal.draw`: returns the escape sequence
    /// to write on stdout (kitty mode), or None when nothing must be emitted.
    /// In Composite mode the frames go through `composite_proto` and the ui
    /// renders them as a ratatui-image widget, so this always returns None.
    pub fn bg_tick(&mut self) -> Option<String> {
        let mode = self.bg_mode();
        if mode == BgMode::Unsupported {
            if !self.bg_state.logged_unsupported {
                self.bg_state.logged_unsupported = true;
                kitty_bg::bg_log("no graphics protocol detected — background disabled");
            }
            return None;
        }
        let source_img = match self.spectrum_bg {
            SpectrumBg::Cover => self.cover_image.as_ref(),
            SpectrumBg::Wallhaven => self.wallhaven_image.as_ref(),
            SpectrumBg::None => None,
        };
        let active = self.active_tab == Tab::Visualizer
            && self.spectrum_area.is_some()
            && self.spectrum_bg != SpectrumBg::None;
        if !active {
            match mode {
                BgMode::Kitty => {
                    if self.bg_state.placed_area.is_some()
                        || self.bg_state.transmitted_key.is_some()
                    {
                        kitty_bg::bg_log(&format!(
                            "inactive: delete tab={:?} area={:?} bg={:?}",
                            self.active_tab, self.spectrum_area, self.spectrum_bg
                        ));
                        self.bg_state = BgState::default();
                        return Some(kitty_bg::delete_sequence());
                    }
                }
                BgMode::Composite => {
                    self.bg_state = BgState::default();
                }
                BgMode::Unsupported => {}
            }
            return None;
        }
        // Fall back to the built-in gradient so the pipeline is exercised
        // (and something displays) even before any real image arrives.
        let area = self.spectrum_area.unwrap();
        let mut img = source_img.unwrap_or_else(|| kitty_bg::default_background());
        let (fw, fh) = self
            .picker
            .as_ref()
            .map(|p| p.font_size())
            .unwrap_or((10, 20));
        let px_w = area.width as u32 * fw as u32;
        let px_h = area.height as u32 * fh as u32;
        // u64::MAX tags the built-in fallback; the real image replaces it
        // with a different key as soon as it arrives.
        let tag = if source_img.is_none() {
            u64::MAX
        } else {
            match self.spectrum_bg {
                SpectrumBg::Cover => self.current_track_id.unwrap_or(0),
                SpectrumBg::Wallhaven => self.wallhaven_seq,
                SpectrumBg::None => 0,
            }
        };
        let mut key = BgKey {
            source: self.spectrum_bg,
            image_tag: tag,
            px_w,
            px_h,
        };
        // A key that already failed to encode falls back to the built-in
        // gradient so a background still shows instead of an error loop.
        if self.bg_state.failed_key.as_ref() == Some(&key) {
            img = kitty_bg::default_background();
            key.image_tag = u64::MAX;
        }

        // Album covers read much better as a sharp center over a blurred
        // backdrop; photos fill the area with a plain cover-crop.
        let layout = match self.spectrum_bg {
            SpectrumBg::Cover => BgLayout::FitHeight,
            _ => BgLayout::Cover,
        };

        if mode == BgMode::Composite {
            // Compose the background off-thread when the key changed.
            if self.bg_state.composed.as_ref().map(|(k, _)| k) != Some(&key)
                && self.bg_state.pending_key.as_ref() != Some(&key)
            {
                kitty_bg::bg_log(&format!(
                    "compose: key tag={} {}x{} area={:?} source_img={}",
                    key.image_tag, px_w, px_h, area, source_img.is_some()
                ));
                self.bg_state.pending_key = Some(key.clone());
                let key_task = key.clone();
                let img = img.clone();
                let tx = self.tx();
                tokio::task::spawn_blocking(move || {
                    let image = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        kitty_bg::compose(&img, px_w, px_h, layout)
                    }))
                    .ok()
                    .map(Arc::new);
                    let _ = tx.send(AppEvent::BackgroundComposed {
                        key: key_task,
                        image,
                    });
                });
            }

            // Draw the spectrum onto the cached background and encode at most
            // one frame at a time; throughput follows the encode cost.
            let frame_bg = self
                .bg_state
                .composed
                .as_ref()
                .and_then(|(k, bg)| if *k == key { Some(bg.clone()) } else { None });
            if let (Some(bg), false, false) = (
                frame_bg,
                self.bg_state.frame_in_flight,
                self.spectrum.bands.is_empty(),
            ) {
                self.bg_state.frame_in_flight = true;
                let bands = self.spectrum.bands.clone();
                let peaks = self.spectrum.peaks.clone();
                let style = self.spectrum_style;
                let picker = self.picker;
                let key_task = key.clone();
                let tx = self.tx();
                tokio::task::spawn_blocking(move || {
                    let proto = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let frame = DynamicImage::ImageRgba8(match style {
                            SpectrumStyle::Bars => {
                                composite_bg::draw_spectrum_bars(&bg, &bands, &peaks)
                            }
                            SpectrumStyle::Curve => composite_bg::draw_spectrum(&bg, &bands),
                        });
                        picker.and_then(|p| {
                            p.new_protocol(frame, area, ratatui_image::Resize::Fit(None))
                                .ok()
                        })
                    }))
                    .ok()
                    .flatten();
                    let _ = tx.send(AppEvent::SpectrumFrameEncoded {
                        key: key_task,
                        area,
                        proto,
                    });
                });
            }
            return None;
        }

        // Encode off-thread when the key changed (new image or new size).
        if self.bg_state.transmitted_key.as_ref() != Some(&key)
            && self.bg_state.pending_key.as_ref() != Some(&key)
        {
            kitty_bg::bg_log(&format!(
                "encode: key tag={} {}x{} area={:?} source_img={}",
                key.image_tag, px_w, px_h, area, source_img.is_some()
            ));
            self.bg_state.pending_key = Some(key.clone());
            let key_task = key.clone();
            let img = img.clone();
            let tx = self.tx();
            tokio::task::spawn_blocking(move || {
                let sequence = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    kitty_bg::transmit_sequence(&img, px_w, px_h, layout)
                }))
                .map(|r| r.unwrap_or_default())
                .unwrap_or_default();
                let _ = tx.send(AppEvent::BackgroundEncoded {
                    key: key_task,
                    sequence,
                });
            });
        }

        // Freshly encoded image: transmit (after deleting the previous one)
        // then place it in the chart area.
        if !self.bg_state.sequence_to_write.is_empty() {
            kitty_bg::bg_log(&format!(
                "transmit+place len={} area={area:?}",
                self.bg_state.sequence_to_write.len()
            ));
            let mut out = std::mem::take(&mut self.bg_state.sequence_to_write);
            out.push_str(&kitty_bg::place_sequence(area));
            self.bg_state.placed_area = Some(area);
            self.bg_state.frame_counter = 0;
            return Some(out);
        }

        // Re-place every frame when the area moved, and every ~10 frames
        // anyway (ratatui clears the screen on resize).
        if self.bg_state.transmitted_key.as_ref() == Some(&key) {
            self.bg_state.frame_counter = self.bg_state.frame_counter.wrapping_add(1);
            if self.bg_state.placed_area != Some(area) || self.bg_state.frame_counter >= 10 {
                kitty_bg::bg_log(&format!("place: area={area:?}"));
                self.bg_state.placed_area = Some(area);
                self.bg_state.frame_counter = 0;
                return Some(kitty_bg::place_sequence(area));
            }
        }
        None
    }

    fn save_settings(&self) {
        let settings = Settings {
            lang: self.lang,
            quality: self.quality,
            volume: self.player.volume,
            audio_fx: self.audio_fx.clone(),
            spectrum_bg: self.spectrum_bg,
            spectrum_style: self.spectrum_style,
            wallhaven_query: self.wallhaven_query.clone(),
            search_limit: self.search_limit,
            layout: self.layout,
        };
        let path = settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn load_settings(&mut self) {
        let path = settings_path();
        if let Ok(json) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<Settings>(&json) {
                self.lang = settings.lang;
                self.quality = settings.quality;
                self.player.volume = settings.volume.min(100);
                self.audio_fx = settings.audio_fx;
                // Nothing is playing yet: store the chain for the next play()
                // instead of pushing it over IPC.
                self.player.af = self.audio_fx.to_mpv_af();
                self.spectrum_bg = settings.spectrum_bg;
                self.spectrum_style = settings.spectrum_style;
                self.wallhaven_query = settings.wallhaven_query;
                self.search_limit = settings.search_limit;
                self.layout = settings.layout;
            }
        }
    }

    pub fn volume_up(&mut self) {
        self.player.volume_up();
        self.save_settings();
    }

    pub fn volume_down(&mut self) {
        self.player.volume_down();
        self.save_settings();
    }

    pub fn set_volume(&mut self, v: u8) {
        self.player.set_volume(v);
        self.save_settings();
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    lang: Lang,
    quality: Quality,
    volume: u8,
    #[serde(default)]
    audio_fx: AudioFx,
    #[serde(default = "default_spectrum_bg")]
    spectrum_bg: SpectrumBg,
    #[serde(default)]
    spectrum_style: SpectrumStyle,
    #[serde(default = "default_wallhaven_query")]
    wallhaven_query: String,
    #[serde(default = "default_search_limit")]
    search_limit: usize,
    #[serde(default)]
    layout: LayoutMode,
}

fn default_spectrum_bg() -> SpectrumBg {
    SpectrumBg::Cover
}

fn default_wallhaven_query() -> String {
    "abstract".to_string()
}

fn default_search_limit() -> usize {
    50
}

fn settings_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tuidal")
        .join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Old settings.json files have no audio_fx key: it must default cleanly.
    #[test]
    fn settings_without_audio_fx_defaults() {
        let s: Settings = serde_json::from_str(
            r#"{"lang":"fr","quality":"lossless","volume":80}"#,
        )
        .unwrap();
        assert_eq!(s.audio_fx, AudioFx::default());
    }
}

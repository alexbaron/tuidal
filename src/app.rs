use crate::i18n::Lang;
use crate::library::{LibraryFocus, LibrarySection, LibraryState};
use crate::player::Player;
use crate::settings::Settings;
use crate::tasks;
use crate::tidal::{
    Album, Artist, CoverInfo, FavAlbum, Lyrics, Mix, Playlist, Quality, SearchResults, StreamInfo,
    TidalDaemonClient, Track,
};
use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use serde::Deserialize as DeserializeAttr;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub use crate::app::search::{SearchFocus, SearchSection, SearchState};

pub mod events;
pub mod library_logic;
pub mod queue;
pub mod search;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Search,
    Queue,
    Now,
    Library,
}

pub enum AppEvent {
    SearchDone(Result<SearchResults, String>, u64),
    SearchMoreDone(Result<SearchResults, String>, u64, SearchSection),
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
    pub queue: Arc<Vec<Track>>,
    pub queue_index: Option<usize>,
}

pub struct App {
    pub tidal: Arc<TidalDaemonClient>,
    pub player: Player,
    pub input_mode: InputMode,
    pub active_tab: Tab,

    pub search: SearchState,
    pub queue: Arc<Vec<Track>>,

    pub selected: usize,
    pub queue_index: Option<usize>,

    pub authenticated: bool,
    pub status_msg: String,
    pub loading: bool,
    pub auto_advance: bool,
    pub should_quit: bool,
    pub show_help: bool,

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
    pub search_generation: u64,

    pub library: LibraryState,

    pub lang: Lang,
    pub quality: Quality,
    pub shuffle: bool,
    pub repeat: RepeatMode,

    pub lyrics: Option<Lyrics>,
}

impl App {
    pub fn new(tidal: Arc<TidalDaemonClient>) -> Self {
        Self {
            tidal,
            player: Player::new(),
            input_mode: InputMode::Normal,
            active_tab: Tab::Search,
            search: SearchState::new(),
            queue: Arc::new(Vec::new()),
            selected: 0,
            queue_index: None,
            authenticated: false,
            status_msg: String::new(),
            loading: false,
            auto_advance: false,
            should_quit: false,
            show_help: false,
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
            search_generation: 0,
            library: LibraryState::new(),

            lang: Lang::Es,
            quality: Quality::Lossless,
            shuffle: false,
            repeat: RepeatMode::All,
            lyrics: None,
        }
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        if self.active_tab == tab {
            return;
        }

        let entering_library = tab == Tab::Library;

        self.active_tab = tab;

        if entering_library {
            self.library.active_section = LibrarySection::Playlists;
            self.library.focus = LibraryFocus::Sidebar;
            self.ensure_section_loaded();
        }
        self.selected = 0;
    }

    pub fn cycle_lang(&mut self) {
        self.lang = self.lang.cycle();
        self.status_msg = self.lang.lang_changed();
        self.save_settings();
    }

    fn tx(&self) -> UnboundedSender<AppEvent> {
        self.event_tx.clone().expect("event_tx no inicializado")
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SearchDone(res, r#gen) => self.handle_search_done(res, r#gen),
            AppEvent::SearchMoreDone(res, r#gen, section) => {
                self.handle_search_more_done(res, r#gen, section)
            }
            AppEvent::StreamReady {
                track,
                info,
                queue_index,
                generation,
            } => self.handle_stream_ready(track, info, queue_index, generation),
            AppEvent::StreamError { error, generation } => {
                self.handle_stream_error(error, generation)
            }
            AppEvent::AuthStarted {
                url,
                code,
                device_code,
            } => self.handle_auth_started(url, code, device_code),
            AppEvent::AuthDone => self.handle_auth_done(),
            AppEvent::AuthError(e) => self.handle_auth_error(e),
            AppEvent::StatusMsg(msg) => self.handle_status_msg(msg),
            AppEvent::CoverReady { info, image } => self.handle_cover_ready(info, image),
            AppEvent::CoverError => self.handle_cover_error(),
            AppEvent::LyricsReady(lyrics) => self.handle_lyrics_ready(lyrics),
            AppEvent::LyricsError => self.handle_lyrics_error(),
            AppEvent::LibraryLoaded { playlists, mixes } => {
                self.handle_library_loaded(playlists, mixes)
            }
            AppEvent::PlaylistTracksLoaded(tracks) => self.handle_playlist_tracks_loaded(tracks),
            AppEvent::FavTracksLoaded(tracks) => self.handle_fav_tracks_loaded(tracks),
            AppEvent::FavAlbumsLoaded(albums) => self.handle_fav_albums_loaded(albums),
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
                        Arc::make_mut(&mut self.queue).push(track.clone());
                    }
                    let qi = self
                        .queue
                        .iter()
                        .position(|t| t.id == track.id)
                        .unwrap_or(0);
                    self.stream_track_bg(track, qi);
                }
            },
            _ => {}
        }
    }

    // ── Background tasks (simplificados — usan Arc<TidalDaemonClient>) ──────

    pub fn do_search_bg(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required.to_string();
            return;
        }
        if self.search.input.is_empty() {
            return;
        }
        let query = self.search.input.clone();
        self.search.record_history(&query);
        self.search.loaded = [0; 4];
        self.search.exhausted = [false; 4];
        self.search_generation += 1;
        let generation = self.search_generation;
        self.loading = true;
        self.status_msg = self.lang.searching(&self.search.input);
        tasks::search(
            self.tidal.clone(),
            self.tx(),
            self.search.input.clone(),
            10,
            0,
            generation,
            None,
        );
    }

    /// Fetches the next page for the active search section and appends it.
    /// Only valid when browsing search content (not the sidebar/drill-down).
    pub fn load_more_bg(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required_short.to_string();
            return;
        }
        if self.active_tab != Tab::Search
            || self.search.viewing.is_some()
            || self.search.focus != SearchFocus::Content
        {
            return;
        }
        if self.search.input.is_empty() {
            return;
        }
        let section = self.search.active_section;
        let idx = section as usize;
        if self.search.exhausted[idx] {
            self.status_msg = self.lang.strings().search_no_more_results.to_string();
            return;
        }
        let offset = self.search.loaded[idx];
        if offset >= 300 {
            self.search.exhausted[idx] = true;
            self.status_msg = self.lang.strings().search_no_more_results.to_string();
            return;
        }
        let query = self.search.input.clone();
        self.search_generation += 1;
        let generation = self.search_generation;
        self.loading = true;
        self.status_msg = self.lang.searching_more(&query);
        tasks::search(
            self.tidal.clone(),
            self.tx(),
            query,
            10,
            offset,
            generation,
            Some(section),
        );
    }

    fn load_lyrics_bg(&mut self, track_id: u64) {
        tasks::lyrics(self.tidal.clone(), self.tx(), track_id);
    }

    fn load_cover_bg(&mut self, track_id: u64) {
        tasks::cover(self.tidal.clone(), self.tx(), track_id);
    }

    pub fn start_login_bg(&mut self) {
        self.loading = true;
        self.status_msg = self.lang.strings().status_starting_login.to_string();
        tasks::start_auth(self.tidal.clone(), self.tx());
    }

    pub fn poll_auth_bg(&mut self) {
        tasks::poll_auth(self.tidal.clone(), self.tx());
    }

    pub fn set_quality(&mut self, q: Quality) {
        self.quality = q;
        self.status_msg = self.lang.quality_changed(q.label());
        tasks::set_quality(self.tidal.clone(), q.as_api_str().to_string());
        self.save_settings();
    }

    pub fn next_tab(&mut self) {
        let entering_library = self.active_tab == Tab::Now;
        self.active_tab = match self.active_tab {
            Tab::Search => Tab::Queue,
            Tab::Queue => Tab::Now,
            Tab::Now => Tab::Library,
            Tab::Library => {
                self.library.active_section = LibrarySection::Playlists;
                self.library.focus = LibraryFocus::Sidebar;
                Tab::Search
            }
        };
        if entering_library {
            self.ensure_section_loaded();
        }
        self.selected = 0;
    }

    pub fn current_list_len(&self) -> usize {
        match self.active_tab {
            Tab::Search => {
                if self.search.viewing.is_some() {
                    self.search.viewing.as_ref().map_or(0, |v| v.tracks.len())
                } else {
                    self.search.current_len()
                }
            }
            Tab::Queue => self.queue.len(),
            Tab::Now => 0,
            Tab::Library => self.section_len(self.library.active_section),
        }
    }

    pub fn next_track(&mut self) {
        if self.active_tab == Tab::Search {
            if let Some(ref mut v) = self.search.viewing {
                let max = v.tracks.len();
                if max > 0 {
                    v.cursor = (v.cursor + 1) % max;
                }
            } else if self.search.focus == SearchFocus::Sidebar {
                self.search.active_section = self.search.active_section.next();
            } else {
                let section = self.search.active_section as usize;
                let max = self.search.current_len();
                if max > 0 {
                    self.search.cursor[section] = (self.search.cursor[section] + 1) % max;
                }
            }
            return;
        }
        let len = self.current_list_len();
        if len > 0 {
            self.selected = (self.selected + 1) % len;
        }
    }

    pub fn prev_track(&mut self) {
        if self.active_tab == Tab::Search {
            if let Some(ref mut v) = self.search.viewing {
                let max = v.tracks.len();
                if max > 0 {
                    v.cursor = if v.cursor == 0 { max - 1 } else { v.cursor - 1 };
                }
            } else if self.search.focus == SearchFocus::Sidebar {
                self.search.active_section = self.search.active_section.prev();
            } else {
                let section = self.search.active_section as usize;
                let max = self.search.current_len();
                if max > 0 {
                    self.search.cursor[section] = if self.search.cursor[section] == 0 {
                        max - 1
                    } else {
                        self.search.cursor[section] - 1
                    };
                }
            }
            return;
        }
        let len = self.current_list_len();
        if len > 0 {
            self.selected = if self.selected == 0 {
                len - 1
            } else {
                self.selected - 1
            };
        }
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
        }
    }

    fn save_settings(&self) {
        let settings = Settings {
            lang: self.lang,
            quality: self.quality,
            volume: self.player.volume,
        };
        settings.save();
    }

    pub fn load_settings(&mut self) {
        if let Some(settings) = Settings::load() {
            self.lang = settings.lang;
            self.quality = settings.quality;
            self.player.volume = settings.volume.min(100);
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

    pub fn clear_search(&mut self) {
        if self.active_tab != Tab::Search {
            return;
        }
        self.search = SearchState::new();
        self.input_mode = InputMode::Normal;
        self.status_msg = self.lang.strings().status_search_cleared.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(id: u64, title: &str) -> Track {
        Track {
            id,
            title: title.to_string(),
            duration: 200,
            track_number: None,
            artists: vec![],
            album: Album {
                id: 0,
                title: String::new(),
            },
            audio_quality: None,
            explicit: None,
        }
    }

    #[test]
    fn test_remove_queue_index_after_current() {
        let mut queue: Arc<Vec<Track>> = Arc::new(vec![
            make_track(1, "A"),
            make_track(2, "B"),
            make_track(3, "C"),
        ]);
        let mut queue_index = Some(1);
        let removed = App::remove_queue_index(&mut queue, &mut queue_index, 2);
        assert_eq!(removed.unwrap().id, 3);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue_index, Some(1));
    }

    #[test]
    fn test_remove_queue_index_before_current() {
        let mut queue: Arc<Vec<Track>> = Arc::new(vec![
            make_track(1, "A"),
            make_track(2, "B"),
            make_track(3, "C"),
        ]);
        let mut queue_index = Some(2);
        let removed = App::remove_queue_index(&mut queue, &mut queue_index, 0);
        assert_eq!(removed.unwrap().id, 1);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue_index, Some(1));
    }

    #[test]
    fn test_remove_queue_index_current_is_first() {
        let mut queue: Arc<Vec<Track>> = Arc::new(vec![make_track(1, "A"), make_track(2, "B")]);
        let mut queue_index = Some(0);
        let removed = App::remove_queue_index(&mut queue, &mut queue_index, 0);
        assert_eq!(removed.unwrap().id, 1);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue_index, Some(0));
    }

    #[test]
    fn test_remove_queue_index_out_of_bounds() {
        let mut queue: Arc<Vec<Track>> = Arc::new(vec![make_track(1, "A")]);
        let mut queue_index = Some(0);
        let removed = App::remove_queue_index(&mut queue, &mut queue_index, 5);
        assert!(removed.is_none());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue_index, Some(0));
    }

    #[test]
    fn test_remove_queue_index_empty_queue() {
        let mut queue: Arc<Vec<Track>> = Arc::new(vec![]);
        let mut queue_index = None;
        let removed = App::remove_queue_index(&mut queue, &mut queue_index, 0);
        assert!(removed.is_none());
    }

    #[test]
    fn test_search_history_caps_at_10() {
        let mut state = SearchState::new();
        for i in 0..12 {
            state.record_history(&format!("query{i}"));
        }
        assert_eq!(state.history.len(), 10);
        assert_eq!(state.history[0], "query11");
        assert_eq!(state.history[9], "query2");
        assert!(!state.history.iter().any(|h| h == "query0"));
        assert!(!state.history.iter().any(|h| h == "query1"));
    }

    #[test]
    fn test_search_history_moves_duplicate_to_front() {
        let mut state = SearchState::new();
        for i in 0..5 {
            state.record_history(&format!("query{i}"));
        }
        // history: q4, q3, q2, q1, q0 — repeating q2 moves it to front
        state.record_history("query2");
        assert_eq!(state.history.len(), 5);
        assert_eq!(state.history[0], "query2");
        assert_eq!(state.history[1], "query4");
        assert_eq!(state.history[2], "query3");
        assert_eq!(state.history[3], "query1");
        assert_eq!(state.history[4], "query0");
    }

    #[test]
    fn test_search_history_navigation_up_down() {
        let mut state = SearchState::new();
        for i in 0..3 {
            state.record_history(&format!("query{i}"));
        }
        // history: q2, q1, q0
        state.input = "draft text".to_string();
        assert_eq!(state.history_cursor, None);

        // ↑ once → most recent
        state.history_up();
        assert_eq!(state.history_cursor, Some(0));
        assert_eq!(state.input, "query2");
        assert_eq!(state.draft, "draft text");

        // ↑ again → older
        state.history_up();
        assert_eq!(state.history_cursor, Some(1));
        assert_eq!(state.input, "query1");

        // ↑ again → oldest
        state.history_up();
        assert_eq!(state.history_cursor, Some(2));
        assert_eq!(state.input, "query0");

        // ↑ at oldest → no change
        state.history_up();
        assert_eq!(state.history_cursor, Some(2));
        assert_eq!(state.input, "query0");

        // ↓ back one
        state.history_down();
        assert_eq!(state.history_cursor, Some(1));
        assert_eq!(state.input, "query1");

        // ↓ back to most recent
        state.history_down();
        assert_eq!(state.history_cursor, Some(0));
        assert_eq!(state.input, "query2");

        // ↓ at 0 → restore draft and exit navigation
        state.history_down();
        assert_eq!(state.history_cursor, None);
        assert_eq!(state.input, "draft text");
    }

    #[test]
    fn test_search_generation_discards_stale() {
        let mut app = make_app_for_test();
        app.search_generation = 5;

        // Stale result (generation 3 < 5) should be silently ignored
        let stale = SearchResults {
            tracks: vec![make_track(99, "Stale")],
            ..SearchResults::default()
        };
        app.handle_event(AppEvent::SearchDone(Ok(stale), 3));
        assert_eq!(app.search.results.tracks.len(), 0);
        assert_eq!(app.search.results.albums.len(), 0);

        // Current generation result (5) should update state
        let current = SearchResults {
            tracks: vec![make_track(1, "Current")],
            ..SearchResults::default()
        };
        app.handle_event(AppEvent::SearchDone(Ok(current), 5));
        assert_eq!(app.search.results.tracks.len(), 1);
        assert_eq!(app.search.results.tracks[0].title, "Current");

        // Stale error should also be ignored
        app.handle_event(AppEvent::SearchDone(Err("stale error".into()), 4));
        // Status message should still reflect the last successful search
        assert_eq!(app.status_msg, "1 resultados");
    }

    fn make_app_for_test() -> App {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tidal = rt.block_on(async {
            crate::tidal::TidalDaemonClient::spawn("tidal.py", "python3", "LOSSLESS")
                .await
                .expect("tidal.py must exist for this test")
        });
        App::new(tidal)
    }

    #[test]
    fn test_search_persists_across_tab_switch() {
        let mut app = make_app_for_test();
        app.search.input = "radiohead".to_string();
        app.search.results = SearchResults {
            tracks: vec![make_track(1, "Karma Police")],
            ..SearchResults::default()
        };
        app.search.active_section = SearchSection::Albums;
        app.search.cursor = [3, 7, 0, 0];
        app.search.focus = SearchFocus::Content;

        // Cycle through all tabs and back to Search
        app.next_tab(); // Queue
        app.next_tab(); // Now
        app.next_tab(); // Library
        app.next_tab(); // back to Search

        assert_eq!(app.search.input, "radiohead");
        assert_eq!(app.search.results.tracks.len(), 1);
        assert_eq!(app.search.results.tracks[0].title, "Karma Police");
        assert_eq!(app.search.active_section, SearchSection::Albums);
        assert_eq!(app.search.cursor, [3, 7, 0, 0]);
        assert_eq!(app.search.focus, SearchFocus::Content);
    }

    #[test]
    fn test_clear_search_resets_state() {
        let mut app = make_app_for_test();
        app.active_tab = Tab::Search;
        app.search.input = "radiohead".to_string();
        app.search.results = SearchResults {
            tracks: vec![make_track(1, "Karma Police")],
            ..SearchResults::default()
        };
        app.search.cursor = [3, 2, 1, 0];
        app.search.focus = SearchFocus::Content;
        app.search.active_section = SearchSection::Artists;

        app.clear_search();

        assert!(app.search.input.is_empty());
        assert_eq!(app.search.results.tracks.len(), 0);
        assert_eq!(app.search.results.albums.len(), 0);
        assert_eq!(app.search.results.artists.len(), 0);
        assert_eq!(app.search.results.playlists.len(), 0);
        assert_eq!(app.search.cursor, [0; 4]);
        assert_eq!(app.search.focus, SearchFocus::Sidebar);
        assert_eq!(app.search.active_section, SearchSection::Tracks);
        assert!(app.search.viewing.is_none());
        assert_eq!(app.status_msg, "Búsqueda limpiada");
    }

    #[test]
    fn test_search_more_appends_and_dedups() {
        let mut app = make_app_for_test();
        app.search_generation = 5;
        app.search.active_section = SearchSection::Tracks;
        app.search.loaded = [10, 0, 0, 0];
        app.search.results = SearchResults {
            tracks: vec![make_track(1, "A"), make_track(2, "B")],
            ..SearchResults::default()
        };
        // full page: duplicate "B" + 9 new tracks (ids 3..=11)
        let mut page2 = vec![make_track(2, "B")];
        for id in 3..=11 {
            page2.push(make_track(id, &format!("T{id}")));
        }
        let page2 = SearchResults {
            tracks: page2,
            ..SearchResults::default()
        };
        app.handle_event(AppEvent::SearchMoreDone(
            Ok(page2),
            5,
            SearchSection::Tracks,
        ));

        assert_eq!(app.search.results.tracks.len(), 11);
        assert_eq!(app.search.results.tracks[2].title, "T3");
        // duplicate "B" not appended → only 9 new items on top of the 10 already loaded
        assert_eq!(app.search.loaded[0], 19);
        assert!(!app.search.exhausted[0]);
    }

    #[test]
    fn test_search_more_marks_exhausted_on_short_page() {
        let mut app = make_app_for_test();
        app.search_generation = 5;
        app.search.loaded = [10, 0, 0, 0];
        let page = SearchResults {
            tracks: vec![make_track(3, "C"), make_track(4, "D"), make_track(5, "E")],
            ..SearchResults::default()
        };
        app.handle_event(AppEvent::SearchMoreDone(Ok(page), 5, SearchSection::Tracks));

        assert_eq!(app.search.results.tracks.len(), 3);
        assert_eq!(app.search.loaded[0], 13);
        assert!(app.search.exhausted[0]);
    }

    #[test]
    fn test_search_more_stale_discarded() {
        let mut app = make_app_for_test();
        app.search_generation = 5;
        app.search.results = SearchResults {
            tracks: vec![make_track(1, "A")],
            ..SearchResults::default()
        };
        app.handle_event(AppEvent::SearchMoreDone(
            Ok(SearchResults {
                tracks: vec![make_track(9, "Stale")],
                ..SearchResults::default()
            }),
            3,
            SearchSection::Tracks,
        ));
        assert_eq!(app.search.results.tracks.len(), 1);
        assert_eq!(app.search.results.tracks[0].title, "A");
        assert_eq!(app.search.loaded[0], 0);
    }

    #[test]
    fn test_search_done_sets_loaded_and_exhausted() {
        let mut app = make_app_for_test();
        app.search_generation = 1;
        let results = SearchResults {
            tracks: vec![make_track(1, "A"), make_track(2, "B")],
            artists: vec![Artist {
                id: 1,
                name: "X".to_string(),
            }],
            ..SearchResults::default()
        };
        app.handle_event(AppEvent::SearchDone(Ok(results), 1));

        assert_eq!(app.search.loaded, [2, 0, 1, 0]);
        // every section returned fewer than a full page → all exhausted
        assert_eq!(app.search.exhausted, [true, true, true, true]);
    }

    #[test]
    fn test_load_more_bg_guards() {
        let mut app = make_app_for_test();

        // not authenticated → status message, no request
        app.load_more_bg();
        assert_eq!(
            app.status_msg,
            app.lang.strings().status_login_required_short.to_string()
        );
        assert_eq!(app.search_generation, 0);

        app.authenticated = true;
        app.search.input = "radiohead".to_string();

        // sidebar focus → no-op
        app.search.focus = SearchFocus::Sidebar;
        app.status_msg.clear();
        app.load_more_bg();
        assert_eq!(app.status_msg, "");
        assert_eq!(app.search_generation, 0);

        // content focus but empty input → no-op
        app.search.focus = SearchFocus::Content;
        app.search.input.clear();
        app.status_msg.clear();
        app.load_more_bg();
        assert_eq!(app.status_msg, "");
        assert_eq!(app.search_generation, 0);

        // exhausted section → status message, no request
        app.search.input = "radiohead".to_string();
        app.search.loaded[0] = 10;
        app.search.exhausted[0] = true;
        app.load_more_bg();
        assert_eq!(
            app.status_msg,
            app.lang.strings().search_no_more_results.to_string()
        );
        assert_eq!(app.search_generation, 0);
    }

    #[test]
    fn test_clear_search_ignored_on_other_tab() {
        let mut app = make_app_for_test();
        app.active_tab = Tab::Queue;
        app.search.input = "radiohead".to_string();
        app.search.results = SearchResults {
            tracks: vec![make_track(1, "Karma Police")],
            ..SearchResults::default()
        };
        app.search.cursor = [3, 2, 1, 0];
        app.search.focus = SearchFocus::Content;
        app.search.active_section = SearchSection::Artists;

        app.clear_search();

        assert_eq!(app.search.input, "radiohead");
        assert_eq!(app.search.results.tracks.len(), 1);
        assert_eq!(app.search.cursor, [3, 2, 1, 0]);
        assert_eq!(app.search.focus, SearchFocus::Content);
        assert_eq!(app.search.active_section, SearchSection::Artists);
        assert_ne!(app.status_msg, "Búsqueda limpiada");
    }
}

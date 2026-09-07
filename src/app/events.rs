use crate::app::search::append_dedup;
use crate::app::{App, SearchFocus, SearchSection, Tab};
use crate::library::{LibrarySection, LibraryViewing};
use crate::player::TrackInfo;
use crate::tidal::{CoverInfo, FavAlbum, Lyrics, Mix, Playlist, SearchResults, Track};
use open;

impl App {
    pub fn handle_search_done(&mut self, res: Result<SearchResults, String>, generation: u64) {
        if generation != self.search_generation {
            return;
        }
        match res {
            Ok(results) => {
                let total: usize = results.tracks.len()
                    + results.albums.len()
                    + results.artists.len()
                    + results.playlists.len();
                self.status_msg = if total == 0 {
                    self.lang.strings().status_no_results.to_string()
                } else {
                    self.lang.results_count(total)
                };
                self.search.results = results;
                self.search.cursor = [0; 4];
                self.search.focus = SearchFocus::Sidebar;
                self.search.active_section = SearchSection::Tracks;
                self.search.viewing = None;
                self.search.loaded = [
                    self.search.results.tracks.len(),
                    self.search.results.albums.len(),
                    self.search.results.artists.len(),
                    self.search.results.playlists.len(),
                ];
                self.search.exhausted = [
                    self.search.results.tracks.len() < 10,
                    self.search.results.albums.len() < 10,
                    self.search.results.artists.len() < 10,
                    self.search.results.playlists.len() < 10,
                ];
                self.active_tab = Tab::Search;
                self.loading = false;
            }
            Err(e) => {
                self.status_msg = self.lang.search_error(&e);
                self.loading = false;
            }
        }
    }

    pub fn handle_search_more_done(
        &mut self,
        res: Result<SearchResults, String>,
        generation: u64,
        section: SearchSection,
    ) {
        if generation != self.search_generation {
            return;
        }
        match res {
            Ok(results) => {
                let idx = section as usize;
                let page_size = match section {
                    SearchSection::Tracks => results.tracks.len(),
                    SearchSection::Albums => results.albums.len(),
                    SearchSection::Artists => results.artists.len(),
                    SearchSection::Playlists => results.playlists.len(),
                };
                let appended = match section {
                    SearchSection::Tracks => {
                        append_dedup(&mut self.search.results.tracks, results.tracks, |t| t.id)
                    }
                    SearchSection::Albums => {
                        append_dedup(&mut self.search.results.albums, results.albums, |a| a.id)
                    }
                    SearchSection::Artists => {
                        append_dedup(&mut self.search.results.artists, results.artists, |a| a.id)
                    }
                    SearchSection::Playlists => {
                        append_dedup(&mut self.search.results.playlists, results.playlists, |p| {
                            p.uuid.clone()
                        })
                    }
                };
                self.search.loaded[idx] += appended;
                if page_size < 10 || appended == 0 {
                    self.search.exhausted[idx] = true;
                }
                self.status_msg = self.lang.results_count(self.search.len(section));
                self.loading = false;
            }
            Err(e) => {
                self.status_msg = self.lang.search_error(&e);
                self.loading = false;
            }
        }
    }

    pub fn handle_stream_ready(
        &mut self,
        track: Track,
        info: crate::tidal::StreamInfo,
        queue_index: usize,
        generation: u64,
    ) {
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
        self.loading = false;
        self.auto_advance = true;
        self.load_cover_bg(track.id);
        self.load_lyrics_bg(track.id);
    }

    pub fn handle_stream_error(&mut self, error: String, generation: u64) {
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

    pub fn handle_auth_started(&mut self, url: String, code: String, device_code: String) {
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

    pub fn handle_auth_done(&mut self) {
        self.authenticated = true;
        self.device_code = None;
        self.user_code = None;
        self.auth_url = None;
        self.status_msg = self.lang.strings().status_auth_done.to_string();
        self.loading = false;
    }

    pub fn handle_auth_error(&mut self, e: String) {
        self.status_msg = self.lang.auth_error(&e);
        self.loading = false;
    }

    pub fn handle_status_msg(&mut self, msg: String) {
        self.status_msg = msg;
    }

    pub fn handle_cover_ready(&mut self, info: CoverInfo, image: image::DynamicImage) {
        self.cover_info = Some(info);
        self.cover_image = Some(image);
        self.cover_proto = None;
        self.last_img_area = None;
    }

    pub fn handle_cover_error(&mut self) {
        self.cover_image = None;
        self.cover_proto = None;
    }

    pub fn handle_lyrics_ready(&mut self, lyrics: Lyrics) {
        self.lyrics = Some(lyrics);
    }

    pub fn handle_lyrics_error(&mut self) {
        self.lyrics = None;
    }

    pub fn handle_library_loaded(&mut self, playlists: Vec<Playlist>, mixes: Vec<Mix>) {
        self.library.playlists = Some(playlists);
        self.library.mixes = Some(mixes);
        self.active_tab = Tab::Library;
        self.loading = false;
        let plen = self.library.playlists.as_ref().map_or(0, |p| p.len());
        let mlen = self.library.mixes.as_ref().map_or(0, |m| m.len());
        self.status_msg = self.lang.library_loaded(plen, mlen);
    }

    pub fn handle_playlist_tracks_loaded(&mut self, tracks: Vec<Track>) {
        let n = tracks.len();
        if !self.search.pending_viewing_title.is_empty() {
            let title = std::mem::take(&mut self.search.pending_viewing_title);
            self.search.viewing = Some(LibraryViewing {
                title,
                tracks,
                cursor: 0,
            });
        } else {
            let title = std::mem::take(&mut self.library.pending_viewing_title);
            self.library.viewing = Some(LibraryViewing {
                title,
                tracks,
                cursor: 0,
            });
        }
        self.loading = false;
        self.status_msg = self.lang.tracks_loaded(n);
    }

    pub fn handle_fav_tracks_loaded(&mut self, tracks: Vec<Track>) {
        let n = tracks.len();
        self.library.fav_tracks = Some(tracks);
        self.loading = false;
        self.status_msg = self.lang.fav_tracks_loaded(n);
    }

    pub fn handle_fav_albums_loaded(&mut self, albums: Vec<FavAlbum>) {
        self.library.fav_albums = Some(albums);
        self.library.cursor[LibrarySection::FavAlbums as usize] = 0;
        self.library.active_section = LibrarySection::FavAlbums;
        self.active_tab = Tab::Library;
        self.loading = false;
        let count = self.library.fav_albums.as_ref().map_or(0, |a| a.len());
        self.status_msg = self.lang.fav_albums_loaded(count);
    }
}

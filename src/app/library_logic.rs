use crate::app::App;
use crate::library::LibrarySection;
use crate::tasks;
use crate::tidal::Track;
use std::sync::Arc;

impl App {
    pub fn load_library_bg(&mut self) {
        if !self.authenticated {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_lib.to_string();
        tasks::library(self.tidal.clone(), self.tx());
    }

    pub fn load_playlist_tracks_bg(&mut self, uuid: String) {
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_playlist.to_string();
        tasks::playlist_tracks(self.tidal.clone(), self.tx(), uuid);
    }

    pub fn load_mix_tracks_bg(&mut self, mix_id: String) {
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_mix.to_string();
        tasks::mix_tracks(self.tidal.clone(), self.tx(), mix_id);
    }

    pub fn library_select_enter(&mut self) {
        match self.library.active_section {
            LibrarySection::Playlists => {
                let cursor = self.library.cursor[LibrarySection::Playlists as usize];
                if let Some(playlists) = &self.library.playlists {
                    if let Some(p) = playlists.get(cursor) {
                        self.library.pending_viewing_title = p.title.clone();
                        self.load_playlist_tracks_bg(p.uuid.clone());
                    }
                }
            }
            LibrarySection::Mixes => {
                let cursor = self.library.cursor[LibrarySection::Mixes as usize];
                if let Some(mixes) = &self.library.mixes {
                    if let Some(m) = mixes.get(cursor) {
                        self.library.pending_viewing_title = m.title.clone();
                        self.load_mix_tracks_bg(m.id.clone());
                    }
                }
            }
            LibrarySection::FavTracks => {
                // ponytail: Enter on individual track handled by play_fav_track
            }
            LibrarySection::FavAlbums => {
                let cursor = self.library.cursor[LibrarySection::FavAlbums as usize];
                if let Some(albums) = &self.library.fav_albums {
                    if let Some(a) = albums.get(cursor) {
                        self.library.pending_viewing_title = a.title.clone();
                        self.load_album_tracks_bg(a.id, a.title.clone());
                    }
                }
            }
        }
    }

    pub fn play_drilldown_track(&mut self) {
        let (track, remaining) = if let Some(ref v) = self.search.viewing {
            match v.tracks.get(v.cursor) {
                Some(t) => (t.clone(), v.tracks[v.cursor..].to_vec()),
                None => return,
            }
        } else if let Some(ref v) = self.library.viewing {
            match v.tracks.get(v.cursor) {
                Some(t) => (t.clone(), v.tracks[v.cursor..].to_vec()),
                None => return,
            }
        } else {
            return;
        };
        self.queue = Arc::new(remaining);
        self.stream_track_bg(track, 0);
    }

    pub fn play_fav_track(&mut self) {
        let cursor = self.library.cursor[LibrarySection::FavTracks as usize];
        let (track, remaining) = match self.library.fav_tracks.as_ref() {
            Some(tracks) => match tracks.get(cursor) {
                Some(t) => (t.clone(), tracks[cursor..].to_vec()),
                None => return,
            },
            None => return,
        };
        self.queue = Arc::new(remaining);
        self.stream_track_bg(track, 0);
    }

    pub fn load_fav_tracks_bg(&mut self) {
        if !self.authenticated {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_fav_tracks.to_string();
        tasks::fav_tracks(self.tidal.clone(), self.tx());
    }

    pub fn load_fav_albums_bg(&mut self) {
        if !self.authenticated {
            return;
        }
        self.loading = true;
        self.status_msg = self.lang.strings().status_loading_fav_albums.to_string();
        tasks::fav_albums(self.tidal.clone(), self.tx());
    }

    pub fn load_album_tracks_bg(&mut self, album_id: u64, album_title: String) {
        self.loading = true;
        self.status_msg = self.lang.loading_album(&album_title);
        tasks::album_tracks(self.tidal.clone(), self.tx(), album_id);
    }

    pub fn ensure_section_loaded(&mut self) {
        match self.library.active_section {
            LibrarySection::Playlists | LibrarySection::Mixes => {
                if self.library.playlists.is_none() && self.library.mixes.is_none() {
                    self.load_library_bg();
                }
            }
            LibrarySection::FavTracks => {
                if self.library.fav_tracks.is_none() {
                    self.load_fav_tracks_bg();
                }
            }
            LibrarySection::FavAlbums => {
                if self.library.fav_albums.is_none() {
                    self.load_fav_albums_bg();
                }
            }
        }
    }

    pub fn section_len(&self, section: LibrarySection) -> usize {
        match section {
            LibrarySection::Playlists => self.library.playlists.as_ref().map_or(0, |p| p.len()),
            LibrarySection::Mixes => self.library.mixes.as_ref().map_or(0, |m| m.len()),
            LibrarySection::FavTracks => self.library.fav_tracks.as_ref().map_or(0, |t| t.len()),
            LibrarySection::FavAlbums => self.library.fav_albums.as_ref().map_or(0, |a| a.len()),
        }
    }

    pub fn add_current_track_to_queue(&mut self) {
        let track = if let Some(ref viewing) = self.search.viewing {
            match viewing.tracks.get(viewing.cursor) {
                Some(t) => t.clone(),
                None => return,
            }
        } else if let Some(ref viewing) = self.library.viewing {
            match viewing.tracks.get(viewing.cursor) {
                Some(t) => t.clone(),
                None => return,
            }
        } else if self.library.active_section == LibrarySection::FavTracks {
            let cursor = self.library.cursor[LibrarySection::FavTracks as usize];
            match self.library.fav_tracks.as_ref().and_then(|t| t.get(cursor)) {
                Some(t) => t.clone(),
                None => return,
            }
        } else {
            return;
        };
        if self.queue.iter().any(|t| t.id == track.id) {
            self.status_msg = self.lang.strings().status_already_in_queue.to_string();
            return;
        }
        Arc::make_mut(&mut self.queue).push(track.clone());
        self.status_msg = format!("▶ Añadida: {}", track.title);
    }

    pub fn add_all_tracks_to_queue(&mut self) {
        let tracks: Vec<Track> = if let Some(ref viewing) = self.search.viewing {
            viewing.tracks.clone()
        } else if let Some(ref viewing) = self.library.viewing {
            viewing.tracks.clone()
        } else if self.library.active_section == LibrarySection::FavTracks {
            match self.library.fav_tracks.as_ref() {
                Some(t) => t.clone(),
                None => return,
            }
        } else {
            return;
        };
        let mut added = 0usize;
        for track in &tracks {
            if !self.queue.iter().any(|t| t.id == track.id) {
                Arc::make_mut(&mut self.queue).push(track.clone());
                added += 1;
            }
        }
        self.status_msg = format!("▶ {added} canciones añadidas a la cola");
    }
}

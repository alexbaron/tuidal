use crate::tidal::SearchResults;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchFocus {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(usize)]
pub enum SearchSection {
    Tracks = 0,
    Albums = 1,
    Artists = 2,
    Playlists = 3,
}

impl SearchSection {
    pub fn next(self) -> Self {
        match self {
            SearchSection::Tracks => SearchSection::Albums,
            SearchSection::Albums => SearchSection::Artists,
            SearchSection::Artists => SearchSection::Playlists,
            SearchSection::Playlists => SearchSection::Tracks,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SearchSection::Tracks => SearchSection::Playlists,
            SearchSection::Albums => SearchSection::Tracks,
            SearchSection::Artists => SearchSection::Albums,
            SearchSection::Playlists => SearchSection::Artists,
        }
    }
}

pub struct SearchState {
    pub focus: SearchFocus,
    pub active_section: SearchSection,
    pub cursor: [usize; 4],
    pub results: SearchResults,
    pub viewing: Option<crate::library::LibraryViewing>,
    pub pending_viewing_title: String,
    pub input: String,
    pub history: std::collections::VecDeque<String>,
    pub history_cursor: Option<usize>,
    pub draft: String,
    /// Items already fetched per section — doubles as the next pagination offset.
    pub loaded: [usize; 4],
    /// True when Tidal reported no more results for that section.
    pub exhausted: [bool; 4],
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            focus: SearchFocus::Sidebar,
            active_section: SearchSection::Tracks,
            cursor: [0; 4],
            results: SearchResults::default(),
            viewing: None,
            pending_viewing_title: String::new(),
            input: String::new(),
            history: VecDeque::new(),
            history_cursor: None,
            draft: String::new(),
            loaded: [0; 4],
            exhausted: [false; 4],
        }
    }

    pub fn len(&self, section: SearchSection) -> usize {
        match section {
            SearchSection::Tracks => self.results.tracks.len(),
            SearchSection::Albums => self.results.albums.len(),
            SearchSection::Artists => self.results.artists.len(),
            SearchSection::Playlists => self.results.playlists.len(),
        }
    }

    pub fn current_len(&self) -> usize {
        self.len(self.active_section)
    }

    pub fn record_history(&mut self, query: &str) {
        if let Some(pos) = self.history.iter().position(|h| h == query) {
            self.history.remove(pos);
        }
        self.history.push_front(query.to_string());
        while self.history.len() > 10 {
            self.history.pop_back();
        }
        self.history_cursor = None;
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_cursor.is_none() {
            self.draft = self.input.clone();
            self.history_cursor = Some(0);
        } else {
            let cursor = self.history_cursor.unwrap();
            if cursor + 1 < self.history.len() {
                self.history_cursor = Some(cursor + 1);
            } else {
                return;
            }
        }
        let idx = self.history_cursor.unwrap();
        self.input.clone_from(&self.history[idx]);
    }

    pub fn history_down(&mut self) {
        match self.history_cursor {
            None => {}
            Some(0) => {
                self.history_cursor = None;
                self.input = std::mem::take(&mut self.draft);
            }
            Some(cursor) => {
                let prev = cursor - 1;
                self.history_cursor = Some(prev);
                self.input.clone_from(&self.history[prev]);
            }
        }
    }
}

/// Appends items from `src` that aren't already present in `dst` (keyed by `key`).
/// Returns the number of items actually appended.
pub(crate) fn append_dedup<T, K: Eq>(
    dst: &mut Vec<T>,
    src: Vec<T>,
    key: impl Fn(&T) -> K,
) -> usize {
    let before = dst.len();
    for item in src {
        if !dst.iter().any(|existing| key(existing) == key(&item)) {
            dst.push(item);
        }
    }
    dst.len() - before
}

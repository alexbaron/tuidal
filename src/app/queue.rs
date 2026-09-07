use crate::app::App;
use crate::app::Tab;
use crate::app::search::SearchSection;
use crate::tasks;
use crate::tidal::Track;
use std::sync::Arc;

impl App {
    pub fn add_selected_to_queue(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required_short.to_string();
            return;
        }
        let track = match self.active_tab {
            Tab::Search => {
                if self.search.active_section != SearchSection::Tracks
                    || self.search.viewing.is_some()
                {
                    return;
                }
                self.search
                    .results
                    .tracks
                    .get(self.search.cursor[SearchSection::Tracks as usize])
                    .cloned()
            }
            Tab::Queue => self.queue.get(self.selected).cloned(),
            Tab::Now | Tab::Library => return,
        };
        let Some(track) = track else { return };
        if self.queue.iter().any(|t| t.id == track.id) {
            self.status_msg = self.lang.strings().status_already_in_queue.to_string();
            return;
        }
        Arc::make_mut(&mut self.queue).push(track);
        self.status_msg = self.lang.strings().status_added_to_queue.to_string();
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        let playing_was_removed = self.queue_index == Some(index);
        let removed = Self::remove_queue_index(&mut self.queue, &mut self.queue_index, index);
        let Some(removed) = removed else { return };
        if playing_was_removed {
            if index < self.queue.len() {
                self.stream_track_bg(self.queue[index].clone(), index);
            } else if !self.queue.is_empty() {
                self.stream_track_bg(self.queue[0].clone(), 0);
            } else {
                self.player.stop();
                self.queue_index = None;
            }
        }
        self.status_msg = format!("Eliminada: {}", removed.title);
    }

    /// Remove a track from the queue and adjust `queue_index`.
    /// Returns the removed track, or `None` if `index` is out of bounds.
    pub(crate) fn remove_queue_index(
        queue: &mut Arc<Vec<crate::tidal::Track>>,
        queue_index: &mut Option<usize>,
        index: usize,
    ) -> Option<crate::tidal::Track> {
        let queue_mut = Arc::make_mut(queue);
        if index >= queue_mut.len() {
            return None;
        }
        let removed = queue_mut.remove(index);
        if let Some(q_idx) = queue_index {
            if *q_idx == index {
                if queue_mut.is_empty() {
                    *queue_index = None;
                } else if *q_idx >= queue_mut.len() {
                    *q_idx = queue_mut.len() - 1;
                }
            } else if *q_idx > index {
                *q_idx -= 1;
            }
        }
        Some(removed)
    }

    pub fn play_selected_bg(&mut self) {
        if !self.authenticated {
            self.status_msg = self.lang.strings().status_login_required_short.to_string();
            return;
        }
        let track = match self.active_tab {
            Tab::Search => {
                if self.search.viewing.is_some() {
                    return;
                }
                self.search
                    .results
                    .tracks
                    .get(self.search.cursor[SearchSection::Tracks as usize])
                    .cloned()
            }
            Tab::Queue => self.queue.get(self.selected).cloned(),
            Tab::Now | Tab::Library => return,
        };
        let Some(track) = track else { return };
        let queue_index = if self.active_tab == Tab::Search
            && self.search.active_section == SearchSection::Tracks
        {
            if !self.queue.iter().any(|t| t.id == track.id) {
                Arc::make_mut(&mut self.queue).push(track.clone());
            }
            self.queue.len() - 1
        } else if self.active_tab == Tab::Queue {
            self.selected
        } else {
            return;
        };
        self.stream_track_bg(track, queue_index);
    }

    pub fn play_next_bg(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let next = match self.queue_index {
            Some(i) if i + 1 < self.queue.len() => i + 1,
            _ => 0,
        };
        let track = self.queue[next].clone();
        self.stream_track_bg(track, next);
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

    pub(crate) fn stream_track_bg(&mut self, track: Track, queue_index: usize) {
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
        let quality = self.quality.as_api_str().to_string();
        tasks::stream(
            self.tidal.clone(),
            self.tx(),
            quality,
            track,
            queue_index,
            generation,
        );
    }
}

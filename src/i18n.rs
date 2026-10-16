use crate::app::{LayoutMode, SpectrumStyle};
use crate::kitty_bg::SpectrumBg;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[serde(alias = "es")]
    Fr,
    En,
    De,
    Ro,
}

pub struct Strings {
    // Tabs
    pub tab_search: &'static str,
    pub tab_queue: &'static str,
    pub tab_now: &'static str,
    pub tab_library: &'static str,
    // Search
    pub search_placeholder: &'static str,
    pub theme_placeholder: &'static str,
    pub filter_placeholder: &'static str,
    pub search_results_title: &'static str,
    // Queue
    pub queue_title: &'static str,
    // Now playing
    pub now_playing_empty: &'static str,
    pub now_playing_title: &'static str,
    pub now_lyrics: &'static str,
    pub now_no_lyrics: &'static str,
    pub loading_image: &'static str,
    // Track list
    pub loading: &'static str,
    pub not_authenticated: &'static str,
    pub no_results_hint: &'static str,
    pub col_title: &'static str,
    pub col_artist: &'static str,
    pub col_album: &'static str,
    pub col_duration: &'static str,
    // Player bar
    pub player_stopped: &'static str,
    // Hint bar
    pub hint_play: &'static str,
    pub hint_pause: &'static str,
    pub hint_next_prev: &'static str,
    pub hint_seek: &'static str,
    pub hint_volume: &'static str,
    pub hint_view: &'static str,
    pub hint_quality: &'static str,
    pub hint_quit: &'static str,
    pub hint_library: &'static str,
    pub hint_theme: &'static str,
    pub hint_filter: &'static str,
    pub hint_panes: &'static str,
    pub hint_layout: &'static str,
    pub hint_radio: &'static str,
    pub hint_fav_tracks: &'static str,
    pub hint_fav_albums: &'static str,
    pub hint_lang: &'static str,
    pub hint_add_queue: &'static str,
    // Library
    pub library_loading: &'static str,
    pub library_hint: &'static str,
    pub library_title: &'static str,
    pub col_name: &'static str,
    pub col_info: &'static str,
    pub col_type: &'static str,
    // Fav albums
    pub fav_albums_empty: &'static str,
    pub fav_albums_title: &'static str,
    pub col_tracks: &'static str,
    // Login overlay
    pub login_title_text: &'static str,
    pub login_open_url: &'static str,
    pub login_code_prefix: &'static str,
    pub login_waiting: &'static str,
    pub login_overlay_title: &'static str,
    // Status messages (static)
    pub status_no_results: &'static str,
    pub status_no_radio_seed: &'static str,
    pub status_auth_done: &'static str,
    pub status_login_required: &'static str,
    pub status_login_required_short: &'static str,
    pub status_starting_login: &'static str,
    pub status_loading_lib: &'static str,
    pub status_loading_playlist: &'static str,
    pub status_loading_mix: &'static str,
    pub status_loading_fav_tracks: &'static str,
    pub status_loading_fav_albums: &'static str,
    pub status_session_loading: &'static str,
    pub status_session_active: &'static str,
    pub status_press_l: &'static str,
    pub status_added_to_queue: &'static str,
    // Visualizer
    pub tab_visualizer: &'static str,
    pub visualizer_title: &'static str,
    pub visualizer_empty: &'static str,
    pub visualizer_analyzing: &'static str,
    pub visualizer_unavailable: &'static str,
    // Effects
    pub tab_effects: &'static str,
    pub fx_title: &'static str,
    pub fx_eq_title: &'static str,
    pub fx_enabled: &'static str,
    pub fx_preset: &'static str,
    pub fx_balance: &'static str,
    pub fx_bass: &'static str,
    pub fx_treble: &'static str,
    pub fx_stereo_width: &'static str,
    pub fx_crossfeed: &'static str,
    pub fx_echo: &'static str,
    pub fx_compressor: &'static str,
    pub fx_normalize: &'static str,
    pub fx_center: &'static str,
    pub fx_on: &'static str,
    pub fx_off: &'static str,
    pub fx_hints: &'static str,
    pub fx_no_mpv: &'static str,
    // Spectrum background
    pub bg_requires_graphics: &'static str,
    pub bg_fetching: &'static str,
    pub visualizer_hints: &'static str,
}

static FR: Strings = Strings {
    tab_search: "Rechercher",
    tab_queue: "File",
    tab_now: "Lecture",
    tab_library: "Bibliothèque",
    search_placeholder: "Appuyez sur / pour rechercher...",
    theme_placeholder: "Thème musical (ex. trip hop, classical)...",
    filter_placeholder: "Filtre fuzzy (Esc pour effacer)...",
    search_results_title: "Résultats",
    queue_title: "File de lecture",
    now_playing_empty: "Aucune lecture — appuyez sur Entrée sur un morceau",
    now_playing_title: "◈ Lecture en cours",
    now_lyrics: "Paroles",
    now_no_lyrics: "Paroles non disponibles",
    loading_image: "⟳ Chargement\n  image...",
    loading: "  ⟳ Chargement...",
    not_authenticated: "  Appuyez sur L pour vous connecter à Tidal",
    no_results_hint: "  Aucun résultat — recherchez avec /",
    col_title: "  Titre",
    col_artist: "Artiste",
    col_album: "Album",
    col_duration: "Durée",
    player_stopped: "Aucune lecture",
    hint_play: "lire",
    hint_pause: "pause",
    hint_next_prev: "suiv/préc",
    hint_seek: "avancer",
    hint_volume: "volume",
    hint_view: "vue",
    hint_quality: "qualité",
    hint_quit: "quitter",
    hint_library: "bibliothèque",
    hint_theme: "thème",
    hint_filter: "filtrer",
    hint_panes: "volets",
    hint_layout: "disposition",
    hint_radio: "radio piste/artiste",
    hint_fav_tracks: "titres fav",
    hint_fav_albums: "albums fav",
    hint_lang: "langue",
    hint_add_queue: "ajouter file",
    library_loading: "  ⟳ Chargement de la bibliothèque...",
    library_hint: "  Appuyez sur 'i' pour charger les playlists et mixes",
    library_title: "Bibliothèque",
    col_name: "Nom",
    col_info: "Info",
    col_type: "Type",
    fav_albums_empty: "  Aucun album favori",
    fav_albums_title: "Albums favoris",
    col_tracks: "Titres",
    login_title_text: "  Connectez-vous à Tidal",
    login_open_url: "  1. Ouvrez cette URL :",
    login_code_prefix: "  2. Code : ",
    login_waiting: "  En attente d'autorisation...",
    login_overlay_title: " ◈ Authentification ",
    status_no_results: "Aucun résultat",
    status_no_radio_seed: "Aucun morceau sélectionné pour la radio",
    status_auth_done: "✓ Authentifié auprès de Tidal",
    status_login_required: "Connectez-vous d'abord avec 'L'",
    status_login_required_short: "Connectez-vous d'abord (L)",
    status_starting_login: "Démarrage de la connexion...",
    status_loading_lib: "⟳ Chargement de la bibliothèque...",
    status_loading_playlist: "⟳ Chargement de la playlist...",
    status_loading_mix: "⟳ Chargement du mix...",
    status_loading_fav_tracks: "⟳ Chargement des titres favoris...",
    status_loading_fav_albums: "⟳ Chargement des albums favoris...",
    status_session_loading: "Chargement de la session...",
    status_session_active: "✓ Session active",
    status_press_l: "Appuyez sur 'L' pour vous connecter à Tidal",
    status_added_to_queue: "▶ Ajouté à la file",
    tab_visualizer: "Spectre",
    visualizer_title: "◈ Spectre audio",
    visualizer_empty: "Aucune lecture — lancez un morceau pour voir le spectre",
    visualizer_analyzing: "⟳ Analyse du flux audio...",
    visualizer_unavailable: "Spectre indisponible pour ce morceau",
    tab_effects: "Effets",
    fx_title: "◈ Effets audio",
    fx_eq_title: "Égaliseur",
    fx_enabled: "Effets activés",
    fx_preset: "Préréglage",
    fx_balance: "Balance",
    fx_bass: "Basses",
    fx_treble: "Aigus",
    fx_stereo_width: "Largeur stéréo",
    fx_crossfeed: "Crossfeed casque",
    fx_echo: "Écho",
    fx_compressor: "Compresseur",
    fx_normalize: "Normalisation du volume",
    fx_center: "centre",
    fx_on: "activé",
    fx_off: "désactivé",
    fx_hints: "↑/↓ choisir  ←/→ régler  Entrée basculer  [ ] préréglage  R réinitialiser",
        fx_no_mpv: "⚠ mpv introuvable — les effets nécessitent mpv (brew install mpv)",
    bg_requires_graphics: "Fond d'écran : terminal avec images (kitty, Sixel, iTerm2) requis",
    bg_fetching: "⟳ Téléchargement d'une image wallhaven...",
    visualizer_hints: "b fond  B nouvelle image  v style",
};

static EN: Strings = Strings {
    tab_search: "Search",
    tab_queue: "Queue",
    tab_now: "Now",
    tab_library: "Library",
    search_placeholder: "Press / to search...",
    theme_placeholder: "Musical theme (e.g. trip hop, classical)...",
    filter_placeholder: "Fuzzy filter (Esc to clear)...",
    search_results_title: "Results",
    queue_title: "Playback Queue",
    now_playing_empty: "Nothing playing — press Enter on a track",
    now_playing_title: "◈ Now Playing",
    now_lyrics: "Lyrics",
    now_no_lyrics: "Lyrics not available",
    loading_image: "⟳ Loading\n  image...",
    loading: "  ⟳ Loading...",
    not_authenticated: "  Press L to log in to Tidal",
    no_results_hint: "  No results — search with /",
    col_title: "  Title",
    col_artist: "Artist",
    col_album: "Album",
    col_duration: "Dur.",
    player_stopped: "Nothing playing",
    hint_play: "play",
    hint_pause: "pause",
    hint_next_prev: "next/prev",
    hint_seek: "seek",
    hint_volume: "volume",
    hint_view: "view",
    hint_quality: "quality",
    hint_quit: "quit",
    hint_library: "library",
    hint_theme: "theme",
    hint_filter: "filter",
    hint_panes: "panes",
    hint_layout: "layout",
    hint_radio: "track/artist radio",
    hint_fav_tracks: "fav tracks",
    hint_fav_albums: "fav albums",
    hint_lang: "language",
    hint_add_queue: "add queue",
    library_loading: "  ⟳ Loading library...",
    library_hint: "  Press 'i' to load playlists and mixes",
    library_title: "Library",
    col_name: "Name",
    col_info: "Info",
    col_type: "Type",
    fav_albums_empty: "  No favorite albums",
    fav_albums_title: "Favorite Albums",
    col_tracks: "Tracks",
    login_title_text: "  Log in to Tidal",
    login_open_url: "  1. Open this URL:",
    login_code_prefix: "  2. Code: ",
    login_waiting: "  Waiting for authorization...",
    login_overlay_title: " ◈ Authentication ",
    status_no_results: "No results",
    status_no_radio_seed: "No track selected for the radio",
    status_auth_done: "✓ Authenticated with Tidal",
    status_login_required: "Log in first with 'L'",
    status_login_required_short: "Log in first (L)",
    status_starting_login: "Starting login...",
    status_loading_lib: "⟳ Loading library...",
    status_loading_playlist: "⟳ Loading playlist...",
    status_loading_mix: "⟳ Loading mix...",
    status_loading_fav_tracks: "⟳ Loading favorite tracks...",
    status_loading_fav_albums: "⟳ Loading favorite albums...",
    status_session_loading: "Loading session...",
    status_session_active: "✓ Session active",
    status_press_l: "Press 'L' to log in to Tidal",
    status_added_to_queue: "▶ Added to queue",
    tab_visualizer: "Spectrum",
    visualizer_title: "◈ Audio Spectrum",
    visualizer_empty: "Nothing playing — play a track to see the spectrum",
    visualizer_analyzing: "⟳ Analyzing audio stream...",
    visualizer_unavailable: "Spectrum unavailable for this track",
    tab_effects: "Effects",
    fx_title: "◈ Audio Effects",
    fx_eq_title: "Equalizer",
    fx_enabled: "Effects enabled",
    fx_preset: "Preset",
    fx_balance: "Balance",
    fx_bass: "Bass",
    fx_treble: "Treble",
    fx_stereo_width: "Stereo width",
    fx_crossfeed: "Headphone crossfeed",
    fx_echo: "Echo",
    fx_compressor: "Compressor",
    fx_normalize: "Volume normalization",
    fx_center: "center",
    fx_on: "enabled",
    fx_off: "disabled",
    fx_hints: "↑/↓ select  ←/→ adjust  Enter toggle  [ ] preset  R reset",
        fx_no_mpv: "⚠ mpv not found — effects require mpv (brew install mpv)",
    bg_requires_graphics: "Background: graphics-capable terminal (kitty, Sixel, iTerm2) required",
    bg_fetching: "⟳ Fetching a wallhaven image...",
    visualizer_hints: "b background  B new image  v style",
};

static DE: Strings = Strings {
    tab_search: "Suchen",
    tab_queue: "Warteschl.",
    tab_now: "Jetzt",
    tab_library: "Bibliothek",
    search_placeholder: "/ zum Suchen drücken...",
    theme_placeholder: "Musikrichtung (z. B. Trip-Hop, Klassik)...",
    filter_placeholder: "Fuzzy-Filter (Esc zum Löschen)...",
    search_results_title: "Ergebnisse",
    queue_title: "Wiedergabeliste",
    now_playing_empty: "Keine Wiedergabe — Enter auf einem Titel drücken",
    now_playing_title: "◈ Jetzt läuft",
    now_lyrics: "Text",
    now_no_lyrics: "Kein Text verfügbar",
    loading_image: "⟳ Lädt\n  Bild...",
    loading: "  ⟳ Lädt...",
    not_authenticated: "  L drücken um sich bei Tidal anzumelden",
    no_results_hint: "  Keine Ergebnisse — suche mit /",
    col_title: "  Titel",
    col_artist: "Künstler",
    col_album: "Album",
    col_duration: "Dauer",
    player_stopped: "Keine Wiedergabe",
    hint_play: "abspielen",
    hint_pause: "pause",
    hint_next_prev: "vor/zurück",
    hint_seek: "spulen",
    hint_volume: "Lautstärke",
    hint_view: "Ansicht",
    hint_quality: "Qualität",
    hint_quit: "beenden",
    hint_library: "Bibliothek",
    hint_theme: "Genre",
    hint_filter: "filtern",
    hint_panes: "Bereiche",
    hint_layout: "Layout",
    hint_radio: "Titel-/Künstler-Radio",
    hint_fav_tracks: "Fav-Titel",
    hint_fav_albums: "Fav-Alben",
    hint_lang: "Sprache",
    hint_add_queue: "hinzufügen",
    library_loading: "  ⟳ Bibliothek wird geladen...",
    library_hint: "  'i' drücken um Playlists und Mixes zu laden",
    library_title: "Bibliothek",
    col_name: "Name",
    col_info: "Info",
    col_type: "Typ",
    fav_albums_empty: "  Keine Lieblingsalben",
    fav_albums_title: "Lieblingsalben",
    col_tracks: "Titel",
    login_title_text: "  Bei Tidal anmelden",
    login_open_url: "  1. URL öffnen:",
    login_code_prefix: "  2. Code: ",
    login_waiting: "  Warte auf Autorisierung...",
    login_overlay_title: " ◈ Authentifizierung ",
    status_no_results: "Keine Ergebnisse",
    status_no_radio_seed: "Kein Titel für das Radio ausgewählt",
    status_auth_done: "✓ Bei Tidal authentifiziert",
    status_login_required: "Zuerst mit 'L' anmelden",
    status_login_required_short: "Zuerst anmelden (L)",
    status_starting_login: "Anmeldung wird gestartet...",
    status_loading_lib: "⟳ Bibliothek wird geladen...",
    status_loading_playlist: "⟳ Playlist wird geladen...",
    status_loading_mix: "⟳ Mix wird geladen...",
    status_loading_fav_tracks: "⟳ Lieblingstitel werden geladen...",
    status_loading_fav_albums: "⟳ Lieblingsalben werden geladen...",
    status_session_loading: "Sitzung wird geladen...",
    status_session_active: "✓ Sitzung aktiv",
    status_press_l: "'L' drücken um sich bei Tidal anzumelden",
    status_added_to_queue: "▶ Zur Warteschlange hinzugefügt",
    tab_visualizer: "Spektrum",
    visualizer_title: "◈ Audiospektrum",
    visualizer_empty: "Keine Wiedergabe — Titel abspielen, um das Spektrum zu sehen",
    visualizer_analyzing: "⟳ Audiostream wird analysiert...",
    visualizer_unavailable: "Spektrum für diesen Titel nicht verfügbar",
    tab_effects: "Effekte",
    fx_title: "◈ Audioeffekte",
    fx_eq_title: "Equalizer",
    fx_enabled: "Effekte aktiv",
    fx_preset: "Voreinstellung",
    fx_balance: "Balance",
    fx_bass: "Bässe",
    fx_treble: "Höhen",
    fx_stereo_width: "Stereobreite",
    fx_crossfeed: "Kopfhörer-Crossfeed",
    fx_echo: "Echo",
    fx_compressor: "Kompressor",
    fx_normalize: "Lautstärkenormalisierung",
    fx_center: "Mitte",
    fx_on: "an",
    fx_off: "aus",
    fx_hints: "↑/↓ wählen  ←/→ ändern  Enter umschalten  [ ] Voreinstellung  R zurücksetzen",
        fx_no_mpv: "⚠ mpv nicht gefunden — Effekte benötigen mpv (brew install mpv)",
    bg_requires_graphics: "Hintergrund: Terminal mit Grafikunterstützung (kitty, Sixel, iTerm2) erforderlich",
    bg_fetching: "⟳ Wallhaven-Bild wird geladen...",
    visualizer_hints: "b Hintergrund  B neues Bild  v Stil",
};

static RO: Strings = Strings {
    tab_search: "Caută",
    tab_queue: "Coadă",
    tab_now: "Acum",
    tab_library: "Bibliotecă",
    search_placeholder: "Apasă / pentru a căuta...",
    theme_placeholder: "Temă muzicală (ex. trip hop, clasică)...",
    filter_placeholder: "Filtru fuzzy (Esc pentru a șterge)...",
    search_results_title: "Rezultate",
    queue_title: "Coadă de redare",
    now_playing_empty: "Nimic nu rulează — apasă Enter pe o piesă",
    now_playing_title: "◈ Se redă acum",
    now_lyrics: "Versuri",
    now_no_lyrics: "Versuri indisponibile",
    loading_image: "⟳ Se încarcă\n  imaginea...",
    loading: "  ⟳ Se încarcă...",
    not_authenticated: "  Apasă L pentru a te autentifica în Tidal",
    no_results_hint: "  Niciun rezultat — caută cu /",
    col_title: "  Titlu",
    col_artist: "Artist",
    col_album: "Album",
    col_duration: "Dur.",
    player_stopped: "Nimic nu rulează",
    hint_play: "redă",
    hint_pause: "pauză",
    hint_next_prev: "urm/ant",
    hint_seek: "avans",
    hint_volume: "volum",
    hint_view: "vedere",
    hint_quality: "calitate",
    hint_quit: "ieșire",
    hint_library: "bibliotecă",
    hint_theme: "temă",
    hint_filter: "filtrează",
    hint_panes: "panouri",
    hint_layout: "aspect",
    hint_radio: "radio piesă/artist",
    hint_fav_tracks: "piese fav",
    hint_fav_albums: "albume fav",
    hint_lang: "limbă",
    hint_add_queue: "adaugă coadă",
    library_loading: "  ⟳ Se încarcă biblioteca...",
    library_hint: "  Apasă 'i' pentru a încărca playlisturi și mixuri",
    library_title: "Bibliotecă",
    col_name: "Nume",
    col_info: "Info",
    col_type: "Tip",
    fav_albums_empty: "  Niciun album favorit",
    fav_albums_title: "Albume favorite",
    col_tracks: "Piese",
    login_title_text: "  Autentifică-te în Tidal",
    login_open_url: "  1. Deschide acest URL:",
    login_code_prefix: "  2. Cod: ",
    login_waiting: "  Se așteaptă autorizarea...",
    login_overlay_title: " ◈ Autentificare ",
    status_no_results: "Niciun rezultat",
    status_no_radio_seed: "Niciun titlu selectat pentru radio",
    status_auth_done: "✓ Autentificat în Tidal",
    status_login_required: "Autentifică-te mai întâi cu 'L'",
    status_login_required_short: "Autentifică-te mai întâi (L)",
    status_starting_login: "Se inițiază autentificarea...",
    status_loading_lib: "⟳ Se încarcă biblioteca...",
    status_loading_playlist: "⟳ Se încarcă playlistul...",
    status_loading_mix: "⟳ Se încarcă mixul...",
    status_loading_fav_tracks: "⟳ Se încarcă piesele favorite...",
    status_loading_fav_albums: "⟳ Se încarcă albumele favorite...",
    status_session_loading: "Se încarcă sesiunea...",
    status_session_active: "✓ Sesiune activă",
    status_press_l: "Apasă 'L' pentru a te autentifica în Tidal",
    status_added_to_queue: "▶ Adăugat în coadă",
    tab_visualizer: "Spectru",
    visualizer_title: "◈ Spectru audio",
    visualizer_empty: "Nimic nu rulează — redă o piesă pentru a vedea spectrul",
    visualizer_analyzing: "⟳ Se analizează fluxul audio...",
    visualizer_unavailable: "Spectru indisponibil pentru această piesă",
    tab_effects: "Efecte",
    fx_title: "◈ Efecte audio",
    fx_eq_title: "Egalizator",
    fx_enabled: "Efecte active",
    fx_preset: "Presetare",
    fx_balance: "Balans",
    fx_bass: "Bași",
    fx_treble: "Înalte",
    fx_stereo_width: "Lățime stereo",
    fx_crossfeed: "Crossfeed căști",
    fx_echo: "Ecou",
    fx_compressor: "Compresor",
    fx_normalize: "Normalizare volum",
    fx_center: "centru",
    fx_on: "activat",
    fx_off: "dezactivat",
    fx_hints: "↑/↓ alege  ←/→ reglează  Enter comută  [ ] presetare  R resetează",
        fx_no_mpv: "⚠ mpv negăsit — efectele necesită mpv (brew install mpv)",
    bg_requires_graphics: "Fundal: necesită un terminal cu grafică (kitty, Sixel, iTerm2)",
    bg_fetching: "⟳ Se descarcă o imagine wallhaven...",
    visualizer_hints: "b fundal  B imagine nouă  v stil",
};

impl Lang {
    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::Fr => &FR,
            Lang::En => &EN,
            Lang::De => &DE,
            Lang::Ro => &RO,
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Lang::Fr => Lang::En,
            Lang::En => Lang::De,
            Lang::De => Lang::Ro,
            Lang::Ro => Lang::Fr,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::Fr => "FR",
            Lang::En => "EN",
            Lang::De => "DE",
            Lang::Ro => "RO",
        }
    }

    // ── Dynamic string methods ────────────────────────────────────────────────

    pub fn results_count(self, n: usize) -> String {
        match self {
            Lang::Fr => format!("{n} résultats"),
            Lang::En => format!("{n} results"),
            Lang::De => format!("{n} Ergebnisse"),
            Lang::Ro => format!("{n} rezultate"),
        }
    }

    pub fn search_error(self, e: &str) -> String {
        match self {
            Lang::Fr => format!("✗ Erreur de recherche : {e}"),
            Lang::En => format!("✗ Search error: {e}"),
            Lang::De => format!("✗ Suchfehler: {e}"),
            Lang::Ro => format!("✗ Eroare căutare: {e}"),
        }
    }

    pub fn stream_error(self, e: &str) -> String {
        match self {
            Lang::Fr => format!("✗ Erreur de stream : {e}"),
            Lang::En => format!("✗ Stream error: {e}"),
            Lang::De => format!("✗ Stream-Fehler: {e}"),
            Lang::Ro => format!("✗ Eroare stream: {e}"),
        }
    }

    pub fn searching(self, q: &str) -> String {
        match self {
            Lang::Fr => format!("Recherche de \"{q}\"..."),
            Lang::En => format!("Searching \"{q}\"..."),
            Lang::De => format!("Suche \"{q}\"..."),
            Lang::Ro => format!("Se caută \"{q}\"..."),
        }
    }

    pub fn radio_loading(self, name: &str) -> String {
        match self {
            Lang::Fr => format!("⟳ Radio « {name} »..."),
            Lang::En => format!("⟳ Radio “{name}”..."),
            Lang::De => format!("⟳ Radio „{name}“..."),
            Lang::Ro => format!("⟳ Radio „{name}”..."),
        }
    }

    pub fn radio_started(self, title: &str, n: usize) -> String {
        match self {
            Lang::Fr => format!("📻 Radio « {title} » : {n} pistes"),
            Lang::En => format!("📻 Radio “{title}”: {n} tracks"),
            Lang::De => format!("📻 Radio „{title}“: {n} Titel"),
            Lang::Ro => format!("📻 Radio „{title}”: {n} piese"),
        }
    }

    pub fn radio_error(self, e: &str) -> String {
        match self {
            Lang::Fr => format!("✗ Radio : {e}"),
            Lang::En => format!("✗ Radio: {e}"),
            Lang::De => format!("✗ Radio: {e}"),
            Lang::Ro => format!("✗ Radio: {e}"),
        }
    }

    pub fn searching_theme(self, q: &str) -> String {
        match self {
            Lang::Fr => format!("⟳ Recherche de playlists « {q} »..."),
            Lang::En => format!("⟳ Searching playlists \"{q}\"..."),
            Lang::De => format!("⟳ Suche Playlists \"{q}\"..."),
            Lang::Ro => format!("⟳ Se caută playlisturi „{q}”..."),
        }
    }

    pub fn theme_results_count(self, n: usize, q: &str) -> String {
        match self {
            Lang::Fr => format!("{n} playlists pour « {q} »"),
            Lang::En => format!("{n} playlists for \"{q}\""),
            Lang::De => format!("{n} Playlists für \"{q}\""),
            Lang::Ro => format!("{n} playlisturi pentru „{q}”"),
        }
    }

    pub fn theme_results_title(self, q: &str) -> String {
        match self {
            Lang::Fr => format!(" Playlists : {q} "),
            Lang::En => format!(" Playlists: {q} "),
            Lang::De => format!(" Playlists: {q} "),
            Lang::Ro => format!(" Playlisturi: {q} "),
        }
    }

    pub fn loading_stream(self, title: &str) -> String {
        match self {
            Lang::Fr => format!("⟳ Récupération du stream : {title}..."),
            Lang::En => format!("⟳ Getting stream: {title}..."),
            Lang::De => format!("⟳ Stream wird geladen: {title}..."),
            Lang::Ro => format!("⟳ Se obține stream: {title}..."),
        }
    }

    pub fn browser_opened(self, code: &str) -> String {
        match self {
            Lang::Fr => format!("Navigateur ouvert. Code : {code}"),
            Lang::En => format!("Browser opened. Code: {code}"),
            Lang::De => format!("Browser geöffnet. Code: {code}"),
            Lang::Ro => format!("Browser deschis. Cod: {code}"),
        }
    }

    pub fn browser_failed(self, e: &str, url: &str) -> String {
        match self {
            Lang::Fr => format!("Impossible d'ouvrir le navigateur ({e}) : {url}"),
            Lang::En => format!("Could not open browser ({e}): {url}"),
            Lang::De => format!("Browser konnte nicht geöffnet werden ({e}): {url}"),
            Lang::Ro => format!("Nu s-a putut deschide browser-ul ({e}): {url}"),
        }
    }

    pub fn auth_error(self, e: &str) -> String {
        match self {
            Lang::Fr => format!("✗ Erreur d'authentification : {e}"),
            Lang::En => format!("✗ Auth error: {e}"),
            Lang::De => format!("✗ Auth-Fehler: {e}"),
            Lang::Ro => format!("✗ Eroare autentificare: {e}"),
        }
    }

    pub fn library_loaded(self, playlists: usize, mixes: usize) -> String {
        match self {
            Lang::Fr => format!("✓ {playlists} playlists, {mixes} mixes"),
            Lang::En => format!("✓ {playlists} playlists, {mixes} mixes"),
            Lang::De => format!("✓ {playlists} Playlists, {mixes} Mixes"),
            Lang::Ro => format!("✓ {playlists} playlisturi, {mixes} mixuri"),
        }
    }

    pub fn tracks_loaded(self, n: usize) -> String {
        match self {
            Lang::Fr => format!("✓ {n} titres chargés"),
            Lang::En => format!("✓ {n} tracks loaded"),
            Lang::De => format!("✓ {n} Titel geladen"),
            Lang::Ro => format!("✓ {n} piese încărcate"),
        }
    }

    pub fn fav_tracks_loaded(self, n: usize) -> String {
        match self {
            Lang::Fr => format!("✓ {n} titres favoris dans la file"),
            Lang::En => format!("✓ {n} favorite tracks in queue"),
            Lang::De => format!("✓ {n} Lieblingstitel in der Warteschlange"),
            Lang::Ro => format!("✓ {n} piese favorite în coadă"),
        }
    }

    pub fn fav_albums_loaded(self, n: usize) -> String {
        match self {
            Lang::Fr => format!("✓ {n} albums dans la collection"),
            Lang::En => format!("✓ {n} albums in collection"),
            Lang::De => format!("✓ {n} Alben in der Sammlung"),
            Lang::Ro => format!("✓ {n} albume în colecție"),
        }
    }

    pub fn quality_changed(self, label: &str) -> String {
        match self {
            Lang::Fr => format!("Qualité : {label}"),
            Lang::En => format!("Quality: {label}"),
            Lang::De => format!("Qualität: {label}"),
            Lang::Ro => format!("Calitate: {label}"),
        }
    }

    pub fn loading_album(self, title: &str) -> String {
        match self {
            Lang::Fr => format!("⟳ Chargement de {title}..."),
            Lang::En => format!("⟳ Loading {title}..."),
            Lang::De => format!("⟳ {title} wird geladen..."),
            Lang::Ro => format!("⟳ Se încarcă {title}..."),
        }
    }

    pub fn library_title_with_counts(self, playlists: usize, mixes: usize) -> String {
        match self {
            Lang::Fr => format!(" Bibliothèque ({playlists} playlists, {mixes} mixes) "),
            Lang::En => format!(" Library ({playlists} playlists, {mixes} mixes) "),
            Lang::De => format!(" Bibliothek ({playlists} Playlists, {mixes} Mixes) "),
            Lang::Ro => format!(" Bibliotecă ({playlists} playlisturi, {mixes} mixuri) "),
        }
    }

    pub fn fav_albums_title_with_count(self, n: usize) -> String {
        match self {
            Lang::Fr => format!(" ◆ Albums favoris ({n}) — Entrée pour charger "),
            Lang::En => format!(" ◆ Favorite Albums ({n}) — Enter to load "),
            Lang::De => format!(" ◆ Lieblingsalben ({n}) — Enter zum Laden "),
            Lang::Ro => format!(" ◆ Albume favorite ({n}) — Enter pentru a încărca "),
        }
    }

    pub fn tracks_count(self, n: u32) -> String {
        match self {
            Lang::Fr => format!("{n} titres"),
            Lang::En => format!("{n} tracks"),
            Lang::De => format!("{n} Titel"),
            Lang::Ro => format!("{n} piese"),
        }
    }

    pub fn lang_changed(self) -> String {
        match self {
            Lang::Fr => "Langue : Français".to_string(),
            Lang::En => "Language: English".to_string(),
            Lang::De => "Sprache: Deutsch".to_string(),
            Lang::Ro => "Limbă: Română".to_string(),
        }
    }

    pub fn fx_status(self, n: usize) -> String {
        match (self, n) {
            (Lang::Fr, 0) => "Effets : neutre".to_string(),
            (Lang::Fr, n) => format!("Effets : {n} filtre(s) actif(s)"),
            (Lang::En, 0) => "Effects: neutral".to_string(),
            (Lang::En, n) => format!("Effects: {n} active filter(s)"),
            (Lang::De, 0) => "Effekte: neutral".to_string(),
            (Lang::De, n) => format!("Effekte: {n} aktive(r) Filter"),
            (Lang::Ro, 0) => "Efecte: neutru".to_string(),
            (Lang::Ro, n) => format!("Efecte: {n} filtru(e) activ(e)"),
        }
    }

    /// Short label for a SpectrumBg source ("none", "cover", "wallhaven").
    pub fn bg_label(self, bg: SpectrumBg) -> &'static str {
        match (self, bg) {
            (Lang::Fr, SpectrumBg::None) => "aucun",
            (Lang::Fr, SpectrumBg::Cover) => "pochette",
            (Lang::En, SpectrumBg::None) => "none",
            (Lang::En, SpectrumBg::Cover) => "cover",
            (Lang::De, SpectrumBg::None) => "kein",
            (Lang::De, SpectrumBg::Cover) => "Cover",
            (Lang::Ro, SpectrumBg::None) => "niciunul",
            (Lang::Ro, SpectrumBg::Cover) => "copertă",
            (_, SpectrumBg::Wallhaven) => "wallhaven",
        }
    }

    pub fn bg_source_changed(self, label: &str) -> String {
        match self {
            Lang::Fr => format!("Fond : {label}"),
            Lang::En => format!("Background: {label}"),
            Lang::De => format!("Hintergrund: {label}"),
            Lang::Ro => format!("Fundal: {label}"),
        }
    }

    /// Short label for a SpectrumStyle ("curve", "bars").
    pub fn spectrum_style_label(self, style: SpectrumStyle) -> &'static str {
        match (self, style) {
            (Lang::Fr, SpectrumStyle::Curve) => "courbe",
            (Lang::En, SpectrumStyle::Curve) => "curve",
            (Lang::De, SpectrumStyle::Curve) => "Kurve",
            (Lang::Ro, SpectrumStyle::Curve) => "curbă",
            (Lang::Fr, SpectrumStyle::Bars) => "barres",
            (Lang::En, SpectrumStyle::Bars) => "bars",
            (Lang::De, SpectrumStyle::Bars) => "Balken",
            (Lang::Ro, SpectrumStyle::Bars) => "bare",
        }
    }

    /// Short label for a LayoutMode ("split", "tabs").
    pub fn layout_label(self, layout: LayoutMode) -> &'static str {
        match (self, layout) {
            (Lang::Fr, LayoutMode::Split) => "vue partagée",
            (Lang::Fr, LayoutMode::Tabs) => "onglets",
            (Lang::En, LayoutMode::Split) => "split view",
            (Lang::En, LayoutMode::Tabs) => "tabs",
            (Lang::De, LayoutMode::Split) => "geteilte Ansicht",
            (Lang::De, LayoutMode::Tabs) => "Tabs",
            (Lang::Ro, LayoutMode::Split) => "vedere împărțită",
            (Lang::Ro, LayoutMode::Tabs) => "file",
        }
    }

    pub fn layout_changed(self, label: &str) -> String {
        match self {
            Lang::Fr => format!("Disposition : {label}"),
            Lang::En => format!("Layout: {label}"),
            Lang::De => format!("Layout: {label}"),
            Lang::Ro => format!("Aspect: {label}"),
        }
    }

    pub fn queue_title_with_totals(self, n: usize, total: &str) -> String {
        match self {
            Lang::Fr => format!("File d'attente ({n} · {total})"),
            Lang::En => format!("Queue ({n} · {total})"),
            Lang::De => format!("Warteschlange ({n} · {total})"),
            Lang::Ro => format!("Coadă ({n} · {total})"),
        }
    }

    pub fn spectrum_style_changed(self, label: &str) -> String {
        match self {
            Lang::Fr => format!("Spectre : {label}"),
            Lang::En => format!("Spectrum: {label}"),
            Lang::De => format!("Spektrum: {label}"),
            Lang::Ro => format!("Spectru: {label}"),
        }
    }

    pub fn bg_error(self, e: &str) -> String {
        match self {
            Lang::Fr => format!("✗ Fond : {e}"),
            Lang::En => format!("✗ Background: {e}"),
            Lang::De => format!("✗ Hintergrund: {e}"),
            Lang::Ro => format!("✗ Fundal: {e}"),
        }
    }
}

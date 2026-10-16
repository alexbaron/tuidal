#!/usr/bin/env python3
"""
tidal.py — Sous-processus intermédiaire entre Rust et Tidal via tidalapi.

Modes :
  python3 tidal.py --daemon                       (mode persistant JSON-RPC)
  python3 tidal.py auth start|poll
  python3 tidal.py search "Daft Punk"
  python3 tidal.py search_playlists "trip hop"
  python3 tidal.py stream 12345 [quality]
  python3 tidal.py cover 12345
  python3 tidal.py playlists
  python3 tidal.py playlist_tracks <uuid>
  python3 tidal.py mixes
  python3 tidal.py mix_tracks <id>
  python3 tidal.py fav_tracks
  python3 tidal.py fav_albums
  python3 tidal.py lyrics <id>
  python3 tidal.py album_tracks <id>

Identifiants API : lus dans `.env` (clés `id` et `secret`) à côté du script
ou dans le cwd, ou via TIDAL_CLIENT_ID / TIDAL_CLIENT_SECRET.
Si Tidal refuse ces identifiants (device flow non autorisé), retombe
automatiquement sur ceux intégrés à tidalapi.
"""

import json
import os
import sys
import threading
from pathlib import Path
from tidalapi.user import ItemOrder, AlbumOrder, OrderDirection
import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry
import tidalapi

SESSION_FILE = Path.home() / ".config" / "tidal-tui" / "tidalapi_session.json"

# Errors raised when a pooled keep-alive connection was closed by the server
# (or a NAT) while a track was playing: the next request hits a dead socket.
TRANSIENT_ERRORS = (requests.exceptions.ConnectionError, requests.exceptions.Timeout)


def mount_retries(session: tidalapi.Session):
    """Retry idempotent requests on dropped connections and 5xx gateway errors."""
    retry = Retry(
        total=3,
        connect=3,
        read=2,
        status=2,
        backoff_factor=0.5,
        status_forcelist=(502, 503, 504),
        raise_on_status=False,
    )
    adapter = HTTPAdapter(max_retries=retry)
    session.request_session.mount("https://", adapter)
    session.request_session.mount("http://", adapter)


def call_with_reconnect(session: tidalapi.Session, fn):
    """Run fn(); if the keep-alive connection was reset, drop the pool and retry once."""
    try:
        return fn()
    except TRANSIENT_ERRORS:
        session.request_session.close()
        return fn()


def load_env_file() -> dict:
    """Parse a .env file found next to this script or in the cwd (first hit wins).

    Returns {} when no .env exists. Blank lines and `#` comments are skipped;
    an optional `export ` prefix and surrounding quotes on values are stripped.
    """
    for candidate in (
        Path(__file__).resolve().parent / ".env",
        Path.cwd() / ".env",
    ):
        if candidate.exists():
            env_path = candidate
            break
    else:
        return {}

    data = {}
    for line in env_path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[len("export "):].strip()
        if "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in ("'", '"'):
            value = value[1:-1]
        data[key] = value
    return data


def tidal_credentials() -> "tuple[str | None, str | None]":
    """Resolve Tidal API credentials: env vars first, then the .env file."""
    client_id = os.environ.get("TIDAL_CLIENT_ID")
    client_secret = os.environ.get("TIDAL_CLIENT_SECRET")
    if client_id or client_secret:
        return client_id, client_secret
    env = load_env_file()
    return env.get("id"), env.get("secret")

QUALITY_MAP = {
    "HI_RES_LOSSLESS": tidalapi.Quality.hi_res_lossless,
    "LOSSLESS":        tidalapi.Quality.high_lossless,
    "HIGH":            tidalapi.Quality.low_320k,
}

FALLBACK_CHAIN = {
    "HI_RES_LOSSLESS": ["HI_RES_LOSSLESS", "LOSSLESS", "HIGH"],
    "LOSSLESS":        ["LOSSLESS", "HIGH"],
    "HIGH":            ["HIGH"],
}


def out(data):
    print(json.dumps(data, ensure_ascii=False))
    sys.stdout.flush()


def make_session(quality=None, use_env=True) -> tidalapi.Session:
    config = tidalapi.Config(quality=quality) if quality else tidalapi.Config()
    client_id, client_secret = tidal_credentials() if use_env else (None, None)
    if client_id:
        config.client_id = client_id
        # tidalapi falls back to client_id when no secret is provided
        config.client_secret = client_secret or client_id
    session = tidalapi.Session(config)
    mount_retries(session)
    # Remember which credentials this session was created with: a token issued
    # for one client_id cannot be refreshed by another.
    session.uses_env_credentials = bool(use_env and client_id)
    return session


def open_session(quality=None) -> tidalapi.Session:
    """Load the saved session, falling back to tidalapi's built-in credentials
    when the .env credentials cannot use the saved token."""
    session = make_session(quality)
    if load_session(session):
        return session
    if session.uses_env_credentials:
        fallback = make_session(quality, use_env=False)
        if load_session(fallback):
            return fallback
    return session


def start_oauth(session):
    """Start the OAuth device flow, falling back to tidalapi's built-in
    credentials when Tidal rejects the .env ones (e.g. not a Limited Input
    Device client). Returns (session, link_login, future)."""
    try:
        link_login, future = session.login_oauth()
    except Exception:
        if not session.uses_env_credentials:
            raise
        session = make_session(use_env=False)
        link_login, future = session.login_oauth()
    return session, link_login, future


def load_session(session: tidalapi.Session) -> bool:
    try:
        session.load_session_from_file(SESSION_FILE)
        return session.check_login()
    except Exception:
        return False


def save_session(session: tidalapi.Session):
    SESSION_FILE.parent.mkdir(parents=True, exist_ok=True)
    session.save_session_to_file(SESSION_FILE)


# ─── Handlers réutilisables (retournent dict/list, utilisés par CLI et daemon) ─


def _track_dict(t: tidalapi.Track) -> dict:
    return {
        "id":            t.id,
        "title":         t.name,
        "duration":      t.duration,
        "track_number":  getattr(t, "track_num", None),
        "artists":       [{"id": a.id, "name": a.name} for a in t.artists],
        "album":         {"id": t.album.id, "title": t.album.name},
        "audio_quality": str(getattr(t, "audio_quality", "") or ""),
        "explicit":      getattr(t, "explicit", False),
    }


def _album_dict(a) -> dict:
    artists = []
    if hasattr(a, 'artists') and a.artists:
        artists = [{"id": art.id, "name": art.name} for art in a.artists]
    elif hasattr(a, 'artist') and a.artist:
        artists = [{"id": a.artist.id, "name": a.artist.name}]
    return {
        "id":             a.id,
        "title":          a.name,
        "numberOfTracks": getattr(a, 'num_tracks', 0) or 0,
        "duration":       getattr(a, 'duration', 0) or 0,
        "artists":        artists,
        "coverUrl":       a.image(320) if hasattr(a, 'image') else None,
    }


def handle_search(session, query, limit=20):
    results = session.search(query, [tidalapi.Track], limit=limit)
    tracks = results.get("tracks", []) or []
    return [_track_dict(t) for t in tracks[:limit]]


def handle_stream(session, track_id, quality=None):
    q_str = quality or "LOSSLESS"
    last_error = ""
    for q in FALLBACK_CHAIN.get(q_str, ["LOSSLESS", "HIGH"]):
        tmp_session = open_session(quality=QUALITY_MAP[q])
        if not tmp_session.check_login():
            raise Exception("Non authentifié")
        try:
            track = tmp_session.track(track_id)
            url = track.get_url()
            if url:
                return {
                    "url":         url,
                    "codec":       "flac" if q in ("HI_RES_LOSSLESS", "LOSSLESS") else "aac",
                    "bit_depth":   24 if q == "HI_RES_LOSSLESS" else 16,
                    "sample_rate": 96000 if q == "HI_RES_LOSSLESS" else 44100,
                    "mime_type":   "audio/flac" if q in ("HI_RES_LOSSLESS", "LOSSLESS") else "audio/aac",
                    "quality":     q,
                }
        except Exception as e:
            last_error = str(e)
            continue
    raise Exception(f"Impossible d'obtenir le stream dans aucune qualité : {last_error}")


def handle_cover(session, track_id):
    track = session.track(track_id)
    # Prefer the highest resolution; not every album offers every size.
    url = None
    for size in (1280, 640, 320):
        try:
            url = track.album.image(size)
            break
        except Exception:
            continue
    return {"url": url, "title": track.name, "artist": track.artist.name, "album": track.album.name}


def handle_lyrics(session, track_id):
    track = session.track(track_id)
    lyrics = track.lyrics()
    return {"trackId": track_id, "lyrics": lyrics.text, "subtitles": lyrics.subtitles}


def _playlist_dict(p):
    return {
        "uuid":             str(p.id),
        "title":            p.name,
        "numberOfTracks":   p.num_tracks or 0,
        "duration":         p.duration or 0,
        "type":             str(getattr(p, 'type', 'USER')),
        "publicPlaylist":   getattr(p, 'public', False),
    }


def handle_playlists(session):
    return [_playlist_dict(p) for p in session.user.playlists()]


def handle_search_playlists(session, query, limit=20):
    results = session.search(query, [tidalapi.Playlist], limit=limit)
    playlists = results.get("playlists", []) or []
    return [_playlist_dict(p) for p in playlists[:limit]]


def handle_playlist_tracks(session, uuid):
    playlist = session.playlist(uuid)
    tracks = playlist.tracks()
    return [_track_dict(t) for t in tracks]


def handle_mixes(session):
    mixes = session.mixes()
    result = []
    for m in mixes:
        result.append({
            "id":       str(m.id),
            "title":    m.title,
            "subTitle": getattr(m, 'sub_title', None),
        })
    return result


def handle_mix_tracks(session, mix_id):
    mix = session.mix(mix_id)
    items = mix.items()
    tracks = [t for t in items if isinstance(t, tidalapi.Track)]
    return [_track_dict(t) for t in tracks]


def handle_favorite_tracks(session):
    favorites = tidalapi.Favorites(session, session.user.id)
    tracks = favorites.tracks(
        limit=500,
        order=ItemOrder.Date,
        order_direction=OrderDirection.Descending,
    )
    return [_track_dict(t) for t in tracks]


def handle_favorite_albums(session):
    favorites = tidalapi.Favorites(session, session.user.id)
    albums = favorites.albums(
        limit=400,
        order=AlbumOrder.DateAdded,
        order_direction=OrderDirection.Descending,
    )
    return [_album_dict(a) for a in albums]


def handle_album_tracks(session, album_id):
    album = session.album(album_id)
    tracks = album.tracks()
    return [_track_dict(t) for t in tracks]


# ─── Mode daemon (JSON-RPC persistant sur stdin/stdout) ────────────────────


HANDLERS = {
    "search":         handle_search,
    "stream":         handle_stream,
    "cover":          handle_cover,
    "lyrics":         handle_lyrics,
    "playlists":      handle_playlists,
    "search_playlists": handle_search_playlists,
    "playlist_tracks": handle_playlist_tracks,
    "mixes":          handle_mixes,
    "mix_tracks":     handle_mix_tracks,
    "fav_tracks":     handle_favorite_tracks,
    "fav_albums":     handle_favorite_albums,
    "album_tracks":   handle_album_tracks,
}


def run_daemon():
    quality = "LOSSLESS"
    session = open_session()
    auth_in_progress = False

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            req = json.loads(line)
        except json.JSONDecodeError:
            continue

        req_id = req.get("id", 0)
        method = req.get("method")
        params = req.get("params", {})

        if method == "shutdown":
            out({"id": req_id, "result": {"ok": True}})
            break

        try:
            if method == "auth_start":
                session, link_login, future = start_oauth(session)
                url = str(link_login.verification_uri_complete)
                code = str(link_login.user_code)

                def wait_and_save(s=session):
                    try:
                        future.result()
                        save_session(s)
                    except Exception:
                        pass

                t = threading.Thread(target=wait_and_save, daemon=True)
                t.start()
                out({"id": req_id, "result": {"url": url, "code": code, "device_code": "pending"}})

            elif method == "auth_poll":
                authed = session.check_login()
                out({"id": req_id, "result": {"authenticated": authed}})

            elif method == "set_quality":
                quality = params["quality"]
                out({"id": req_id, "result": {"ok": True}})

            elif method in HANDLERS:
                if not call_with_reconnect(session, session.check_login):
                    out({"id": req_id, "error": "Non authentifié"})
                    continue
                handler = HANDLERS[method]
                if method == "stream" and "quality" not in params:
                    params["quality"] = quality
                result = call_with_reconnect(
                    session, lambda: handler(session, **params)
                )
                out({"id": req_id, "result": result})

            else:
                out({"id": req_id, "error": f"Méthode inconnue : {method}"})

        except Exception as e:
            out({"id": req_id, "error": str(e)})


# ─── Commandes CLI (legacy) ──────────────────────────────────────────────────


def cli_auth_start():
    session, link_login, future = start_oauth(make_session())
    url = str(link_login.verification_uri_complete)
    code = str(link_login.user_code)
    out({"url": url, "code": code})
    try:
        future.result()
        save_session(session)
        out({"authenticated": True})
    except Exception as e:
        out({"authenticated": False, "error": str(e)})


def cli_auth_poll():
    # open_session falls back to tidalapi's credentials when the saved token
    # was not issued for the .env client_id.
    session = open_session()
    out({"authenticated": session.check_login()})


def cli_search(query, limit=20):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_search(session, query, limit))
    except Exception as e:
        out({"error": str(e)})


def cli_search_playlists(query, limit=20):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_search_playlists(session, query, limit))
    except Exception as e:
        out({"error": str(e)})


def cli_stream(track_id, quality="LOSSLESS"):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_stream(session, track_id, quality))
    except Exception as e:
        out({"error": str(e)})


def cli_cover(track_id):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_cover(session, track_id))
    except Exception as e:
        out({"error": str(e)})


def cli_playlists():
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_playlists(session))
    except Exception as e:
        out({"error": str(e)})


def cli_playlist_tracks(uuid):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_playlist_tracks(session, uuid))
    except Exception as e:
        out({"error": str(e)})


def cli_mixes():
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_mixes(session))
    except Exception as e:
        out({"error": str(e)})


def cli_mix_tracks(mix_id):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_mix_tracks(session, mix_id))
    except Exception as e:
        out({"error": str(e)})


def cli_favorite_tracks():
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_favorite_tracks(session))
    except Exception as e:
        out({"error": str(e)})


def cli_favorite_albums():
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_favorite_albums(session))
    except Exception as e:
        out({"error": str(e)})


def cli_lyrics(track_id):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_lyrics(session, track_id))
    except Exception as e:
        out({"error": str(e)})


def cli_album_tracks(album_id):
    session = open_session()
    if not session.check_login():
        out({"error": "Non authentifié"})
        return
    try:
        out(handle_album_tracks(session, album_id))
    except Exception as e:
        out({"error": str(e)})


# ─── Entry point ────────────────────────────────────────────────────────────

if __name__ == "__main__":
    if "--daemon" in sys.argv:
        run_daemon()
        sys.exit(0)

    args = sys.argv[1:]

    if not args:
        out({"error": "Usage : tidal.py <auth start|auth poll|search ...>"})
        sys.exit(1)

    match args:
        case ["auth", "start"]:
            cli_auth_start()
        case ["auth", "poll"]:
            cli_auth_poll()
        case ["search", query]:
            cli_search(query)
        case ["search", query, limit]:
            cli_search(query, int(limit))
        case ["search_playlists", query]:
            cli_search_playlists(query)
        case ["stream", track_id]:
            cli_stream(int(track_id))
        case ["stream", track_id, quality]:
            cli_stream(int(track_id), quality)
        case ["cover", track_id]:
            cli_cover(int(track_id))
        case ["playlists"]:
            cli_playlists()
        case ["playlist_tracks", uuid]:
            cli_playlist_tracks(uuid)
        case ["mixes"]:
            cli_mixes()
        case ["mix_tracks", mix_id]:
            cli_mix_tracks(mix_id)
        case ["fav_tracks"]:
            cli_favorite_tracks()
        case ["fav_albums"]:
            cli_favorite_albums()
        case ["lyrics", track_id]:
            cli_lyrics(int(track_id))
        case ["album_tracks", album_id]:
            cli_album_tracks(int(album_id))
        case _:
            out({"error": f"Commande inconnue : {args}"})
            sys.exit(1)

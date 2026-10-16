Fork of the original Tuidal for personal purpose.
- french interface (FR / EN / DE / RO)
- add some cool features — see the [Feature guide](#feature-guide)
- [x] new tab spectre (cava engine, bars or curve)
<img width="1031" height="513" alt="image" src="https://github.com/user-attachments/assets/0a64bbff-e7cf-4f93-8ab9-7d1601e9f386" />
- [x] preset / balance / ... based on mpv
<img width="873" height="643" alt="image" src="https://github.com/user-attachments/assets/864baf19-547a-49ff-95d4-7f88e9009076" />
- [x] Add background image behind the spectrum (kitty)
<img width="1271" height="829" alt="image" src="https://github.com/user-attachments/assets/079f0484-96b4-412b-aa71-14290e3f428b" />
- [x] split view (search | queue), live search, fuzzy filter, theme search
- [x] track / artist radio, playlist export (files, Tidal, HTTP upload)
- [x] system media controls (MPRIS on Linux, Now Playing on macOS)
# Tuidal

A terminal music player for Tidal, built with Rust and ratatui. Streams lossless audio directly in your terminal with album art support via the Kitty, Sixel or iTerm2 graphics protocols.

![tuidal screenshot](./img/2026-03-30-233237_hyprshot.png)
![tuidal screenshot](./img/2026-03-30-233249_hyprshot.png)
![tuidal screenshot](./img/2026-03-30-233255_hyprshot.png)
![tuidal screenshot](./img/2026-03-30-233310_hyprshot.png)

## Features

- 🎵 Stream FLAC lossless and HiRes audio
- 🖼️ Album art rendered inline (Kitty, Sixel or iTerm2 terminals; halfblocks fallback elsewhere)
- 🔍 Search tracks
- 📋 Queue management
- 📚 Library — playlists, mixes, favorites
- ⚡ Non-blocking UI with async background operations
- 🔑 OAuth Device Flow authentication (no password required)
- 📈 Real-time audio spectrum tab — cava-style bars or curve
- 📻 Track / artist radio — Tidal's native stations from any track (`r`) or its artist (`R`)
- 📤 Playlist export — queue, playlist, mix or album to M3U8 / CSV / JSON (metadata and Tidal links only), straight into a new Tidal playlist, or POSTed to a configurable URL
- 🪟 Split view — search on the left, queue on the right, with `h`/`l` to move focus, `S` to toggle, automatic fallback to tabs under 100 columns
- 🖼️ Optional background image behind the spectrum (album cover or random wallhaven.cc wallpaper): true under-the-text background on kitty; on Sixel/iTerm2 the spectrum is composited into the image (lower frame rate, Sixel slower)
- 🎛️ Effects tab: 10-band equalizer with presets, balance, bass/treble, stereo width, crossfeed, echo, compressor, volume normalization (requires mpv)
- 🎼 Theme search — find editorial playlists by genre or mood (trip hop, classical, lofi…)
- ⚡ fzf-style fuzzy filter on any list + live search-as-you-type
- 🎛️ System media controls — MPRIS on Linux (playerctl, GNOME, polybar…), media keys / Control Center / Now Playing on macOS

---

## Requirements

### System

- [Rust](https://rustup.rs/) (edition 2024)
- [mpv](https://mpv.io/) — audio playback
- [Python 3.10+](https://www.python.org/) with [tidalapi](https://github.com/EbbLabs/python-tidal)
- A graphics-capable terminal — for album art and the spectrum background (optional): [kitty](https://sw.kovidgoyal.net/kitty/), Ghostty (kitty protocol), iTerm2, WezTerm (iTerm2/Sixel). Other terminals fall back to halfblocks/no image.

### Python dependency

The Tidal API side lives in `tidal.py`, which needs the [tidalapi](https://github.com/EbbLabs/python-tidal) package. The recommended setup is a virtual environment in the project root, which tuidal picks up automatically (see [Python environment](#python-environment-venv)):

```bash
python3 -m venv .venv
.venv/bin/pip install tidalapi
```

---

## Installation

```bash
git clone https://github.com/VicMeGa/tuidal
cd tuidal
python3 -m venv .venv && .venv/bin/pip install tidalapi
cargo build --release
```

Copy `tidal.py` to the same directory as the binary:

```bash
cp tidal.py target/release/
```

Or run directly from the project root with `cargo run`.

> Note: `tidal.py` is looked up **next to the binary first**, then in the current directory. After pulling changes that touch `tidal.py`, copy it again, otherwise the binary keeps running the stale copy in `target/release/`.

### Python environment (venv)

tuidal never needs tidalapi installed system-wide. At startup it resolves the Python interpreter used to run `tidal.py` in this order:

1. `TUIDAL_PYTHON_PATH` — explicit interpreter path, if set and non-empty;
2. `$VIRTUAL_ENV/bin/python` — the venv currently activated in your shell;
3. `.venv/bin/python` next to `tidal.py` (i.e. in the project root, or next to the binary if you copied the venv there);
4. `.venv/bin/python` in the current working directory;
5. plain `python3` from `PATH`.

Typical workflows:

```bash
# A) Project-local venv, no activation needed (recommended)
python3 -m venv .venv
.venv/bin/pip install tidalapi
cargo run                      # finds .venv/ next to tidal.py

# B) Any venv, activated in the shell
python3 -m venv ~/.venvs/tuidal
source ~/.venvs/tuidal/bin/activate
pip install tidalapi
./target/release/tuidal        # uses $VIRTUAL_ENV

# C) Any interpreter, pinned explicitly (systemd units, scripts, NixOS...)
export TUIDAL_PYTHON_PATH="$HOME/.venvs/tuidal/bin/python"
./target/release/tuidal
```

To upgrade tidalapi later: `.venv/bin/pip install -U tidalapi`. To check which interpreter and script will be used, run `.venv/bin/python tidal.py auth poll` from the project root — it must print `{"authenticated": ...}` without an import error.

### API credentials

You can use your own Tidal API credentials by creating a `.env` file (at the project root or next to `tidal.py`) containing:

```
id=YOUR_TIDAL_CLIENT_ID
secret=YOUR_TIDAL_CLIENT_SECRET
```

Alternatively, set the `TIDAL_CLIENT_ID` and `TIDAL_CLIENT_SECRET` environment variables (they take precedence over the `.env` file). If neither is provided — or if Tidal rejects your credentials (e.g. they are not enabled for the device flow) — tuidal automatically falls back to tidalapi's built-in credentials. The `.env` file is git-ignored.

---

## NixOS

Enter a development shell with all dependencies:

```bash
nix-shell -p rustup mpv pkg-config openssl
```

For Python with tidalapi, use a virtual environment (see below) or add it to your shell:

```nix
# shell.nix
{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustup
    mpv
    pkg-config
    openssl
    python3
  ];
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
}
```

Then create the virtual environment inside the shell (recommended on NixOS, where `pip install` into the system Python is not possible):

```bash
python3 -m venv .venv
.venv/bin/pip install tidalapi
```

tuidal detects `.venv/` automatically; see [Python environment](#python-environment-venv) for the other options.

### Settings (`~/.config/tuidal/settings.json`)

Written automatically whenever a persisted value changes; every key is optional.

| Key | Values | Default | Notes |
|-----|--------|---------|-------|
| `lang` | `fr` / `en` / `de` / `ro` | `fr` | `Alt+L` cycles |
| `quality` | `HI_RES_LOSSLESS` / `LOSSLESS` / `HIGH` | `LOSSLESS` | keys `1` / `2` / `3` |
| `volume` | 0–100 | 85 | |
| `audio_fx` | object | neutral | Effects tab state |
| `layout` | `split` / `tabs` | `split` | `S` toggles; split needs ≥ 100 columns |
| `search_limit` | integer | 50 | max results per track/theme search; radio uses at least 50 |
| `spectrum_style` | `bars` / `curve` | `bars` | `v` toggles |
| `spectrum_bg` | `none` / `cover` / `wallhaven` | `cover` | `b` cycles |
| `wallhaven_query` | string | `abstract` | wallhaven.cc search used by `B` |
| `export_dir` | path | unset | see [Playlist export](#playlist-export) |
| `export_upload_url` | URL | unset | enables the `u` export destination |
| `export_upload_format` | `json` / `csv` / `m3u8` | `json` | body format of the upload |
| `export_upload_token` | string | unset | sent as `Authorization: Bearer`; never displayed |

Environment variables override their settings counterpart: `TUIDAL_EXPORT_DIR`,
`TUIDAL_EXPORT_UPLOAD_URL`, `TUIDAL_EXPORT_UPLOAD_FORMAT`, `TUIDAL_EXPORT_UPLOAD_TOKEN`.

### Python Executable Path (`TUIDAL_PYTHON_PATH`)

Overrides the interpreter used to run `tidal.py`, taking precedence over the automatic venv detection described in [Python environment](#python-environment-venv).

```bash
export TUIDAL_PYTHON_PATH="/home/youruser/.venvs/tuidal/bin/python"
cargo run # or ./target/release/tuidal
```

### Image Protocol Override (`TUIDAL_IMAGE_PROTOCOL`)

tuidal auto-detects the terminal graphics protocol. To force one (or to enable images when detection fails), set `TUIDAL_IMAGE_PROTOCOL` to `kitty`, `sixel`, `iterm2` or `halfblocks` (case-insensitive). It wins over detection, including the kitty auto-detection.

```bash
TUIDAL_IMAGE_PROTOCOL=sixel ./target/release/tuidal
```

### Background Debug Log (`TUIDAL_BG_DEBUG`)

Set `TUIDAL_BG_DEBUG=1` to append background/picker diagnostics to `/tmp/tuidal-bg.log`.

---

## Tidal Authentication

tuidal uses the **OAuth Device Flow** — no password is stored or required.

### Option A — from inside the TUI

1. Run tuidal and press `L`
2. A URL and code will appear on screen
3. Open the URL in your browser, log in to Tidal, and enter the code
4. The session is saved automatically to `~/.config/tidal-tui/tidalapi_session.json`

### Option B — from the command line (recommended for first login)

```bash
.venv/bin/python tidal.py auth start   # or: python3 tidal.py auth start
```

Output:
```
{"url": "link.tidal.com/XXXXX", "code": "XXXXX"}
```

Open the URL in your browser, log in to Tidal, and enter the displayed code. The process will block until you authorize, then save the session automatically.

### Check if you are logged in

```bash
.venv/bin/python tidal.py auth poll
```

If logged in:
```json
{"authenticated": true}
```

If not logged in:
```json
{"authenticated": false, "pending": false}
```

Sessions are reused automatically on subsequent runs. If the session expires, run `auth start` again or press `L` inside the TUI.

---

## Usage

```
cargo run
# or after build:
./target/release/tuidal
```

### Keybindings

| Key | Action |
|-----|--------|
| `/` or `s` | Search |
| `Enter` | Play selected |
| `Space` | Pause / Resume |
| `n` / `p` | Next / Previous track |
| `←` / `→` | Seek backward / forward |
| `+` / `-` | Volume up / down |
| `Tab` | Cycle tabs (Search → Queue → Now → Spectrum → Effects → Library) |
| `i` | Load library (on Library tab) |
| `g` | Search playlists by musical theme (e.g. trip hop, classical) |
| `f` | Fuzzy-filter the current list (Esc clears) |
| `h` / `l` | Focus search / queue pane (split view) |
| `S` | Toggle split view ↔ tabs |
| `r` | Start track radio from selected/playing track |
| `R` | Start artist radio (except on Effects tab) |
| `e` | Export queue / playlist / mix / album — `1/2/3` M3U8/CSV/JSON, `t` new Tidal playlist, `u` upload |
| `j` / `k` or `↓` / `↑` | Navigate list |
| `1` | Quality: HiRes FLAC 24bit |
| `2` | Quality: FLAC 16bit/44.1kHz |
| `3` | Quality: AAC 320kbps |
| `L` | Login to Tidal |
| `Alt+L` | Cycle language (FR → EN → DE → RO) |
| `q` | Quit |

#### Effects tab

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Select control |
| `←` / `→` | Adjust value |
| `Enter` | Toggle / reset to neutral |
| `[` / `]` | Previous / next EQ preset |
| `R` | Reset all effects |

#### Spectrum tab

| Key | Action |
|-----|--------|
| `b` | Cycle background (none → cover → wallhaven) |
| `B` | Fetch a new random wallpaper (wallhaven source) |
| `v` | Toggle spectrum style (bars ↔ curve) |

The spectrum engine is a Rust port of [cava](https://github.com/karlstav/cava)'s
cavacore (MIT, © Karl Stavestrand): logarithmic band splitting, autosens,
gravity falloff and noise reduction.

---

## Feature guide

### Split view (search | queue)

On terminals at least 100 columns wide, the *Search* and *Queue* tabs share the
body: results on the left (60 %), queue on the right (40 %). Each pane keeps its
own selection, so looking up a track never loses your place in the queue.

- The focused pane has an accent border and title; the other pane stays dimmed
  with its selected row still highlighted.
- `h` focuses the search pane, `l` the queue pane; `Tab` keeps cycling through
  all tabs as before (which also moves the focus left → right).
- `Enter`, `a`, `f`, `r`/`R` and `e` act on the focused pane. `Enter` on a result
  appends it to the queue and plays it — it shows up on the right with ▶.
- The queue title shows the count and total duration, e.g. `Queue (12 · 48:12)`.
- `S` switches to the classic one-tab layout (persisted as `layout`); narrower
  terminals fall back to tabs automatically.

### Live search and fuzzy filter

- **Search as you type** (`/`): the query is sent 300 ms after the last keystroke
  once it has at least 3 characters. Only one request is in flight at a time and
  superseded replies are dropped, so the list always matches the latest text.
  `Enter` re-runs the search only if the text changed; `Esc` closes the prompt
  without discarding a reply in flight. A `⟳` next to the cursor means a request
  is running.
- **Fuzzy filter** (`f`): fzf-style, instant, local to the focused list (search
  results, queue, playlists/mixes, favourite albums). Matching is powered by
  [nucleo-matcher](https://github.com/helix-editor/nucleo) (the Helix matcher):
  case-insensitive, scattered characters allowed (`mss atk` finds *Massive
  Attack*), results sorted by score with matched characters highlighted. The bar
  shows `shown/total`. `↑`/`↓` or `Ctrl-j`/`Ctrl-k` move inside the filtered
  list, `Enter` performs the usual action and keeps the filter, `Esc` clears it
  (also from normal mode).
- `search_limit` (default 50) caps the number of remote results.

### Theme search

`g` opens a *theme* prompt (`trip hop`, `classical`, `lofi jazz`…). Tidal's own
genre endpoint only exposes ~20 coarse genres, so tuidal instead searches
editorial playlists by free text, which matches moods and sub-genres very well
(*Trip Hop Treats*, *Massive Attack Essentials*, *Classical Focus*…). Results
land on the Library tab under the title `Playlists: <theme>`; `Enter` loads a
playlist into the queue. `i` reloads your own library and leaves theme mode.
Live search applies to the theme prompt too.

### Track and artist radio

Tidal's native stations, straight from the keyboard:

- `r` starts a radio from the track under the cursor (search or queue pane) or,
  from any other tab, from the track currently playing.
- `R` starts a radio from that track's main artist (except on the Effects tab,
  where `R` resets the effects).

The station (50 tracks, or `search_limit` if larger) **replaces the queue** and
playback starts on its first track. Also available on the HTTP API:
`POST /track-radio`, `POST /artist-radio`.

### Playlist export

`e` opens the export popup for the **queue** or, on the Library tab, for the
**selected playlist / mix / favourite album** (its tracks are fetched first).
Destinations:

| Key | Destination | Details |
|-----|-------------|---------|
| `1` / `m` | M3U8 file | `#EXTM3U` + `#EXTINF` lines with `https://tidal.com/browse/track/<id>` web links |
| `2` / `c` | CSV file | `position,title,artists,album,duration_seconds,isrc,tidal_id,url`, RFC 4180 quoting |
| `3` / `j` | JSON file | full metadata (`title`, `exported_at`, `tracks[]` with artists, album, ISRC, url) |
| `4` / `t` | Tidal playlist | creates `<title> (tuidal)` (or `tuidal queue <timestamp>`) in your Tidal library and adds the tracks |
| `5` / `u` | HTTP upload | `POST` of the rendered export to `export_upload_url` (shown only when configured) |

Files are named `<slug>-<YYYYMMDD-HHMMSS>.<ext>` and written to `export_dir`,
else `$TUIDAL_EXPORT_DIR`, else `~/Music/tuidal`, else `~/.config/tuidal/exports`.
Exports contain **metadata and Tidal web links only** — never stream URLs or
audio. The ISRC comes from Tidal and makes re-import into other services easy.

Upload request shape, for writing a receiver:

```
POST <export_upload_url>
Content-Type: application/json | text/csv; charset=utf-8 | audio/x-mpegurl; charset=utf-8
X-Tuidal-Title: <playlist title>
X-Tuidal-Format: json | csv | m3u8
Authorization: Bearer <export_upload_token>      (only when a token is set)

<rendered export>
```

Any 2xx reply is a success (`✓ Uploaded to <host> (HTTP 201)`); the timeout is 15 s.

### System media controls

- **Linux — MPRIS** (`org.mpris.MediaPlayer2.tuidal`): play/pause, next,
  previous, stop, seek, volume, shuffle/loop status and metadata including the
  cover (`mpris:artUrl`). Works with `playerctl`, GNOME Shell, KDE, polybar,
  waybar, etc.
- **macOS — Now Playing / media keys / Control Center** via
  `MPRemoteCommandCenter`: the keyboard media keys, the Control Center widget
  and AirPods controls drive tuidal, and the track, artist, album and cover are
  shown in Now Playing. tuidal registers as a regular app (a Dock icon appears
  while it runs); mpv is started with `--input-media-keys=no` so it does not
  compete for the keys. A hidden `tuidal --media-probe` flag registers a fake
  session for 60 s and prints every received command — handy to check the
  wiring.

### Local HTTP API

tuidal listens on `http://127.0.0.1:7837` (loopback only) so scripts, status
bars and hotkey daemons can drive it:

| Method | Route | Effect |
|--------|-------|--------|
| GET | `/status` | state, track, position, volume, shuffle/repeat, queue |
| POST | `/play-pause`, `/next`, `/previous` | transport |
| POST | `/seek-forward`, `/seek-backward` | ±10 s |
| POST | `/volume-up`, `/volume-down`, `/volume?level=<0-100>` | volume |
| POST | `/shuffle`, `/repeat` | toggle shuffle / cycle repeat |
| POST | `/track-radio`, `/artist-radio` | start a radio from the selected/playing track |
| POST | `/play-track` (JSON body `{id,title,artist,album,duration}`) | play a given track |
| POST | `/just-play?q=<text>` | search and play the first match |
| GET | `/search?q=<text>&limit=<n>` | track search |
| GET | `/queue`, `/library`, `/library/favorites`, `/library/favorite-albums` | lists |
| GET | `/library/playlist/{uuid}`, `/library/mix/{id}`, `/library/album/{id}` | tracks of an item |

```bash
curl -X POST localhost:7837/play-pause
curl -X POST 'localhost:7837/just-play?q=teardrop massive attack'
curl localhost:7837/status | jq .title
```

### Spectrum tab

The analyser is a Rust port of [cava](https://github.com/karlstav/cava)'s
cavacore (MIT, © Karl Stavestrand): two FFT resolutions (bass / mid-treble),
logarithmic band distribution, auto-sensitivity, gravity fall-off and integral
smoothing, fed by a second download of the stream decoded with symphonia and
kept in sync with mpv's playback position.

- `v` toggles **bars** (default: eighth-block glyphs, three-colour gradient,
  decaying peak markers) and **curve** (Braille line chart).
- `b` cycles the background: none, the album cover, or a random wallhaven.cc
  wallpaper (`B` fetches a new one). On kitty the image sits under the text; on
  Sixel/iTerm2 the bars/curve are drawn into the image itself.
- `TUIDAL_BG_DEBUG=1` traces the engine state (bar count, sensitivity, fps,
  samples fed) and the background pipeline to `/tmp/tuidal-bg.log`.

### Robustness and performance

- Dropped keep-alive connections (`Connection reset by peer` after a long idle
  while a track plays) are retried transparently: every tidalapi session gets a
  `urllib3` retry policy and the daemon flushes its connection pool and retries
  once on a reset.
- The Python daemon serves API calls on a small thread pool: replies are matched
  by request id, so a slow search never delays the next track's stream URL.

---

## Project Structure

```
tuidal/
├── src/
│   ├── main.rs          # Entry point, key handling, event loop (macOS: AppKit loop on the main thread)
│   ├── app.rs           # Application state, background tasks, settings
│   ├── ui.rs            # Terminal UI rendering (ratatui): split view, lists, popups, spectrum
│   ├── tidal.rs         # Rust client for the Python daemon (JSON-RPC over stdin/stdout)
│   ├── player.rs        # mpv subprocess wrapper (IPC socket)
│   ├── api.rs           # Local HTTP API (axum)
│   ├── mpris.rs         # MPRIS D-Bus server (Linux)
│   ├── media_macos.rs   # Now Playing / media keys (macOS, souvlaki)
│   ├── fuzzy.rs         # fzf-style matcher (nucleo-matcher)
│   ├── export.rs        # M3U8 / CSV / JSON rendering, file write, HTTP upload
│   ├── spectrum.rs      # Stream re-download + decoding, streaming adapter over cavacore
│   ├── cavacore.rs      # Port of cava's cavacore (MIT)
│   ├── composite_bg.rs  # Pixel rendering of the spectrum into the background (Sixel/iTerm2)
│   ├── kitty_bg.rs      # kitty graphics protocol background placement
│   ├── audio_fx.rs      # Equalizer / effects model → mpv filter chain
│   └── i18n.rs          # UI strings (FR, EN, DE, RO)
├── tidal.py             # Python daemon bridging to tidalapi
└── Cargo.toml
```

---

## How It Works

tuidal uses a two-process architecture:

```
Rust TUI  ──subprocess──►  tidal.py  ──►  Tidal API
(ratatui)                  (tidalapi)
```

`tidal.py` handles all Tidal API calls and OAuth authentication via [tidalapi](https://github.com/EbbLabs/python-tidal). It runs as a persistent daemon and communicates with the Rust process via stdin/stdout as JSON-RPC lines; API calls are served by a small thread pool, so replies can come back out of order (matched by `id`) and a slow search never delays the next track's stream URL. This approach avoids dealing with Tidal's unofficial API credentials directly in Rust, and benefits from tidalapi's maintained session handling.

---

## Cargo.toml dependencies

```toml
[dependencies]
ratatui       = "0.29"
crossterm     = { version = "0.28", features = ["event-stream"] }
tokio         = { version = "1", features = ["full"] }
serde         = { version = "1", features = ["derive"] }
serde_json    = "1"
anyhow        = "1"
rodio         = "0.20"
dirs          = "5"
libc          = "0.2"
ratatui-image = { version = "6", default-features = false }
image         = "0.25"
reqwest = { version = "0.12", features = ["json"] }
open = "5"
axum  = "0.8"
zbus  = "5"
symphonia = { version = "0.5", features = ["isomp4", "aac"] }
rustfft   = "6"
base64    = "0.22"
nucleo-matcher = "0.3"

[target.'cfg(target_os = "macos")'.dependencies]
souvlaki = "0.8.3"   # media keys / Control Center / Now Playing
cocoa    = "0.24"    # AppKit run loop on the main thread (souvlaki dep too)
objc     = "0.2"
```

---

## Notes

- Audio quality depends on your Tidal subscription.
- The spectrum tab downloads the stream a second time for analysis.
- Wallhaven images are fetched at runtime from wallhaven.cc (SFW, random, query configurable via `wallhaven_query` in `settings.json`). If HiRes is unavailable for a track, tuidal automatically falls back to Lossless, then High.
- Album art uses the Kitty, Sixel or iTerm2 protocols (halfblocks fallback elsewhere). The spectrum background is a true under-the-text image on kitty only; on Sixel/iTerm2 it is composited into an image at a lower frame rate.
- This project uses tidalapi, an unofficial Tidal API client. Use it only for personal use and in accordance with Tidal's terms of service.

---

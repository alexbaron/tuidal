
Fork of the original Tuidal for personal purpose.
- french interface
- add some cool features
- [x] new tab spectre
<img width="1031" height="513" alt="image" src="https://github.com/user-attachments/assets/0a64bbff-e7cf-4f93-8ab9-7d1601e9f386" />
- [x] preset / balance / ... based on mpv
<img width="873" height="643" alt="image" src="https://github.com/user-attachments/assets/864baf19-547a-49ff-95d4-7f88e9009076" />
- [x] Add background image behind the spectrum (kitty)
<img width="1271" height="829" alt="image" src="https://github.com/user-attachments/assets/079f0484-96b4-412b-aa71-14290e3f428b" />
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
- 📈 Real-time audio spectrum (FFT) tab
- 🖼️ Optional background image behind the spectrum (album cover or random wallhaven.cc wallpaper): true under-the-text background on kitty; on Sixel/iTerm2 the spectrum is composited into the image (lower frame rate, Sixel slower)
- 🎛️ Effects tab: 10-band equalizer with presets, balance, bass/treble, stereo width, crossfeed, echo, compressor, volume normalization (requires mpv)
- 🎼 Theme search — find editorial playlists by genre or mood (trip hop, classical, lofi…)
- 🎛️ System media controls — MPRIS on Linux (playerctl, GNOME, polybar…), media keys / Control Center / Now Playing on macOS

---

## Requirements

### System

- [Rust](https://rustup.rs/) (edition 2024)
- [mpv](https://mpv.io/) — audio playback
- [Python 3.10+](https://www.python.org/) with [tidalapi](https://github.com/EbbLabs/python-tidal)
- A graphics-capable terminal — for album art and the spectrum background (optional): [kitty](https://sw.kovidgoyal.net/kitty/), Ghostty (kitty protocol), iTerm2, WezTerm (iTerm2/Sixel). Other terminals fall back to halfblocks/no image.

### Python dependency

```bash
pip install tidalapi
```

---

## Installation

```bash
git clone https://github.com/VicMeGa/tuidal
cd tuidal
cargo build --release
```

Copy `tidal.py` to the same directory as the binary:

```bash
cp tidal.py target/release/
```

Or run directly from the project root with `cargo run`.

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

Then install tidalapi inside the shell:

```bash
pip install tidalapi --break-system-packages
```

Or use a virtual environment (recommended on NixOS):

```bash
python3 -m venv ~/my_python
~/my_python/bin/pip install tidalapi
```

### Settings (`~/.config/tuidal/settings.json`)

Persisted keys include `lang`, `quality`, `volume`, `audio_fx`, `spectrum_bg`
(`"none"` | `"cover"` | `"wallhaven"`, default `"cover"`) and `wallhaven_query`
(default `"abstract"`).

### Python Executable Path (`TUIDAL_PYTHON_PATH`)

By default, tuidal uses `python3` as the executable. You can specify a custom Python interpreter **without modifying the code** by setting the environment variable `TUIDAL_PYTHON_PATH` before running tuidal.

**Example:**

```bash
export TUIDAL_PYTHON_PATH="/home/youruser/my_python/bin/python3"
cargo run # or ./target/release/tuidal
```

If not set, tuidal defaults to running `python3` from your system PATH. This makes tuidal portable across environments, virtualenvs, and OS distributions.

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
python3 tidal.py auth start
```

Output:
```
{"url": "link.tidal.com/XXXXX", "code": "XXXXX"}
```

Open the URL in your browser, log in to Tidal, and enter the displayed code. The process will block until you authorize, then save the session automatically.

### Check if you are logged in

```bash
python3 tidal.py auth poll
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

---

## Project Structure

```
tuidal/
├── src/
│   ├── main.rs       # Entry point, event loop
│   ├── app.rs        # Application state and background tasks
│   ├── ui.rs         # Terminal UI rendering (ratatui)
│   ├── tidal.rs      # Rust client — calls tidal.py as subprocess
│   └── player.rs     # mpv subprocess wrapper
├── tidal.py          # Python bridge to tidalapi
└── Cargo.toml
```

---

## How It Works

tuidal uses a two-process architecture:

```
Rust TUI  ──subprocess──►  tidal.py  ──►  Tidal API
(ratatui)                  (tidalapi)
```

`tidal.py` handles all Tidal API calls and OAuth authentication via [tidalapi](https://github.com/EbbLabs/python-tidal). It communicates with the Rust process via stdin/stdout as JSON. This approach avoids dealing with Tidal's unofficial API credentials directly in Rust, and benefits from tidalapi's maintained session handling.

---

## Cargo.toml dependencies

```toml
[dependencies]
ratatui       = "0.29"
crossterm     = { version = "0.28", features = ["event-stream"] }
tokio         = { version = "1", features = ["full"] }
reqwest       = { version = "0.12", features = ["json", "blocking"] }
serde         = { version = "1", features = ["derive"] }
serde_json    = "1"
anyhow        = "1"
libc          = "0.2"
ratatui-image = "6"
image         = "0.25"
```

---

## Notes

- Audio quality depends on your Tidal subscription.
- The spectrum tab downloads the stream a second time for analysis.
- Wallhaven images are fetched at runtime from wallhaven.cc (SFW, random, query configurable via `wallhaven_query` in `settings.json`). If HiRes is unavailable for a track, tuidal automatically falls back to Lossless, then High.
- Album art uses the Kitty, Sixel or iTerm2 protocols (halfblocks fallback elsewhere). The spectrum background is a true under-the-text image on kitty only; on Sixel/iTerm2 it is composited into an image at a lower frame rate.
- This project uses tidalapi, an unofficial Tidal API client. Use it only for personal use and in accordance with Tidal's terms of service.

---

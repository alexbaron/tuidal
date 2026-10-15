//! audio_fx.rs — Audio effects applied live through mpv's `af` filter chain.
//!
//! Filters are sent either at launch (`--af=<AF>`) or hot over the IPC socket
//! (`{"command":["af","set","<AF>"]}` / `{"command":["af","clr",""]}`).

use serde::{Deserialize, Serialize};

/// Center frequency (Hz) of each equalizer band.
pub const EQ_FREQS: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/// Ordered list of controls shown in the Effects tab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FxControl {
    Enabled,
    Preset,
    Balance,
    Bass,
    Treble,
    Eq(usize),
    StereoWidth,
    Crossfeed,
    Echo,
    Compressor,
    Normalize,
}

impl FxControl {
    pub const ALL: [FxControl; 20] = [
        FxControl::Enabled,
        FxControl::Preset,
        FxControl::Balance,
        FxControl::Bass,
        FxControl::Treble,
        FxControl::Eq(0),
        FxControl::Eq(1),
        FxControl::Eq(2),
        FxControl::Eq(3),
        FxControl::Eq(4),
        FxControl::Eq(5),
        FxControl::Eq(6),
        FxControl::Eq(7),
        FxControl::Eq(8),
        FxControl::Eq(9),
        FxControl::StereoWidth,
        FxControl::Crossfeed,
        FxControl::Echo,
        FxControl::Compressor,
        FxControl::Normalize,
    ];
}

/// Equalizer presets: (name, 10 gain values in dB).
pub const PRESETS: &[(&str, [i8; 10])] = &[
    ("Flat", [0; 10]),
    ("Bass Boost", [6, 5, 4, 2, 0, 0, 0, 0, 0, 0]),
    ("Treble Boost", [0, 0, 0, 0, 0, 0, 2, 4, 5, 6]),
    ("Rock", [5, 3, -1, -2, -1, 1, 3, 4, 4, 4]),
    ("Pop", [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2]),
    ("Jazz", [3, 2, 1, 2, -1, -1, 0, 1, 2, 3]),
    ("Classical", [4, 3, 2, 1, -1, -1, 0, 2, 3, 3]),
    ("Vocal", [-2, -3, -1, 1, 3, 4, 3, 1, 0, -1]),
    ("Electronic", [5, 4, 1, 0, -2, 2, 1, 2, 4, 5]),
    ("Loudness", [6, 4, 0, 0, -2, 0, -1, 0, 3, 5]),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioFx {
    /// Master bypass switch.
    pub enabled: bool,
    /// -100 (left) .. 100 (right).
    pub balance: i8,
    /// -15..15 dB.
    pub bass: i8,
    /// -15..15 dB.
    pub treble: i8,
    /// -12..12 dB per `EQ_FREQS` band.
    pub eq: [i8; 10],
    /// 0..200 %, 100 = neutral.
    pub stereo_width: u8,
    pub crossfeed: bool,
    pub echo: bool,
    pub compressor: bool,
    pub normalize: bool,
}

impl Default for AudioFx {
    fn default() -> Self {
        Self {
            enabled: true,
            balance: 0,
            bass: 0,
            treble: 0,
            eq: [0; 10],
            stereo_width: 100,
            crossfeed: false,
            echo: false,
            compressor: false,
            normalize: false,
        }
    }
}

impl AudioFx {
    /// True when no parameter would change the sound.
    pub fn is_neutral(&self) -> bool {
        self.balance == 0
            && self.bass == 0
            && self.treble == 0
            && self.eq.iter().all(|g| *g == 0)
            && self.stereo_width == 100
            && !self.crossfeed
            && !self.echo
            && !self.compressor
            && !self.normalize
    }

    /// mpv lavfi filters for every non-neutral parameter, in chain order.
    pub fn active_filters(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.balance != 0 {
            let l = (1.0 - self.balance as f32 / 100.0).min(1.0);
            let r = (1.0 + self.balance as f32 / 100.0).min(1.0);
            f.push(format!("pan=stereo|c0={l:.3}*c0|c1={r:.3}*c1"));
        }
        if self.bass != 0 {
            f.push(format!("bass=g={}", self.bass));
        }
        if self.treble != 0 {
            f.push(format!("treble=g={}", self.treble));
        }
        for (i, g) in self.eq.iter().enumerate() {
            if *g != 0 {
                f.push(format!("equalizer=f={}:t=o:w=1:g={g}", EQ_FREQS[i]));
            }
        }
        if self.stereo_width != 100 {
            f.push(format!("extrastereo=m={:.2}", self.stereo_width as f32 / 100.0));
        }
        if self.crossfeed {
            f.push("crossfeed=strength=0.5".to_string());
        }
        if self.echo {
            f.push("aecho=0.8:0.6:200:0.3".to_string());
        }
        if self.compressor {
            f.push("acompressor=threshold=-18dB:ratio=3:attack=20:release=250".to_string());
        }
        if self.normalize {
            f.push("dynaudnorm=f=250:g=15".to_string());
        }
        f
    }

    /// mpv `--af` value; empty when disabled or fully neutral.
    pub fn to_mpv_af(&self) -> String {
        if !self.enabled || self.is_neutral() {
            return String::new();
        }
        let filters = self.active_filters();
        if filters.is_empty() {
            String::new()
        } else {
            format!("lavfi=[{}]", filters.join(","))
        }
    }

    pub fn apply_preset(&mut self, idx: usize) {
        if let Some((_, eq)) = PRESETS.get(idx) {
            self.eq = *eq;
        }
    }

    /// Index of the preset matching `eq` exactly, if any.
    pub fn matching_preset(&self) -> Option<usize> {
        PRESETS.iter().position(|(_, eq)| *eq == self.eq)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// JSON command sent over the mpv IPC socket to set (or clear) the af chain.
pub fn af_ipc_command(af: &str) -> String {
    if af.is_empty() {
        serde_json::json!({"command": ["af", "clr", ""]}).to_string()
    } else {
        serde_json::json!({"command": ["af", "set", af]}).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert_eq!(AudioFx::default().to_mpv_af(), "");
    }

    #[test]
    fn disabled_is_empty() {
        let mut fx = AudioFx::default();
        fx.enabled = false;
        fx.eq[0] = 5;
        assert_eq!(fx.to_mpv_af(), "");
    }

    #[test]
    fn balance_pan() {
        let mut fx = AudioFx::default();
        fx.balance = 50;
        assert!(fx.to_mpv_af().contains("pan=stereo|c0=0.500*c0|c1=1.000*c1"));
        fx.balance = -100;
        assert!(fx.to_mpv_af().contains("c0=1.000*c0|c1=0.000*c1"));
    }

    #[test]
    fn single_eq_band() {
        let mut fx = AudioFx::default();
        fx.eq[5] = 3;
        assert_eq!(fx.to_mpv_af(), "lavfi=[equalizer=f=1000:t=o:w=1:g=3]");
    }

    #[test]
    fn full_chain_order() {
        let mut fx = AudioFx::default();
        fx.balance = 20;
        fx.bass = 3;
        fx.treble = -2;
        fx.eq = [1; 10];
        fx.stereo_width = 150;
        fx.crossfeed = true;
        fx.echo = true;
        fx.compressor = true;
        fx.normalize = true;
        let af = fx.to_mpv_af();
        assert!(af.starts_with("lavfi=[pan="));
        assert!(af.ends_with("dynaudnorm=f=250:g=15]"));
        let order = [
            "pan=", "bass=", "treble=", "equalizer=", "extrastereo=", "crossfeed=", "aecho=",
            "acompressor=", "dynaudnorm=",
        ];
        let pos: Vec<usize> = order.iter().map(|k| af.find(k).unwrap()).collect();
        assert!(pos.windows(2).all(|w| w[0] < w[1]), "bad order: {af}");
    }

    #[test]
    fn preset_roundtrip() {
        let mut fx = AudioFx::default();
        fx.apply_preset(1);
        assert_eq!(fx.matching_preset(), Some(1));
    }

    /// Requires a running mpv with the IPC socket:
    ///   mpv --no-config --ao=null --really-quiet \
    ///     --input-ipc-server=/tmp/tuidal-mpv.sock --loop=inf \
    ///     "av://lavfi:sine=frequency=440:duration=60" &
    /// Run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn ipc_af_roundtrip_with_running_mpv() {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        const SOCKET: &str = "/tmp/tuidal-mpv.sock";
        if !std::path::Path::new(SOCKET).exists() {
            eprintln!("skipped: mpv socket not present");
            return;
        }
        let mut fx = AudioFx::default();
        fx.bass = 3;
        fx.eq[0] = 4;
        let af = fx.to_mpv_af();
        let cmd = af_ipc_command(&af);

        let mut stream = UnixStream::connect(SOCKET).unwrap();
        stream.write_all(cmd.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream
            .write_all(br#"{"command":["get_property","af"]}"#)
            .unwrap();
        stream.write_all(b"\n").unwrap();
        // mpv interleaves async events with command replies: read lines until
        // one carries a "request_id" (i.e. an actual reply), or the timeout.
        let mut resp = String::new();
        let mut reader = std::io::BufReader::new(&mut stream);
        for _ in 0..10 {
            let mut line = String::new();
            match std::io::BufRead::read_line(&mut reader, &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    resp.push_str(&line);
                    if line.contains("\"request_id\"") && !line.contains("\"data\":null") {
                        break;
                    }
                }
            }
        }
        assert!(resp.contains("bass=g=3"), "mpv af response: {resp}");
    }
}

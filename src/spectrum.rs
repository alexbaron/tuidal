//! spectrum.rs — Parallel audio analysis for the spectrum tab.
//!
//! mpv/ffplay plays the stream in an external process, so we cannot tap its
//! audio. Instead we download the same stream URL a second time, decode it
//! with symphonia into a shared mono buffer, and run an FFT (rustfft) at the
//! current playback position.

use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Sampling rate of the analysis buffer is kept near this value by decimation.
const ANALYSIS_RATE: u32 = 24_000;
/// Stop decoding after this much audio; it is plenty for visualization.
const MAX_SECONDS: u64 = 15 * 60;
/// Push decoded samples into the shared buffer at most this often (seconds of audio).
const PUSH_CHUNK_SECS: f64 = 0.25;

// ─── GrowingBuffer: a media source filled progressively by the download ─────

struct GrowingInner {
    data: Vec<u8>,
    complete: bool,
    total_len: Option<u64>,
    cancelled: bool,
}

/// Shared byte buffer: a download task pushes chunks while the decoder reads.
pub struct GrowingBuffer {
    inner: Mutex<GrowingInner>,
    cv: Condvar,
}

impl GrowingBuffer {
    fn new() -> Self {
        Self {
            inner: Mutex::new(GrowingInner {
                data: Vec::new(),
                complete: false,
                total_len: None,
                cancelled: false,
            }),
            cv: Condvar::new(),
        }
    }

    fn push(&self, chunk: &[u8]) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.data.extend_from_slice(chunk);
            self.cv.notify_all();
        }
    }

    fn finish(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.complete = true;
            self.cv.notify_all();
        }
    }

    fn cancel(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.cancelled = true;
            self.cv.notify_all();
        }
    }

    fn set_total_len(&self, total_len: Option<u64>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.total_len = total_len;
        }
    }
}

/// Read/Seek view over a GrowingBuffer. Reads block until data arrives.
pub struct GrowingReader {
    buf: Arc<GrowingBuffer>,
    pos: u64,
}

impl GrowingReader {
    fn new(buf: Arc<GrowingBuffer>) -> Self {
        Self { buf, pos: 0 }
    }
}

impl Read for GrowingReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let mut inner = self
            .buf
            .inner
            .lock()
            .map_err(|_| std::io::Error::other("lock poisoned"))?;
        loop {
            if self.pos < inner.data.len() as u64 {
                let avail = inner.data.len() as u64 - self.pos;
                let n = avail.min(out.len() as u64) as usize;
                let start = self.pos as usize;
                out[..n].copy_from_slice(&inner.data[start..start + n]);
                self.pos += n as u64;
                return Ok(n);
            }
            if inner.complete || inner.cancelled {
                return Ok(0);
            }
            inner = self
                .buf
                .cv
                .wait(inner)
                .map_err(|_| std::io::Error::other("lock poisoned"))?;
        }
    }
}

impl Seek for GrowingReader {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let inner = self
            .buf
            .inner
            .lock()
            .map_err(|_| std::io::Error::other("lock poisoned"))?;
        let new_pos = match from {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(off) => {
                let total = inner.total_len.ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "SeekFrom::End without known total length",
                    )
                })?;
                total as i64 + off
            }
            SeekFrom::Current(off) => self.pos as i64 + off,
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        // Seeking past received data is allowed: the next read blocks until
        // the download catches up (e.g. M4A with moov atom at end of file).
        self.pos = new_pos as u64;
        Ok(self.pos)
    }
}

impl MediaSource for GrowingReader {
    fn is_seekable(&self) -> bool {
        self.buf
            .inner
            .lock()
            .map(|i| i.total_len.is_some())
            .unwrap_or(false)
    }

    fn byte_len(&self) -> Option<u64> {
        self.buf.inner.lock().ok().and_then(|i| i.total_len)
    }
}

// ─── AnalysisBuffer: decoded mono samples, progressively filled ─────────────

#[derive(Debug)]
pub enum AnalysisState {
    Pending,
    Ready,
    /// Failure reason, kept for logging/debugging.
    #[allow(dead_code)]
    Failed(String),
}

pub struct AnalysisBuffer {
    /// Decimated mono samples (see `decimate_mono`), analysis-rate spaced.
    pub samples: Vec<f32>,
    /// Analysis rate: source_rate / decimation factor.
    pub rate: u32,
    pub state: AnalysisState,
}

impl AnalysisBuffer {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            rate: ANALYSIS_RATE,
            state: AnalysisState::Pending,
        }
    }
}

/// Mix interleaved frames down to mono (channel average), then decimate by
/// averaging `k` consecutive mono samples. Leftover mono samples (< k) are
/// kept in `carry` for the next call.
fn decimate_mono(interleaved: &[f32], channels: usize, k: usize, carry: &mut Vec<f32>) -> Vec<f32> {
    let channels = channels.max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i + channels <= interleaved.len() {
        let mut m = 0.0;
        for c in 0..channels {
            m += interleaved[i + c];
        }
        carry.push(m / channels as f32);
        i += channels;
        if carry.len() == k {
            let sum: f32 = carry.iter().sum();
            out.push(sum / k as f32);
            carry.clear();
        }
    }
    out
}

// ─── start_analysis: download + decode in the background ────────────────────

/// Download `url` and decode it into a shared AnalysisBuffer.
/// `cancel` aborts both the download and the decode loop.
pub fn start_analysis(url: String, cancel: Arc<AtomicBool>) -> Arc<Mutex<AnalysisBuffer>> {
    let analysis = Arc::new(Mutex::new(AnalysisBuffer::new()));
    let growing = Arc::new(GrowingBuffer::new());

    // Downloader task: push HTTP chunks into the growing buffer.
    {
        let growing = growing.clone();
        let analysis = analysis.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut resp = match reqwest::get(&url).await {
                Ok(r) => r,
                Err(e) => {
                    fail(&analysis, format!("download error: {e}"));
                    growing.finish();
                    return;
                }
            };
            if let Err(e) = resp.error_for_status_ref() {
                fail(&analysis, format!("http error: {e}"));
                growing.finish();
                return;
            }
            growing.set_total_len(resp.content_length());
            loop {
                if cancel.load(Ordering::Relaxed) {
                    growing.cancel();
                    return;
                }
                match resp.chunk().await {
                    Ok(Some(bytes)) => growing.push(&bytes),
                    Ok(None) => break,
                    Err(e) => {
                        fail(&analysis, format!("download error: {e}"));
                        growing.finish();
                        return;
                    }
                }
            }
            growing.finish();
        });
    }

    // Decoder task: symphonia on a blocking thread, reading the growing buffer.
    {
        let analysis = analysis.clone();
        tokio::task::spawn_blocking(move || {
            decode_stream(growing, analysis, cancel);
        });
    }

    analysis
}

fn fail(analysis: &Mutex<AnalysisBuffer>, msg: String) {
    if let Ok(mut buf) = analysis.lock() {
        buf.state = AnalysisState::Failed(msg);
    }
}

fn decode_stream(
    growing: Arc<GrowingBuffer>,
    analysis: Arc<Mutex<AnalysisBuffer>>,
    cancel: Arc<AtomicBool>,
) {
    let reader = GrowingReader::new(growing);
    let mss = MediaSourceStream::new(Box::new(reader), Default::default());
    let probed = match symphonia::default::get_probe().format(
        &Hint::new(),
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) {
        Ok(p) => p,
        Err(e) => {
            fail(&analysis, format!("probe error: {e}"));
            return;
        }
    };
    let mut format = probed.format;
    let Some(track) = format.default_track() else {
        fail(&analysis, "no audio track".to_string());
        return;
    };
    let track_id = track.id;
    let mut decoder = match symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
    {
        Ok(d) => d,
        Err(e) => {
            fail(&analysis, format!("codec error: {e}"));
            return;
        }
    };

    let mut carry: Vec<f32> = Vec::new();
    // Local accumulation; pushed under lock in ~0.25 s chunks.
    let mut local: Vec<f32> = Vec::new();
    let mut push_threshold = 0usize;
    let mut sample_rate: Option<u32> = None;
    let mut k = 1usize;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => {
                finish_decoding(&analysis, &mut local, sample_rate, Some(e.to_string()));
                return;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => {
                finish_decoding(&analysis, &mut local, sample_rate, Some(e.to_string()));
                return;
            }
        };
        let spec = *decoded.spec();
        let source_rate = spec.rate;
        let channels = spec.channels.count();
        if sample_rate.is_none() {
            k = (source_rate as f64 / ANALYSIS_RATE as f64).ceil() as usize;
            k = k.max(1);
            push_threshold = ((ANALYSIS_RATE as f64) * PUSH_CHUNK_SECS) as usize;
            sample_rate = Some(source_rate);
            if let Ok(mut buf) = analysis.lock() {
                buf.rate = source_rate / k as u32;
            }
        }
        let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sb.copy_interleaved_ref(decoded);
        let decimated = decimate_mono(sb.samples(), channels, k, &mut carry);
        local.extend_from_slice(&decimated);
        if local.len() >= push_threshold {
            push_samples(&analysis, &mut local);
        }
        if reached_max(&analysis) {
            break;
        }
    }
    finish_decoding(&analysis, &mut local, sample_rate, None);
}

fn push_samples(analysis: &Mutex<AnalysisBuffer>, local: &mut Vec<f32>) {
    if let Ok(mut buf) = analysis.lock() {
        buf.samples.append(local);
        // Ready as soon as data is available; decoding may still continue.
        if matches!(buf.state, AnalysisState::Pending) && !buf.samples.is_empty() {
            buf.state = AnalysisState::Ready;
        }
    }
}

fn reached_max(analysis: &Mutex<AnalysisBuffer>) -> bool {
    analysis
        .lock()
        .map(|b| b.samples.len() as u64 >= MAX_SECONDS * b.rate as u64)
        .unwrap_or(true)
}

fn finish_decoding(
    analysis: &Mutex<AnalysisBuffer>,
    local: &mut Vec<f32>,
    _rate: Option<u32>,
    error: Option<String>,
) {
    if let Ok(mut buf) = analysis.lock() {
        buf.samples.append(local);
        buf.state = if buf.samples.is_empty() {
            match error {
                Some(e) => AnalysisState::Failed(e),
                None => AnalysisState::Failed("no decodable audio".to_string()),
            }
        } else {
            AnalysisState::Ready
        };
    }
}

// ─── SpectrumAnalyzer: streaming adapter over CavaCore ─────────────────────

/// cava cut-off frequencies: keeps roughly the previous 30 Hz–16 kHz axis.
const LOW_CUT_OFF: f32 = 40.0;
const HIGH_CUT_OFF: f32 = 16_000.0;
/// cava-recommended noise reduction.
const NOISE_REDUCTION: f32 = 0.77;
/// Peak-indicator decay per update (~50 ms tick).
const PEAK_DECAY: f32 = 0.015;

pub struct SpectrumAnalyzer {
    core: Option<crate::cavacore::CavaCore>,
    /// Parameters the current core was built with.
    core_bars: usize,
    core_rate: u32,
    /// Sample index in `AnalysisBuffer::samples` already fed to the core.
    fed_until: usize,
    debug_counter: u32,
    /// Smoothed bar heights, normalized to 0..1.
    pub bands: Vec<f32>,
    /// Geometric centre frequency (Hz) of each bar.
    pub band_freqs: Vec<f32>,
    /// Slowly decaying peak marker per bar (0..1), for the bars renderer.
    pub peaks: Vec<f32>,
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        Self {
            core: None,
            core_bars: 0,
            core_rate: 0,
            fed_until: 0,
            debug_counter: 0,
            bands: Vec::new(),
            band_freqs: Vec::new(),
            peaks: Vec::new(),
        }
    }

    /// Feed the samples decoded since the last call into the cava core and
    /// copy the resulting bars into `bands`. The core is rebuilt when the
    /// sample rate or the requested bar count changes, and reset+rewound on
    /// seeks. Returns false when nothing new was fed AND there is nothing to
    /// display yet (previous display is kept otherwise).
    pub fn update(&mut self, buf: &AnalysisBuffer, position: Duration, n_bands: usize) -> bool {
        let rate = buf.rate.max(1);
        if self.core.is_none() || self.core_bars != n_bands || self.core_rate != rate {
            let core = crate::cavacore::CavaCore::new(
                n_bands.max(1),
                rate,
                true,
                NOISE_REDUCTION,
                LOW_CUT_OFF,
                HIGH_CUT_OFF,
            );
            self.band_freqs = core.bar_freqs().to_vec();
            self.bands = vec![0.0; n_bands.max(1)];
            self.peaks = vec![0.0; n_bands.max(1)];
            self.core_bars = n_bands;
            self.core_rate = rate;
            self.core = Some(core);
        }
        let Some(core) = self.core.as_mut() else {
            return false;
        };

        let target = ((position.as_secs_f64() * rate as f64) as usize).min(buf.samples.len());
        // Seek (jump > 1 s either way): resync the core. Smaller backward
        // steps are mpv time-pos jitter and must not reset the sensitivity.
        let one_sec = rate as usize;
        if target + one_sec < self.fed_until || target > self.fed_until + one_sec {
            core.reset();
            self.fed_until = target.saturating_sub(core.input_buffer_size());
        }

        let fed = target > self.fed_until;
        let new_count = target.saturating_sub(self.fed_until);
        if fed {
            let bars = core.execute(&buf.samples[self.fed_until..target]);
            self.bands = bars.to_vec();
            self.fed_until = target;
        }
        // Diagnostic trace (TUIDAL_BG_DEBUG=1): one line every ~20 updates.
        self.debug_counter = self.debug_counter.wrapping_add(1);
        if self.debug_counter.is_multiple_of(20) {
            let max = self.bands.iter().cloned().fold(0.0f32, f32::max);
            let full = self.bands.iter().filter(|v| **v >= 0.99).count();
            let peak_in = buf.samples[self.fed_until.saturating_sub(new_count)..self.fed_until]
                .iter()
                .fold(0.0f32, |m, s| m.max(s.abs()));
            crate::kitty_bg::bg_log(&format!(
                "spectrum: rate={rate} n={} pos={:.2}s target={target} fed={fed} new={new_count} \
                 in_peak={peak_in:.3} max={max:.3} full={full} sens={:.3e} fps={:.1}",
                self.bands.len(),
                position.as_secs_f64(),
                core.sens(),
                core.framerate(),
            ));
        }

        // Peak markers decay every frame.
        if self.peaks.len() != self.bands.len() {
            self.peaks = vec![0.0; self.bands.len()];
        }
        for (p, b) in self.peaks.iter_mut().zip(self.bands.iter()) {
            *p = (*p - PEAK_DECAY).max(*b).clamp(0.0, 1.0);
        }

        fed || !self.bands.is_empty()
    }

    /// Clear bands (new track / playback stopped).
    pub fn reset(&mut self) {
        self.core = None;
        self.core_bars = 0;
        self.core_rate = 0;
        self.fed_until = 0;
        self.bands.clear();
        self.band_freqs.clear();
        self.peaks.clear();
    }
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn growing_reader_blocks_until_data() {
        let buf = Arc::new(GrowingBuffer::new());
        let writer = buf.clone();
        let data = b"hello world, streaming bytes!".to_vec();
        let split = data.clone();
        std::thread::spawn(move || {
            for chunk in split.chunks(10) {
                writer.push(chunk);
                std::thread::sleep(Duration::from_millis(20));
            }
            writer.finish();
        });
        let mut reader = GrowingReader::new(buf);
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        assert_eq!(got, data);

        // Seek back and read the last two bytes.
        let len = got.len() as u64;
        reader.seek(SeekFrom::Start(len - 2)).unwrap();
        let mut tail = [0u8; 2];
        reader.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, &data[data.len() - 2..]);
        // Read past EOF returns 0.
        let mut b = [0u8; 8];
        assert_eq!(reader.read(&mut b).unwrap(), 0);
    }

    fn sine_buffer(freq: f32, secs: f32, rate: u32) -> AnalysisBuffer {
        let n = (secs * rate as f32) as usize;
        let samples = (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect();
        AnalysisBuffer {
            samples,
            rate,
            state: AnalysisState::Ready,
        }
    }

    fn argmax_band_freq(an: &SpectrumAnalyzer) -> f32 {
        let idx = an
            .bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        an.band_freqs[idx]
    }

    #[test]
    fn sine_peak_lands_in_right_band() {
        let buf = sine_buffer(1000.0, 3.0, 24_000);
        let mut an = SpectrumAnalyzer::new();
        assert!(an.update(&buf, Duration::from_secs_f64(1.5), 64));
        let f = argmax_band_freq(&an);
        assert!((850.0..=1150.0).contains(&f), "peak at {f} Hz");

        let buf = sine_buffer(100.0, 3.0, 24_000);
        an.reset();
        assert!(an.update(&buf, Duration::from_secs_f64(1.5), 64));
        let f = argmax_band_freq(&an);
        assert!((80.0..=125.0).contains(&f), "peak at {f} Hz");
    }

    #[test]
    fn decimation_averages_and_rate() {
        let mut carry = Vec::new();
        let out = decimate_mono(&[1.0, 1.0, 3.0, 3.0, 5.0, 5.0, 7.0, 7.0], 2, 2, &mut carry);
        assert_eq!(out, vec![2.0, 6.0]);
        assert!(carry.is_empty());

        // Partial groups carry over between packets: 3 mono frames then 1.
        let mut carry = Vec::new();
        let o1 = decimate_mono(&[2.0, 4.0, 6.0], 1, 2, &mut carry);
        assert_eq!(o1, vec![3.0]);
        let o2 = decimate_mono(&[8.0], 1, 2, &mut carry);
        assert_eq!(o2, vec![7.0]);
        assert!(carry.is_empty());
    }

    #[tokio::test]
    async fn analysis_over_http_finds_440hz() {
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_err()
        {
            return; // ffmpeg unavailable: skip
        }
        let flac = "/tmp/tuidal-test-440.flac";
        let m4a = "/tmp/tuidal-test-440.m4a";
        for (out, codec_args) in [
            (flac, vec!["-c:a", "flac"]),
            (m4a, vec!["-c:a", "aac", "-b:a", "128k"]),
        ] {
            let status = std::process::Command::new("ffmpeg")
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=44100:duration=4",
                    "-ac",
                    "2",
                ])
                .args(codec_args)
                .arg(out)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("ffmpeg failed to start");
            assert!(status.success(), "ffmpeg failed for {out}");
        }

        // Serve /tmp over HTTP on an ephemeral port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let app = axum::Router::new().route(
            "/{name}",
            axum::routing::get(|axum::extract::Path(name): axum::extract::Path<String>| async move {
                let path = format!("/tmp/{name}");
                match tokio::fs::read(&path).await {
                    Ok(bytes) => axum::response::Response::builder()
                        .header("content-type", "application/octet-stream")
                        .header("content-length", bytes.len())
                        .body(axum::body::Body::from(bytes))
                        .unwrap(),
                    Err(_) => axum::response::Response::builder()
                        .status(404)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                }
            }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        for file in ["tuidal-test-440.flac", "tuidal-test-440.m4a"] {
            let url = format!("http://127.0.0.1:{port}/{file}");
            let cancel = Arc::new(AtomicBool::new(false));
            let analysis = start_analysis(url, cancel);
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            loop {
                let (len, rate, done) = analysis
                    .lock()
                    .map(|b| (b.samples.len(), b.rate, matches!(b.state, AnalysisState::Ready | AnalysisState::Failed(_))))
                    .unwrap_or((0, 0, false));
                if len >= rate as usize * 3 || done {
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "timeout waiting for {file}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let mut an = SpectrumAnalyzer::new();
            let ok = {
                let buf = analysis.lock().unwrap();
                an.update(&buf, Duration::from_secs(2), 64)
            };
            assert!(ok, "no decoded data for {file}");
            let f = argmax_band_freq(&an);
            assert!((380.0..=510.0).contains(&f), "{file}: peak at {f} Hz");
        }
        // Tidy up artifacts.
        let _ = std::fs::remove_file(flac);
        let _ = std::fs::remove_file(m4a);
        let _ = std::io::stdout().flush();
    }
}

//! Playlist/queue export to M3U8, CSV or JSON. Metadata and Tidal web links
//! only — never stream URLs or audio content.

use std::fs;
use std::path::{Path, PathBuf};

use crate::tidal::Track;

#[derive(Clone, Copy, PartialEq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    M3u8,
    Csv,
    #[default]
    Json,
}

/// Where an export goes: a file on disk, a new Tidal playlist, or a URL.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ExportDestination {
    File(ExportFormat),
    Tidal,
    Upload,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::M3u8 => "m3u8",
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::M3u8 => "M3U8",
            ExportFormat::Csv => "CSV",
            ExportFormat::Json => "JSON",
        }
    }
}

/// Default export directory: $TUIDAL_EXPORT_DIR, else
/// `<audio dir>/tuidal`, else `<config dir>/tuidal/exports`.
pub fn default_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TUIDAL_EXPORT_DIR")
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    if let Some(audio) = dirs::audio_dir() {
        return audio.join("tuidal");
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tuidal")
        .join("exports")
}

/// Lowercase slug for the file name (max 60 chars, "playlist" when empty).
fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 60 {
            break;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "playlist".to_string() } else { out }
}

/// Local-time timestamp "YYYYMMDD-HHMMSS" via libc localtime_r.
pub fn timestamp_local() -> (String, libc::tm) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&secs, &mut tm);
        (
            format!(
                "{:04}{:02}{:02}-{:02}{:02}{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday,
                tm.tm_hour,
                tm.tm_min,
                tm.tm_sec
            ),
            tm,
        )
    }
}

/// RFC 3339 UTC timestamp for the JSON `exported_at` field.
fn timestamp_utc_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::gmtime_r(&secs, &mut tm);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}

/// Sanitised file name "<slug>-<YYYYMMDD-HHMMSS>.<ext>".
pub fn file_name(format: ExportFormat, title: &str) -> String {
    let (ts, _) = timestamp_local();
    format!("{}-{}.{}", slug(title), ts, format.extension())
}

/// RFC 4180 CSV field quoting.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn track_url(id: u64) -> String {
    format!("https://tidal.com/browse/track/{id}")
}

/// Render the tracks in the given format.
pub fn render(format: ExportFormat, title: &str, tracks: &[Track]) -> String {
    match format {
        ExportFormat::M3u8 => {
            let mut out = format!("#EXTM3U\n#PLAYLIST:{title}\n");
            for t in tracks {
                out.push_str(&format!(
                    "#EXTINF:{},{} - {}\n{}\n",
                    t.duration,
                    t.artist_names(),
                    t.title,
                    track_url(t.id)
                ));
            }
            out
        }
        ExportFormat::Csv => {
            let mut out =
                "position,title,artists,album,duration_seconds,isrc,tidal_id,url\n".to_string();
            for (i, t) in tracks.iter().enumerate() {
                let artists = t
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    i + 1,
                    csv_field(&t.title),
                    csv_field(&artists),
                    csv_field(&t.album.title),
                    t.duration,
                    csv_field(t.isrc.as_deref().unwrap_or("")),
                    t.id,
                    csv_field(&track_url(t.id)),
                ));
            }
            out
        }
        ExportFormat::Json => {
            let doc = serde_json::json!({
                "title": title,
                "exported_at": timestamp_utc_rfc3339(),
                "source": "tuidal",
                "tracks": tracks.iter().enumerate().map(|(i, t)| serde_json::json!({
                    "position": i + 1,
                    "id": t.id,
                    "title": t.title,
                    "artists": t.artists.iter().map(|a| serde_json::json!({
                        "id": a.id,
                        "name": a.name,
                    })).collect::<Vec<_>>(),
                    "album": {"id": t.album.id, "title": t.album.title},
                    "duration": t.duration,
                    "isrc": t.isrc,
                    "url": track_url(t.id),
                })).collect::<Vec<_>>(),
            });
            serde_json::to_string_pretty(&doc).unwrap_or_default()
        }
    }
}

/// Write the rendered export and return the file path.
pub fn write(
    format: ExportFormat,
    dir: &Path,
    title: &str,
    tracks: &[Track],
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(file_name(format, title));
    fs::write(&path, render(format, title, tracks))?;
    Ok(path)
}

/// POST the rendered export to `url`. Returns the HTTP status on 2xx.
pub async fn upload(
    url: &str,
    token: Option<&str>,
    format: ExportFormat,
    title: &str,
    tracks: &[Track],
) -> anyhow::Result<u16> {
    let content_type = match format {
        ExportFormat::M3u8 => "audio/x-mpegurl; charset=utf-8",
        ExportFormat::Csv => "text/csv; charset=utf-8",
        ExportFormat::Json => "application/json",
    };
    let mut req = reqwest::Client::new()
        .post(url)
        .timeout(std::time::Duration::from_secs(15))
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .header("X-Tuidal-Title", title)
        .header("X-Tuidal-Format", format.extension())
        .body(render(format, title, tracks));
    if let Some(t) = token
        && !t.is_empty()
    {
        req = req.bearer_auth(t);
    }
    let status = req.send().await?.status().as_u16();
    if !(200..300).contains(&status) {
        anyhow::bail!("HTTP {status}");
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::{Album, Artist};

    fn track(id: u64, title: &str, secs: u64) -> Track {
        Track {
            id,
            title: title.to_string(),
            duration: secs,
            track_number: None,
            artists: vec![Artist {
                id: 7,
                name: "Art, Ist".into(),
            }],
            album: Album {
                id: 9,
                title: "Alb".into(),
            },
            audio_quality: None,
            explicit: None,
            isrc: Some("GB1234567890".into()),
        }
    }

    #[test]
    fn csv_quotes_commas_and_quotes() {
        let mut t = track(42, "Say \"Hi\", Twice", 61);
        t.artists[0].name = "The, Band".into();
        let out = render(ExportFormat::Csv, "queue", &[t]);
        let line = out.lines().nth(1).unwrap();
        assert!(line.contains("\"Say \"\"Hi\"\", Twice\""), "line: {line}");
        assert!(line.contains("\"The, Band\""), "line: {line}");
        assert!(line.ends_with("https://tidal.com/browse/track/42"));
    }

    #[test]
    fn m3u8_header_and_extinf() {
        let out = render(ExportFormat::M3u8, "My Mix", &[track(42, "T", 95)]);
        let mut lines = out.lines();
        assert_eq!(lines.next(), Some("#EXTM3U"));
        assert_eq!(lines.next(), Some("#PLAYLIST:My Mix"));
        assert_eq!(lines.next(), Some("#EXTINF:95,Art, Ist - T"));
        assert_eq!(lines.next(), Some("https://tidal.com/browse/track/42"));
    }

    #[test]
    fn json_roundtrip_track_count_and_url() {
        let out = render(
            ExportFormat::Json,
            "q",
            &[track(1, "a", 1), track(2, "b", 2)],
        );
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["source"], "tuidal");
        assert_eq!(v["tracks"].as_array().unwrap().len(), 2);
        assert_eq!(
            v["tracks"][0]["url"],
            "https://tidal.com/browse/track/1"
        );
    }

    #[test]
    fn file_name_slug_and_extension() {
        let n = file_name(ExportFormat::Json, "Trip Hop !! Mix");
        assert!(n.starts_with("trip-hop-mix-"), "name: {n}");
        assert!(n.ends_with(".json"), "name: {n}");
        let empty = file_name(ExportFormat::Csv, "!!!");
        assert!(empty.starts_with("playlist-") && empty.ends_with(".csv"));
    }

    #[test]
    fn upload_posts_rendered_body() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        // Tiny one-shot HTTP server: capture the request, answer 201.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let captured2 = captured.clone();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            // Read until we have headers + full body.
            loop {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req).to_string();
                if let Some(hpos) = text.find("\r\n\r\n") {
                    let clen: usize = text[..hpos]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse().ok())
                        })
                        .unwrap_or(0);
                    if text.len() >= hpos + 4 + clen {
                        break;
                    }
                }
            }
            *captured2.lock().unwrap() = String::from_utf8_lossy(&req).to_string();
            stream
                .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let status = rt
            .block_on(upload(
                &format!("http://127.0.0.1:{port}/recv"),
                Some("sekrit"),
                ExportFormat::Json,
                "Demo",
                &[track(5, "Upload Me", 10)],
            ))
            .unwrap();
        assert_eq!(status, 201);
        let req = captured.lock().unwrap().clone();
        // reqwest emits lowercase header names.
        assert!(req.contains("authorization: Bearer sekrit"), "{req}");
        assert!(req.contains("x-tuidal-format: json"), "{req}");
        assert!(req.contains("x-tuidal-title: Demo"), "{req}");
        assert!(req.contains("Upload Me"), "{req}");
    }

    #[test]
    fn write_and_read_back() {
        let dir = std::env::temp_dir().join("tuidal-export-test-8930917");
        let path = write(ExportFormat::Json, &dir, "demo", &[track(5, "x", 10)]).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"demo\""));
        let _ = fs::remove_dir_all(&dir);
    }
}

//! macOS system media controls (media keys / Control Center / Now Playing)
//! via `souvlaki`.
//!
//! MPRemoteCommandCenter handlers only fire while an AppKit run loop runs on
//! the MAIN thread, so this module must be driven from `fn main` on macOS
//! while the tokio runtime lives on a spawned thread.

#![cfg(target_os = "macos")]
// `msg_send!`/`class!` from objc 0.2 internally expand to `cfg(feature =
// "cargo-clippy")` checks which trip `unexpected_cfgs`; not fixable on our side.
#![allow(unexpected_cfgs)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy};
use cocoa::base::{YES, id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSDefaultRunLoopMode};
use objc::{class, msg_send, sel, sel_impl};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::app::{ApiCommand, ApiStatus, ApiTrack, AppEvent};

const PUMP_INTERVAL: Duration = Duration::from_millis(250);

/// Fields that, when changed, require a Now Playing metadata refresh.
#[derive(PartialEq)]
struct Snapshot {
    state: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<u64>,
    cover_url: Option<String>,
}

fn snapshot(s: &ApiStatus) -> Snapshot {
    Snapshot {
        state: s.state.clone(),
        title: s.title.clone(),
        artist: s.artist.clone(),
        album: s.album.clone(),
        duration: s.duration,
        cover_url: s.cover_url.clone(),
    }
}

/// Mirror of `MprisPlayer::play`: resume when paused, restart the current
/// queue entry when stopped, do nothing when already playing.
fn play_command(status: &Arc<RwLock<ApiStatus>>) -> Option<ApiCommand> {
    let s = status.read().unwrap();
    match s.state.as_str() {
        "paused" => Some(ApiCommand::PlayPause),
        "stopped" => s.queue_index.and_then(|i| {
            s.queue.get(i).map(|track| {
                ApiCommand::PlayTrack(ApiTrack {
                    id: track.id,
                    title: track.title.clone(),
                    artist: track.artist_names(),
                    album: track.album.title.clone(),
                    duration: track.duration,
                })
            })
        }),
        _ => None,
    }
}

fn map_event(ev: MediaControlEvent, status: &Arc<RwLock<ApiStatus>>) -> Option<ApiCommand> {
    match ev {
        MediaControlEvent::Play => play_command(status),
        MediaControlEvent::Pause => {
            if status.read().unwrap().state == "playing" {
                Some(ApiCommand::PlayPause)
            } else {
                None
            }
        }
        MediaControlEvent::Toggle => Some(ApiCommand::PlayPause),
        MediaControlEvent::Next => Some(ApiCommand::Next),
        MediaControlEvent::Previous => Some(ApiCommand::Prev),
        MediaControlEvent::Stop => Some(ApiCommand::Stop),
        MediaControlEvent::Seek(dir) => {
            let secs = match dir {
                SeekDirection::Forward => 10,
                SeekDirection::Backward => -10,
            };
            Some(ApiCommand::Seek(secs))
        }
        MediaControlEvent::SeekBy(dir, dur) => {
            let secs = dur.as_secs() as i64;
            Some(ApiCommand::Seek(match dir {
                SeekDirection::Forward => secs,
                SeekDirection::Backward => -secs,
            }))
        }
        MediaControlEvent::SetPosition(pos) => Some(ApiCommand::SetPosition(pos.0.as_secs())),
        MediaControlEvent::SetVolume(v) => {
            Some(ApiCommand::VolumeSet((v.clamp(0.0, 1.0) * 100.0) as u8))
        }
        // Quit/Raise/OpenUri: not applicable to a TUI app.
        _ => None,
    }
}

/// Drain pending AppKit events, then run the main run loop for up to
/// `timeout`. The `runMode:beforeDate:` call is what dispatches the
/// MPRemoteCommandCenter handlers (they are invoked on the main run loop /
/// main dispatch queue); the `nextEventMatchingMask:` drain additionally
/// flushes ordinary NSEvents so a Dock icon stays responsive.
unsafe fn pump_run_loop(timeout: Duration) {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        let distant_past: id = msg_send![class!(NSDate), distantPast];
        loop {
            let event: id = msg_send![app, nextEventMatchingMask: u64::MAX
                untilDate: distant_past
                inMode: NSDefaultRunLoopMode
                dequeue: YES];
            if event == nil {
                break;
            }
            let () = msg_send![app, sendEvent: event];
        }
        let run_loop: id = msg_send![class!(NSRunLoop), currentRunLoop];
        let until: id =
            msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: timeout.as_secs_f64()];
        let () = msg_send![run_loop, runMode: NSDefaultRunLoopMode beforeDate: until];
        let () = msg_send![pool, drain];
    }
}

/// Bring the process up as a regular (Dock-icon) application so it can own
/// a Now Playing session.
unsafe fn init_appkit() {
    unsafe {
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular);
        app.finishLaunching();
    }
}

/// Runs the AppKit event loop on the calling (main) thread and mirrors
/// `status` into Now Playing until `done` becomes true. Media commands are
/// forwarded to `tx` as `AppEvent::ApiCmd`.
pub fn run_main_thread_loop(
    status: Arc<RwLock<ApiStatus>>,
    tx: UnboundedSender<AppEvent>,
    done: Arc<AtomicBool>,
) {
    unsafe {
        init_appkit();
    }

    let mut controls = match MediaControls::new(PlatformConfig {
        dbus_name: "tuidal",
        display_name: "tuidal",
        hwnd: None,
    }) {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("media controls unavailable: {e}");
            None
        }
    };

    if let Some(controls) = controls.as_mut() {
        let handler_status = status.clone();
        let handler_tx = tx.clone();
        let _ = controls.attach(move |ev| {
            if let Some(cmd) = map_event(ev, &handler_status) {
                let _ = handler_tx.send(AppEvent::ApiCmd(cmd));
            }
        });
    }

    let Some(controls) = controls.as_mut() else {
        // Without media controls there is nothing to do on the main thread;
        // just wait for the app thread to finish.
        while !done.load(Ordering::SeqCst) {
            std::thread::sleep(PUMP_INTERVAL);
        }
        return;
    };

    let mut prev: Option<Snapshot> = None;
    let mut last_elapsed = 0u64;
    let mut last_push = Instant::now();

    while !done.load(Ordering::SeqCst) {
        unsafe {
            pump_run_loop(PUMP_INTERVAL);
        }

        let (snap, elapsed) = {
            let s = status.read().unwrap();
            (snapshot(&s), s.elapsed)
        };

        let metadata_changed = prev.as_ref() != Some(&snap);
        if metadata_changed {
            let _ = controls.set_metadata(MediaMetadata {
                title: snap.title.as_deref(),
                artist: snap.artist.as_deref(),
                album: snap.album.as_deref(),
                duration: snap.duration.map(Duration::from_secs),
                cover_url: snap.cover_url.as_deref(),
            });
        }

        // Push playback when state or track changed, or when the reported
        // position jumped by more than 2 s (seek). While playing, macOS
        // extrapolates elapsed time on its own.
        let expected = if snap.state == "playing" {
            last_elapsed.saturating_add(last_push.elapsed().as_secs())
        } else {
            last_elapsed
        };
        let seeked = elapsed.abs_diff(expected) > 2;
        if metadata_changed || seeked {
            let progress = Some(MediaPosition(Duration::from_secs(elapsed)));
            let _ = controls.set_playback(match snap.state.as_str() {
                "playing" => MediaPlayback::Playing { progress },
                "paused" => MediaPlayback::Paused { progress },
                _ => MediaPlayback::Stopped,
            });
            last_elapsed = elapsed;
            last_push = Instant::now();
        }

        prev = Some(snap);
    }
}

/// Hidden `--media-probe` entry point: registers a fake Now Playing session
/// and prints every received media command, for manual media-keys testing.
pub fn run_probe() -> Result<()> {
    let status = Arc::new(RwLock::new(ApiStatus {
        state: "playing".to_string(),
        title: Some("Probe".to_string()),
        artist: Some("tuidal".to_string()),
        album: Some("Probe Album".to_string()),
        duration: Some(200),
        cover_url: Some("https://picsum.photos/512".to_string()),
        ..Default::default()
    }));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    std::thread::spawn(move || {
        while let Some(ev) = rx.blocking_recv() {
            if let AppEvent::ApiCmd(cmd) = ev {
                let name = match cmd {
                    ApiCommand::PlayPause => "PlayPause",
                    ApiCommand::Next => "Next",
                    ApiCommand::Prev => "Prev",
                    ApiCommand::VolumeUp => "VolumeUp",
                    ApiCommand::VolumeDown => "VolumeDown",
                    ApiCommand::VolumeSet(_) => "VolumeSet",
                    ApiCommand::SeekForward => "SeekForward",
                    ApiCommand::SeekBackward => "SeekBackward",
                    ApiCommand::PlayTrack(_) => "PlayTrack",
                    ApiCommand::ToggleShuffle => "ToggleShuffle",
                    ApiCommand::CycleRepeat => "CycleRepeat",
                    ApiCommand::Stop => "Stop",
                    ApiCommand::Seek(_) => "Seek",
                    ApiCommand::SetPosition(_) => "SetPosition",
                    ApiCommand::TrackRadio => "TrackRadio",
                    ApiCommand::ArtistRadio => "ArtistRadio",
                };
                eprintln!("probe: media command -> {name}");
            }
        }
    });

    // Tick elapsed once per second like the real app does.
    {
        let status = status.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            status.write().unwrap().elapsed += 1;
        });
    }

    // Auto-exit after 60 s.
    let done = Arc::new(AtomicBool::new(false));
    {
        let done = done.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(60));
            done.store(true, Ordering::SeqCst);
        });
    }

    eprintln!("probe: Now Playing session active for 60 s — press media keys");
    run_main_thread_loop(status, tx, done);
    Ok(())
}

//! OS media transport integration (Windows SMTC / macOS Now Playing).
//!
//! Uses souvlaki to report playback state and metadata to the OS,
//! and receive media key events (play/pause/stop).

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

/// Wraps souvlaki MediaControls. Created once at app startup.
pub struct MediaSession {
    controls: Mutex<MediaControls>,
    last_metadata: Mutex<Option<(String, String, String, Option<String>)>>,
    last_playback: Mutex<Option<MediaPlayback>>,
}

// Safety: MediaControls is only accessed through Mutex.
// On Windows, SMTC calls happen on the thread that created it (UI thread).
unsafe impl Send for MediaSession {}
unsafe impl Sync for MediaSession {}

impl MediaSession {
    /// Create and attach media controls.
    /// `hwnd` is required on Windows (raw window handle), ignored on other platforms.
    #[allow(unused_variables)]
    pub fn new(hwnd: *mut std::ffi::c_void, app_handle: AppHandle) -> Option<Self> {
        let config = PlatformConfig {
            dbus_name: "radiko_desktop",
            display_name: "Radiko Desktop",
            hwnd: Some(hwnd),
        };

        let mut controls = match MediaControls::new(config) {
            Ok(c) => c,
            Err(e) => {
                warn!("Media controls unavailable: {:?}", e);
                return None;
            }
        };

        // Handle OS media key events → invoke Tauri commands
        let handle = app_handle.clone();
        if let Err(e) = controls.attach(move |event| {
            let h = handle.clone();
            match event {
                MediaControlEvent::Play => {
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "play");
                    });
                }
                MediaControlEvent::Pause => {
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "pause");
                    });
                }
                MediaControlEvent::Toggle => {
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "toggle");
                    });
                }
                MediaControlEvent::Stop => {
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "stop");
                    });
                }
                MediaControlEvent::Next => {
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "next");
                    });
                }
                MediaControlEvent::Previous => {
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "previous");
                    });
                }
                _ => {}
            }
        }) {
            warn!("Failed to attach media controls: {:?}", e);
            return None;
        }

        info!("OS media controls initialized");
        Some(Self {
            controls: Mutex::new(controls),
            last_metadata: Mutex::new(None),
            last_playback: Mutex::new(None),
        })
    }

    pub fn set_metadata(&self, title: &str, artist: &str, cover_url: Option<&str>) {
        self.set_metadata_full(title, artist, artist, cover_url);
    }

    pub fn set_metadata_full(&self, title: &str, artist: &str, album: &str, cover_url: Option<&str>) {
        // QUICK CHECK: Avoid redundant OS updates if metadata hasn't changed
        {
            if let Ok(last) = self.last_metadata.lock() {
                if let Some((l_title, l_artist, l_album, l_cover)) = last.as_ref() {
                    if l_title == title && l_artist == artist && l_album == album && l_cover.as_deref() == cover_url {
                        return; // No change, skip OS call
                    }
                }
            }
        }

        if let Ok(ref mut c) = self.controls.lock() {
            let smtc_cover = cover_url.map(|u| {
                if let Some(path_part) = u.strip_prefix("file:///") {
                    #[cfg(target_os = "windows")]
                    {
                        format!("file://{}", path_part.replace('/', "\\"))
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        format!("file:///{}", path_part)
                    }
                } else {
                    u.to_string()
                }
            });
            let smtc_cover_ref = smtc_cover.as_deref();

            let metadata = MediaMetadata {
                title: Some(title),
                artist: Some(artist),
                album: Some(album),
                cover_url: smtc_cover_ref,
                ..Default::default()
            };

            if let Err(e) = c.set_metadata(metadata) {
                warn!("SMTC set_metadata failed: {:?}", e);
                // Retry once without cover if it was a file path issue
                if smtc_cover_ref.is_some() {
                    let _ = c.set_metadata(MediaMetadata {
                        title: Some(title),
                        artist: Some(artist),
                        album: Some(album),
                        cover_url: None,
                        ..Default::default()
                    });
                }
            } else {
                // Update cache upon success
                if let Ok(mut last) = self.last_metadata.lock() {
                    *last = Some((title.to_string(), artist.to_string(), album.to_string(), cover_url.map(|s| s.to_string())));
                }
            }
        }
    }

    pub fn set_playing(&self) {
        self.set_playback(MediaPlayback::Playing { progress: None });
    }

    pub fn set_paused(&self) {
        self.set_playback(MediaPlayback::Paused { progress: None });
    }

    pub fn set_stopped(&self) {
        self.set_playback(MediaPlayback::Stopped);
    }

    fn set_playback(&self, playback: MediaPlayback) {
        // Caching: Skip if status is the same
        {
            if let Ok(last) = self.last_playback.lock() {
                if let Some(l) = last.as_ref() {
                    // Simple comparison (discriminant-based)
                    if std::mem::discriminant(l) == std::mem::discriminant(&playback) {
                        return;
                    }
                }
            }
        }

        if let Ok(ref mut c) = self.controls.lock() {
            if let Err(e) = c.set_playback(playback.clone()) {
                warn!("SMTC set_playback failed: {:?}", e);
            } else {
                if let Ok(mut last) = self.last_playback.lock() {
                    *last = Some(playback);
                }
            }
        }
    }
}

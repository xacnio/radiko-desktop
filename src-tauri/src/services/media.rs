//! OS media transport integration (Windows SMTC / macOS Now Playing).
//!
//! Uses souvlaki to report playback state and metadata to the OS,
//! and receive media key events (play/pause/stop).

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

/// Wraps souvlaki MediaControls. Created once at app startup.
pub struct MediaSession {
    controls: Mutex<MediaControls>,
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
        })
    }

    pub fn set_metadata(&self, title: &str, artist: &str, cover_url: Option<&str>) {
        info!("SMTC set_metadata: title='{}', artist='{}', cover={:?}", title, artist, cover_url);
        if let Ok(ref mut c) = self.controls.lock() {
            // souvlaki on Windows expects file://C:\path (strips "file://" prefix for GetFileFromPathAsync)
            // Our URLs use file:///C:/path, so convert for SMTC
            let smtc_cover = cover_url.map(|u| {
                if let Some(_path_part) = u.strip_prefix("file:///") {
                    #[cfg(target_os = "windows")]
                    {
                        format!("file://{}", _path_part.replace('/', "\\"))
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        u.to_string()
                    }
                } else {
                    u.to_string()
                }
            });
            let smtc_cover_ref = smtc_cover.as_deref();

            // First try with cover, if that fails try without
            let metadata = MediaMetadata {
                title: Some(title),
                artist: Some(artist),
                cover_url: smtc_cover_ref,
                ..Default::default()
            };
            match c.set_metadata(metadata) {
                Ok(()) => {
                    info!("SMTC metadata set successfully");
                },
                Err(e) => {
                    warn!("SMTC set_metadata failed with cover: {:?}, retrying without cover", e);
                    let metadata_no_cover = MediaMetadata {
                        title: Some(title),
                        artist: Some(artist),
                        cover_url: None,
                        ..Default::default()
                    };
                    if let Err(e2) = c.set_metadata(metadata_no_cover) {
                        error!("SMTC set_metadata failed even without cover: {:?}", e2);
                    } else {
                        info!("SMTC metadata set successfully (without cover)");
                    }
                }
            }
        } else {
            error!("SMTC: failed to lock controls mutex");
        }
    }

    pub fn set_playing(&self) {
        if let Ok(ref mut c) = self.controls.lock() {
            let _ = c.set_playback(MediaPlayback::Playing { progress: None });
        }
    }

    pub fn set_paused(&self) {
        if let Ok(ref mut c) = self.controls.lock() {
            let _ = c.set_playback(MediaPlayback::Paused { progress: None });
        }
    }

    pub fn set_stopped(&self) {
        if let Ok(ref mut c) = self.controls.lock() {
            let _ = c.set_playback(MediaPlayback::Stopped);
        }
    }
}

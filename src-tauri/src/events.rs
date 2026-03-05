use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info};

use crate::services::media::MediaSession;
use crate::player::types::{PlaybackStatus, StreamMetadata};

pub fn emit_status(app: &AppHandle, status: PlaybackStatus) {
    info!("Playback status changed: {:?}", status);

    // Persist status in AppState so get_status() returns the correct value
    if let Some(state) = app.try_state::<crate::state::AppState>() {
        if let Ok(mut ps) = state.inner.lock() {
            ps.status = status;
        }
    }

    if let Err(e) = app.emit("playback-status", status) {
        error!("Failed to emit playback-status event: {}", e);
    }

    // Update OS media transport
    if let Some(ms) = app.try_state::<MediaSession>() {
        match status {
            PlaybackStatus::Playing => {
                ms.set_playing();
                #[cfg(target_os = "windows")]
                crate::platform::thumbbar::set_playing(true);
                // Set metadata immediately so OS player sees the station name
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    if let Ok(ps) = state.inner.lock() {
                        let artist = ps.station_name.as_deref().unwrap_or("Radiko");
                        let title_opt = ps.stream_metadata.as_ref().and_then(|m| m.title.clone());
                        let title = title_opt.as_deref().unwrap_or(artist);
                        let cover = ps.station_image.as_deref()
                            .filter(|u| u.starts_with("file:///"))
                            .or(ps.default_cover.as_deref());
                        info!("emit_status(Playing): station='{}', title='{}', cover={:?}", artist, title, cover);
                        ms.set_metadata(title, artist, cover);
                    } else {
                        error!("emit_status: failed to lock AppState");
                    }
                } else {
                    error!("emit_status: AppState not available");
                }
            }
            PlaybackStatus::Paused => {
                ms.set_paused();
                #[cfg(target_os = "windows")]
                crate::platform::thumbbar::set_playing(false);
            }
            PlaybackStatus::Stopped => {
                ms.set_stopped();
                #[cfg(target_os = "windows")]
                crate::platform::thumbbar::set_playing(false);
            }
            PlaybackStatus::Connecting => {
                // Immediately show new station as paused so old one doesn't linger
                ms.set_paused();
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    if let Ok(ps) = state.inner.lock() {
                        let artist = ps.station_name.as_deref().unwrap_or("Radiko");
                        let cover = ps.default_cover.as_deref();
                        ms.set_metadata(artist, artist, cover);
                    }
                }
            }
            _ => {}
        }
    } else {
        error!("emit_status: MediaSession not available");
    }
}

pub fn emit_metadata(app: &AppHandle, metadata: StreamMetadata) {
    if let Some(ref title) = metadata.title {
        info!("Stream metadata: {}", title);
    }
    if let Err(e) = app.emit("stream-metadata", &metadata) {
        error!("Failed to emit stream-metadata event: {}", e);
    }

    // Trigger background metadata enrichment (fetch cover art/links)
    if let Some(ref title) = metadata.title {
        let app_handle = app.clone();
        let title_clone = title.clone();
        
        let mut station_name = "Unknown Radio".to_string();
        if let Some(state) = app.try_state::<crate::state::AppState>() {
            if let Ok(ps) = state.inner.lock() {
                if let Some(ref s) = ps.station_name {
                    station_name = s.clone();
                }
            }
        }
        
        tokio::spawn(async move {
            crate::services::enricher::enrich_metadata_background(app_handle, title_clone, station_name).await;
        });
    }

    // Update OS media transport
    if let Some(ms) = app.try_state::<MediaSession>() {
        let mut artist = "Radiko".to_string();
        let mut cover_url = None;
        if let Some(state) = app.try_state::<crate::state::AppState>() {
            if let Ok(mut ps) = state.inner.lock() {
                ps.stream_metadata = Some(metadata.clone());
                if let Some(ref s) = ps.station_name {
                    artist = s.clone();
                }
                if let Some(ref c) = ps.station_image {
                    if c.starts_with("file:///") {
                        cover_url = Some(c.clone());
                    }
                }
                // Fallback to default cover
                if cover_url.is_none() {
                    if let Some(ref dc) = ps.default_cover {
                        cover_url = Some(dc.clone());
                    }
                }
            }
        }
        
        let title = metadata.title.as_deref().unwrap_or(&artist);
        ms.set_metadata(title, &artist, cover_url.as_deref());

        // Refresh playback status so OS UI buttons stay enabled/synced
        if let Some(state) = app.try_state::<crate::state::AppState>() {
            if let Ok(ps) = state.inner.lock() {
                match ps.status {
                    PlaybackStatus::Playing => ms.set_playing(),
                    PlaybackStatus::Paused => ms.set_paused(),
                    PlaybackStatus::Stopped => ms.set_stopped(),
                    _ => {}
                }
            }
        }
    }
}

pub fn emit_error(app: &AppHandle, message: &str) {
    error!("Stream error: {}", message);
    if let Err(e) = app.emit("stream-error", message) {
        error!("Failed to emit stream-error event: {}", e);
    }
}

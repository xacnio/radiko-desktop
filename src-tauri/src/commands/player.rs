//! Player control commands: play, stop, pause, resume, volume, status, preview, audio level.

use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{info, warn};

use crate::error::AppError;
use crate::events;
use crate::player;
use crate::player::types::{PlaybackStatus, StatusResponse};
use crate::settings::Settings;
use crate::state::AppState;

use super::{app_data_dir, favicon::download_cover};

#[tauri::command]
pub fn get_proxy_url(state: State<'_, AppState>, url: String) -> String {
    format!(
        "http://127.0.0.1:{}/proxy?url={}",
        state.proxy_port,
        urlencoding::encode(&url)
    )
}

#[tauri::command]
pub async fn play(
    url: String,
    station_name: Option<String>,
    station_image: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // Serialize play requests to prevent race conditions when rapidly switching stations
    let _play_guard = state.play_lock.lock().await;

    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::InvalidUrl("URL cannot be empty".into()));
    }

    info!("Play requested: {}", url);

    // Take handle out of state BEFORE stopping (stop() blocks on join)
    let old_handle = {
        let mut ps = state.inner.lock().unwrap();
        let h = ps.handle.take();
        ps.status = PlaybackStatus::Connecting;
        ps.current_url = Some(url.clone());
        ps.station_name = station_name;
        ps.station_image = station_image.clone();
        ps.stream_metadata = None;
        h
    };

    // Emit connecting BEFORE stopping old player, so SMTC updates immediately
    events::emit_status(&app, PlaybackStatus::Connecting);

    let expected_url = url.clone();
    let app_clone = app.clone();
    if let Some(img_url) = station_image {
        tauri::async_runtime::spawn(async move {
            info!("download_cover: starting download for {}", img_url);
            match download_cover(img_url, app_clone.clone()).await {
                Ok(local_path) => {
                    info!("download_cover: success → {}", local_path);
                    if let Some(app_state) = app_clone.try_state::<AppState>() {
                        if let Ok(mut ps) = app_state.inner.lock() {
                            // Only update if still playing the same station
                            if ps.current_url.as_deref() != Some(expected_url.as_str()) {
                                info!("download_cover: station changed, discarding");
                                return;
                            }
                            ps.station_image = Some(local_path);
                        }
                    }
                    // Refresh OS metadata
                    if let Some(ms) = app_clone.try_state::<crate::services::media::MediaSession>()
                    {
                        if let Some(app_state) = app_clone.try_state::<AppState>() {
                            if let Ok(ps) = app_state.inner.lock() {
                                let artist = ps.station_name.as_deref().unwrap_or("Radiko Desktop");
                                let title_opt =
                                    ps.stream_metadata.as_ref().and_then(|m| m.title.clone());
                                let title = title_opt.as_deref().unwrap_or(artist);
                                let cover_url = ps.station_image.as_deref();
                                info!(
                                    "download_cover: re-setting metadata with cover={:?}",
                                    cover_url
                                );
                                ms.set_metadata(title, artist, cover_url);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("download_cover: failed: {}", e);
                }
            }
        });
    }
    if let Some(handle) = old_handle {
        handle.stop();
    }

    let volume = state.inner.lock().unwrap().volume;
    let handle = player::start(url.clone(), volume * volume, app.clone(), true).await?;

    {
        let mut ps = state.inner.lock().unwrap();
        ps.handle = Some(handle);
    }

    // Persist last URL
    if let Ok(dir) = app_data_dir(&app) {
        let ps = state.inner.lock().unwrap();
        let mut settings = Settings::load(&dir);
        settings.volume = ps.volume;
        settings.last_url = ps.current_url.clone();
        let _ = settings.save(&dir);
    }

    Ok(())
}

#[tauri::command]
pub async fn stop(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    info!("Stop requested");

    let old_handle = {
        let mut ps = state.inner.lock().unwrap();
        let h = ps.handle.take();
        ps.status = PlaybackStatus::Stopped;
        ps.stream_metadata = None;
        h
    };
    if let Some(handle) = old_handle {
        handle.stop();
    }

    events::emit_status(&app, PlaybackStatus::Stopped);
    Ok(())
}

#[tauri::command]
pub async fn preview_play(
    url: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::InvalidUrl("URL cannot be empty".into()));
    }

    // STOP THE MAIN PLAYER FIRST
    let main_handle = {
        let mut ps = state.inner.lock().unwrap();
        let h = ps.handle.take();
        ps.status = PlaybackStatus::Stopped;
        ps.stream_metadata = None;
        h
    };
    if let Some(h) = main_handle {
        h.stop();
    }
    events::emit_status(&app, PlaybackStatus::Stopped);

    // Stop current preview if any
    let old_preview = {
        let mut ps = state.inner.lock().unwrap();
        ps.preview_handle.take()
    };
    if let Some(h) = old_preview {
        h.stop();
    }

    let volume = state.inner.lock().unwrap().volume;
    // Preview uses 60% of current volume to be subtle
    let preview_volume = (volume * 0.6).powi(2);

    match player::start(url, preview_volume, app, false).await {
        Ok(handle) => {
            let mut ps = state.inner.lock().unwrap();
            ps.preview_handle = Some(handle);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn preview_stop(state: State<'_, AppState>) -> Result<(), AppError> {
    let handle = {
        let mut ps = state.inner.lock().unwrap();
        ps.preview_handle.take()
    };
    if let Some(h) = handle {
        h.stop();
    }
    Ok(())
}

#[tauri::command]
pub async fn pause(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let ps = state.inner.lock().unwrap();
    if let Some(ref handle) = ps.handle {
        handle.sink.pause();
        drop(ps);

        let mut ps = state.inner.lock().unwrap();
        ps.status = PlaybackStatus::Paused;
        drop(ps);

        events::emit_status(&app, PlaybackStatus::Paused);
        Ok(())
    } else {
        Err(AppError::InvalidState("not playing".into()))
    }
}

#[tauri::command]
pub async fn resume(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let ps = state.inner.lock().unwrap();
    if let Some(ref handle) = ps.handle {
        handle.sink.play();
        drop(ps);

        let mut ps = state.inner.lock().unwrap();
        ps.status = PlaybackStatus::Playing;
        drop(ps);

        events::emit_status(&app, PlaybackStatus::Playing);
        Ok(())
    } else {
        Err(AppError::InvalidState("not playing".into()))
    }
}

#[tauri::command]
pub async fn set_volume(
    level: f32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let level = level.clamp(0.0, 1.0);

    let (current_url, volume) = {
        let mut ps = state.inner.lock().unwrap();
        ps.volume = level;
        if let Some(ref handle) = ps.handle {
            handle.sink.set_volume(level * level);
        }
        (ps.current_url.clone(), ps.volume)
    };

    if let Ok(dir) = app_data_dir(&app) {
        let mut settings = Settings::load(&dir);
        settings.volume = volume;
        settings.last_url = current_url;
        let _ = settings.save(&dir);
    }
    
    let _ = app.emit("volume-changed", volume);

    Ok(())
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusResponse, AppError> {
    let ps = state.inner.lock().unwrap();
    Ok(StatusResponse {
        status: ps.status,
        url: ps.current_url.clone(),
        volume: ps.volume,
        metadata: ps.stream_metadata.clone(),
        station_name: ps.station_name.clone(),
        station_image: ps.station_image.clone(),
    })
}

/// Re-trigger metadata enrichment for the currently playing song.
/// Called after frontend refresh so cover art / links are re-emitted.
#[tauri::command]
pub async fn re_enrich(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let (title, station_name) = {
        let ps = state.inner.lock().unwrap();
        let t = ps
            .stream_metadata
            .as_ref()
            .and_then(|m| m.title.clone())
            .unwrap_or_default();
        let s = ps
            .station_name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        (t, s)
    };
    if !title.is_empty() {
        tokio::spawn(async move {
            crate::services::enricher::enrich_metadata_background(app, title, station_name).await;
        });
    }
    Ok(())
}

// ===========================================================================
// Audio level (lock-free read from global atomic)
// ===========================================================================

#[tauri::command]
pub fn get_audio_level() -> f32 {
    use std::sync::atomic::Ordering;
    f32::from_bits(crate::player::decoder::AUDIO_LEVEL.load(Ordering::Relaxed))
}

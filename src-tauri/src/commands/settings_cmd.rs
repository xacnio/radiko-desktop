//! Settings, misc utility commands: get/save settings, reset, open URL, fetch listeners, get OS.

use tauri::{AppHandle, Manager};
use tracing::info;

use crate::error::AppError;
use crate::settings::Settings;

use super::app_data_dir;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, AppError> {
    let dir = app_data_dir(&app)?;
    Ok(Settings::load(&dir))
}

#[tauri::command]
pub fn save_sort_order(
    sort_by: Option<String>,
    sort_order: Option<String>,
    app: AppHandle,
) -> Result<(), AppError> {
    println!(
        "[RUST] save_sort_order called. Mode: {:?}, Order: {:?}",
        sort_by, sort_order
    );
    let dir = app_data_dir(&app)?;
    let mut settings = Settings::load(&dir);

    settings.sort_by = sort_by;
    settings.sort_order = sort_order;

    // Antivirus-friendly atomic write: Write to .tmp then rename
    let json =
        serde_json::to_string_pretty(&settings).map_err(|e| AppError::Settings(e.to_string()))?;
    let temp_p = dir.join("settings.json.tmp");
    let target_p = dir.join("settings.json");

    std::fs::write(&temp_p, json).map_err(|e| {
        AppError::Settings(format!("Could not write temporary settings file: {}", e))
    })?;
    std::fs::rename(&temp_p, &target_p).map_err(|e| {
        AppError::Settings(format!(
            "Could not update settings (Antivirus block?): {}",
            e
        ))
    })?;

    Ok(())
}

#[tauri::command]
pub fn save_tray_settings(
    app: AppHandle,
    minimize_to_tray: bool,
    close_to_tray: bool,
) -> Result<(), AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut settings = Settings::load(&dir);

    settings.minimize_to_tray = minimize_to_tray;
    settings.close_to_tray = close_to_tray;

    // Update runtime state too
    let state = app.state::<crate::state::AppState>();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.minimize_to_tray = minimize_to_tray;
        inner.close_to_tray = close_to_tray;
    }

    settings.save(&dir)
}

#[tauri::command]
pub fn save_language(app: AppHandle, lang: String) -> Result<(), AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut settings = Settings::load(&dir);
    settings.language = Some(lang.clone());
    let res = settings.save(&dir);
    if res.is_ok() {
        use tauri::Emitter;
        let _ = app.emit("language-changed", lang);
    }
    res
}

#[tauri::command]
pub fn save_theme(app: AppHandle, theme: String) -> Result<(), AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut settings = Settings::load(&dir);
    settings.theme = Some(theme);
    settings.save(&dir)
}

#[tauri::command]
pub fn save_skip_ads(app: AppHandle, skip_ads: bool) -> Result<(), AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut settings = Settings::load(&dir);
    settings.skip_ads = skip_ads;

    // Update runtime state too
    let state = app.state::<crate::state::AppState>();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.skip_ads = skip_ads;
    }

    settings.save(&dir)
}

#[tauri::command]
pub fn get_audio_devices() -> Result<Vec<String>, AppError> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let host = rodio::cpal::default_host();
    
    let mut devices = Vec::new();
    match host.output_devices() {
        Ok(output_devices) => {
            for device in output_devices {
                if let Ok(name) = device.name() {
                    devices.push(name);
                }
            }
        }
        Err(e) => return Err(AppError::AudioOutput(format!("Failed to enumerate output devices: {}", e))),
    };
    
    Ok(devices)
}

#[tauri::command]
pub async fn set_audio_device(
    app: AppHandle,
    device: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<(), AppError> {
    let dir = crate::commands::app_data_dir(&app)?;
    let mut settings = Settings::load(&dir);

    let device_opt = if device.trim().is_empty() { None } else { Some(device.clone()) };
    settings.output_device = device_opt.clone();

    // Update runtime state too
    let (was_playing, url, station_name, station_image) = {
        let mut inner = state.inner.lock().unwrap();
        inner.output_device = device_opt;
        
        let playing = inner.status == crate::player::types::PlaybackStatus::Playing 
            || inner.status == crate::player::types::PlaybackStatus::Connecting
            || inner.status == crate::player::types::PlaybackStatus::Reconnecting;
            
        (
            playing,
            inner.current_url.clone(),
            inner.station_name.clone(),
            inner.station_image.clone()
        )
    };

    settings.save(&dir)?;

    // If something was playing, restart playback with the new device immediately
    if was_playing {
        if let Some(target_url) = url {
            crate::commands::play(
                target_url,
                station_name,
                station_image,
                app.clone(),
                state,
            ).await?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn open_browser_url(url: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn()
            .map_err(|e| AppError::Settings(e.to_string()))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| AppError::Settings(e.to_string()))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| AppError::Settings(e.to_string()))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn reset_setup(app: AppHandle) -> Result<(), AppError> {
    info!("Reset setup requested");
    let dir = app_data_dir(&app)?;

    // 1. Delete all radio files (they start with radio_)
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("radio_")
                        || name.starts_with("custom_")
                        || name == "settings.json"
                        || name == "identified_songs.json"
                    {
                        info!("Deleting: {:?}", name);
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
    }
    // 2. Mark WebView data for deletion on next launch
    if let Ok(data_dir) = app.path().app_data_dir() {
        let flag = data_dir.join(".pending_reset");
        let _ = std::fs::write(&flag, "reset");
        info!("Pending reset flag written to {:?}", flag);
    }

    Ok(())
}

#[tauri::command]
pub async fn fetch_live_listeners(url: String) -> Result<Option<u32>, AppError> {
    let parsed_url = reqwest::Url::parse(&url).map_err(|e| AppError::InvalidUrl(e.to_string()))?;
    let base_url = format!(
        "{}://{}:{}",
        parsed_url.scheme(),
        parsed_url.host_str().unwrap_or(""),
        parsed_url.port_or_known_default().unwrap_or(80)
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    // Try Shoutcast 7.html
    let sc_url = format!("{}/7.html", base_url);
    if let Ok(resp) = client
        .get(&sc_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
    {
        let resp: reqwest::Response = resp;
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                let text: String = text;
                let mut cleaned = text.clone();
                while let Some(start) = cleaned.find('<') {
                    if let Some(end) = cleaned[start..].find('>') {
                        cleaned.replace_range(start..start + end + 1, "");
                    } else {
                        break;
                    }
                }
                let cleaned = cleaned.trim().to_string();
                let parts: Vec<&str> = cleaned.split(',').collect();

                if !parts.is_empty() {
                    if let Ok(listeners) = parts[0].parse::<u32>() {
                        return Ok(Some(listeners));
                    }
                }
            }
        }
    }

    // Try Icecast status-json.xsl
    let ic_url = format!("{}/status-json.xsl", base_url);
    if let Ok(resp) = client
        .get(&ic_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
    {
        let resp: reqwest::Response = resp;
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let json: serde_json::Value = json;
                let path = parsed_url.path();
                if let Some(icestats) = json.get("icestats") {
                    if let Some(source) = icestats.get("source") {
                        if let Some(source_array) = source.as_array() {
                            for s in source_array {
                                if let Some(listenurl_val) = s.get("listenurl") {
                                    let listenurl = listenurl_val.as_str().unwrap_or("");
                                    if listenurl.ends_with(path) {
                                        if let Some(listeners_val) = s.get("listeners") {
                                            if let Some(l) = listeners_val.as_u64() {
                                                return Ok(Some(l as u32));
                                            }
                                        }
                                    }
                                }
                            }
                        } else if let Some(source_obj) = source.as_object() {
                            if let Some(listeners_val) = source_obj.get("listeners") {
                                if let Some(l) = listeners_val.as_u64() {
                                    return Ok(Some(l as u32));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[tauri::command]
pub fn get_os() -> String {
    std::env::consts::OS.to_string()
}

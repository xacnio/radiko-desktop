//! Settings, misc utility commands: get/save settings, reset, open URL, fetch listeners, get OS.

use tracing::info;
use tauri::{AppHandle, Manager};

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
    app: AppHandle
) -> Result<(), AppError> {
    println!("[RUST] save_sort_order called. Mode: {:?}, Order: {:?}", sort_by, sort_order);
    let dir = app_data_dir(&app)?;
    let mut settings = Settings::load(&dir);
    
    settings.sort_by = sort_by;
    settings.sort_order = sort_order;
    
    // Antivirus-friendly atomic write: Write to .tmp then rename
    let json = serde_json::to_string_pretty(&settings).map_err(|e| AppError::Settings(e.to_string()))?;
    let temp_p = dir.join("settings.json.tmp");
    let target_p = dir.join("settings.json");
    
    std::fs::write(&temp_p, json).map_err(|e| AppError::Settings(format!("Could not write temporary settings file: {}", e)))?;
    std::fs::rename(&temp_p, &target_p).map_err(|e| AppError::Settings(format!("Could not update settings (Antivirus block?): {}", e)))?;
    
    Ok(())
}

#[tauri::command]
pub fn save_language(app: AppHandle, lang: String) -> Result<(), AppError> {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut settings = Settings::load(&dir);
    settings.language = Some(lang);
    settings.save(&dir)
}

#[tauri::command]
pub fn save_theme(app: AppHandle, theme: String) -> Result<(), AppError> {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut settings = Settings::load(&dir);
    settings.theme = Some(theme);
    settings.save(&dir)
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
                    if name.starts_with("radio_") || name.starts_with("custom_") || name == "settings.json" || name == "identified_songs.json" {
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
    let base_url = format!("{}://{}:{}", parsed_url.scheme(), parsed_url.host_str().unwrap_or(""), parsed_url.port_or_known_default().unwrap_or(80));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    // Try Shoutcast 7.html
    let sc_url = format!("{}/7.html", base_url);
    if let Ok(resp) = client.get(&sc_url).header("User-Agent", "Mozilla/5.0").send().await {
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
    if let Ok(resp) = client.get(&ic_url).header("User-Agent", "Mozilla/5.0").send().await {
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

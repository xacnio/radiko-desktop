//! Custom station CRUD + favorites commands.

use tauri::AppHandle;

use crate::error::AppError;
use crate::services::stations;

use super::app_data_dir;

#[tauri::command]
pub async fn get_custom_stations(app: AppHandle) -> Result<Vec<stations::Station>, AppError> {
    let dir = app_data_dir(&app)?;
    let p = dir.join("custom_stations.json");
    if !p.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&p).map_err(|e| AppError::Settings(e.to_string()))?;
    let mut list: Vec<stations::Station> = serde_json::from_str(&data).unwrap_or_default();
    
    // Silently remove missing cached files to prevent front-end 404 logs
    let mut changed = false;
    for s in list.iter_mut() {
        if s.favicon.starts_with("file://") {
            let path_str = if cfg!(target_os = "windows") {
                if s.favicon.starts_with("file:///") { &s.favicon[8..] } else { &s.favicon[7..] }
            } else {
                &s.favicon[7..]
            };
            if !std::path::Path::new(path_str).exists() {
                s.favicon = String::new();
                changed = true;
            }
        }
    }
    
    if changed {
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(&p, json);
        }
    }
    
    Ok(list)
}

#[tauri::command]
pub async fn save_custom_station(mut station: stations::Station, app: AppHandle) -> Result<(), AppError> {
    let mut list = get_custom_stations(app.clone()).await.unwrap_or_default();
    
    if let Some(pos) = list.iter().position(|s| s.stationuuid == station.stationuuid) {
        // preserve indices if not provided by frontend
        if station.all_index == 0 { station.all_index = list[pos].all_index; }
        if station.fav_index == 0 { station.fav_index = list[pos].fav_index; }
        list[pos] = station;
    } else {
        // New station: assign next indices
        let max_all = list.iter().map(|s| s.all_index).max().unwrap_or(0);
        let max_fav = list.iter().map(|s| s.fav_index).max().unwrap_or(0);
        station.all_index = max_all + 1;
        station.fav_index = max_fav + 1;
        list.push(station);
    }
    
    let p = app_data_dir(&app)?.join("custom_stations.json");
    let json = serde_json::to_string_pretty(&list).map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::write(&p, json).map_err(|e| AppError::Settings(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn save_custom_stations_batch(stations: Vec<stations::Station>, app: AppHandle) -> Result<usize, AppError> {
    let mut list = get_custom_stations(app.clone()).await.unwrap_or_default();
    let count = stations.len();
    
    for mut station in stations {
        let max_all = list.iter().map(|s| s.all_index).max().unwrap_or(0);
        let max_fav = list.iter().map(|s| s.fav_index).max().unwrap_or(0);
        station.all_index = max_all + 1;
        station.fav_index = max_fav + 1;
        list.push(station);
    }
    
    let p = app_data_dir(&app)?.join("custom_stations.json");
    let json = serde_json::to_string_pretty(&list).map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::write(&p, json).map_err(|e| AppError::Settings(e.to_string()))?;
    
    Ok(count)
}

#[tauri::command]
pub async fn update_station_indices(updates: Vec<stations::Station>, app: AppHandle) -> Result<(), AppError> {
    let mut list = get_custom_stations(app.clone()).await.unwrap_or_default();
    
    for update in updates {
        if let Some(pos) = list.iter().position(|s| s.stationuuid == update.stationuuid) {
            list[pos].all_index = update.all_index;
            list[pos].fav_index = update.fav_index;
        }
    }
    
    let p = app_data_dir(&app)?.join("custom_stations.json");
    let json = serde_json::to_string_pretty(&list).map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::write(&p, json).map_err(|e| AppError::Settings(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_custom_station(uuid: String, app: AppHandle) -> Result<(), AppError> {
    let mut list = get_custom_stations(app.clone()).await.unwrap_or_default();
    list.retain(|s| s.stationuuid != uuid);
    let p = app_data_dir(&app)?.join("custom_stations.json");
    let json = serde_json::to_string_pretty(&list).map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::write(&p, json).map_err(|e| AppError::Settings(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn clear_missing_favicon(uuid: String, app: AppHandle) -> Result<(), AppError> {
    let mut list = get_custom_stations(app.clone()).await.unwrap_or_default();
    let mut changed = false;
    if let Some(pos) = list.iter().position(|s| s.stationuuid == uuid) {
        if list[pos].favicon.starts_with("file://") || list[pos].favicon.starts_with("asset://") {
            list[pos].favicon = "".to_string();
            changed = true;
        }
    }
    if changed {
        let p = app_data_dir(&app)?.join("custom_stations.json");
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(&p, json);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(uuid: String, app: AppHandle) -> Result<(), AppError> {
    let mut list = get_custom_stations(app.clone()).await.unwrap_or_default();
    if let Some(pos) = list.iter().position(|s| s.stationuuid == uuid) {
        list[pos].is_favorite = !list[pos].is_favorite;
        let p = app_data_dir(&app)?.join("custom_stations.json");
        let json = serde_json::to_string_pretty(&list).map_err(|e| AppError::Settings(e.to_string()))?;
        std::fs::write(&p, json).map_err(|e| AppError::Settings(e.to_string()))?;
    }
    Ok(())
}

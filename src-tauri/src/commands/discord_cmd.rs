//! Discord Rich Presence commands

use tauri::{AppHandle, State};
use crate::error::AppError;
use crate::settings::Settings;
use crate::state::AppState;

#[tauri::command]
pub async fn set_discord_rpc(
    app: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let dir = crate::commands::app_data_dir(&app)?;
    let mut settings = Settings::load(&dir);
    
    settings.discord_rpc = enabled;
    settings.save(&dir)?;
    
    // Update runtime state
    state.discord_rpc.set_enabled(enabled);
    
    // If enabled and playing, update presence immediately
    if enabled {
        if let Ok(ps) = state.inner.lock() {
            if let Some(ref station_name) = ps.station_name {
                let meta = ps.stream_metadata.as_ref().and_then(|m| m.title.as_deref());
                let enriched = ps.enriched_cover.as_deref();
                let album_name = ps.enriched_album.as_deref();
                state.discord_rpc.update_presence(station_name, meta, enriched, album_name);
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
pub fn get_discord_rpc_status(state: State<'_, AppState>) -> bool {
    state.discord_rpc.is_enabled()
}

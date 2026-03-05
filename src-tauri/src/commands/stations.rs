//! Station directory commands (radio-browser.info API).

use crate::error::AppError;
use crate::services::stations;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_stations(
    name: Option<String>,
    country: Option<String>,
    state: Option<String>,
    language: Option<String>,
    tag: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    hide_broken: Option<bool>,
    only_verified: Option<bool>,
) -> Result<Vec<stations::Station>, AppError> {
    stations::search(name, country, state, language, tag, limit, offset, hide_broken, only_verified).await
}

#[tauri::command]
pub async fn get_top_stations(limit: Option<u32>) -> Result<Vec<stations::Station>, AppError> {
    stations::top_stations(limit).await
}

#[tauri::command]
pub async fn get_countries() -> Result<Vec<stations::CountryItem>, AppError> {
    stations::countries().await
}

#[tauri::command]
pub async fn get_languages() -> Result<Vec<stations::LanguageItem>, AppError> {
    stations::languages().await
}

#[tauri::command]
pub async fn get_tags(limit: Option<u32>) -> Result<Vec<stations::TagItem>, AppError> {
    stations::tags(limit).await
}

#[tauri::command]
pub async fn get_states(country: String) -> Result<Vec<stations::StateItem>, AppError> {
    stations::states(country).await
}

#[tauri::command]
pub async fn get_all_country_stations(country: String) -> Result<Vec<stations::Station>, AppError> {
    stations::all_country_stations(country).await
}

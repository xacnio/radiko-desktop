use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub volume: f32,
    pub last_url: Option<String>,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            last_url: None,
            sort_by: Some("manual".to_string()),
            sort_order: Some("asc".to_string()),
            language: None,
            theme: None,
        }
    }
}

impl Settings {
    /// Load settings from disk. Returns defaults if file is missing or corrupt.
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join("settings.json");
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist settings to disk. Creates the directory if needed.
    pub fn save(&self, app_data_dir: &Path) -> Result<(), AppError> {
        fs::create_dir_all(app_data_dir)
            .map_err(|e| AppError::Settings(e.to_string()))?;
        let path = app_data_dir.join("settings.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Settings(e.to_string()))?;
        fs::write(&path, content)
            .map_err(|e| AppError::Settings(e.to_string()))?;
        Ok(())
    }
}

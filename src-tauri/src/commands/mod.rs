//! Tauri command handlers — thin wrappers over Player and State.
//!
//! Split into feature-based submodules for maintainability.

mod player;
mod eq;
mod stations;
mod custom_stations;
mod favicon;
mod browser;
mod recognition;
mod scraping;
mod settings_cmd;
mod backup;

// ── Shared helpers used across submodules ────────────────────────────

use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;
use crate::error::AppError;

/// Convert a local path to a `file:///` URL with forward slashes (required on Windows).
pub(crate) fn path_to_file_url(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", s)
}

/// Helper to get the app data directory from the AppHandle.
pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let path = app.path()
        .app_data_dir()
        .map_err(|e| AppError::Settings(e.to_string()))?;
        
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| AppError::Settings(e.to_string()))?;
    }
    
    Ok(path)
}

// ── Re-exports: every pub command is re-exported so lib.rs stays unchanged ──

pub use self::player::*;
pub use self::eq::*;
pub use self::stations::*;
pub use self::custom_stations::*;
pub use self::favicon::*;
pub use self::browser::*;
pub use self::recognition::*;
pub use self::scraping::*;
pub use self::settings_cmd::*;
pub use self::backup::*;

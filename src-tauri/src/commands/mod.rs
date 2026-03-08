//! Tauri command handlers — thin wrappers over Player and State.
//!
//! Split into feature-based submodules for maintainability.

mod backup;
mod browser;
mod custom_stations;
mod discord_cmd;
mod eq;
mod favicon;
mod player;
mod recognition;
mod scraping;
mod settings_cmd;
mod stations;

// ── Shared helpers used across submodules ────────────────────────────

use crate::error::AppError;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

/// Convert a local path to a `file:///` URL with forward slashes (required on Windows).
pub(crate) fn path_to_file_url(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", s)
}

/// Helper to get the app data directory from the AppHandle.
pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Settings(e.to_string()))?;

    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| AppError::Settings(e.to_string()))?;
    }

    Ok(path)
}

// ── Re-exports: every pub command is re-exported so lib.rs stays unchanged ──

pub use self::backup::*;
pub use self::browser::*;
pub use self::custom_stations::*;
pub use self::discord_cmd::*;
pub use self::eq::*;
pub use self::favicon::*;
pub use self::player::*;
pub use self::recognition::*;
pub use self::scraping::*;
pub use self::settings_cmd::*;
pub use self::stations::*;

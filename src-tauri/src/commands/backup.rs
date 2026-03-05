//! Backup/restore commands: export, import, analyze.

use std::path::PathBuf;
use std::fs::File;
use std::io::{Read, Write};
use tracing::info;
use tauri::{AppHandle, Emitter, Manager};
use zip::write::SimpleFileOptions;

use crate::error::AppError;
use crate::services::stations;

use super::{path_to_file_url, app_data_dir};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct BackupOptions {
    pub include_radios: bool,
    pub include_songs: bool,
    pub include_images: bool,
}

#[derive(serde::Serialize)]
pub struct BackupMetadata {
    pub radio_count: usize,
    pub song_count: usize,
    pub has_images: bool,
    pub path: String,
}

#[tauri::command]
pub async fn analyze_backup() -> Result<Option<BackupMetadata>, AppError> {
    let open_path = rfd::AsyncFileDialog::new()
        .set_title("Select Backup File")
        .add_filter("Zip", &["zip"])
        .pick_file()
        .await;

    let path = match open_path {
        Some(p) => p.path().to_path_buf(),
        None => return Ok(None),
    };

    let file = File::open(&path).map_err(|e| AppError::Settings(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| AppError::Settings(e.to_string()))?;

    let mut radio_count = 0;
    let mut song_count = 0;
    let mut has_images = false;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| AppError::Settings(e.to_string()))?;
        let name = file.name().to_owned();

        if name == "custom_stations.json" {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                if let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                    radio_count = list.len();
                }
            }
        } else if name == "identified_songs.json" {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                if let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                    song_count = list.len();
                }
            }
        } else if name.starts_with("images/") {
            has_images = true;
        }
    }
    
    drop(archive);

    Ok(Some(BackupMetadata {
        radio_count,
        song_count,
        has_images,
        path: path.to_string_lossy().to_string(),
    }))
}

#[tauri::command]
pub async fn export_backup(options: BackupOptions, app: AppHandle) -> Result<(), AppError> {
    let data_dir = app_data_dir(&app)?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| AppError::Settings(e.to_string()))?;

    let mut temp = tempfile::NamedTempFile::new().map_err(|e| AppError::Settings(e.to_string()))?;
    
    {
        let mut zip = zip::ZipWriter::new(&mut temp);
        let options_zip = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let mut total_items = 0;
        if options.include_radios { total_items += 1; }
        if options.include_songs { total_items += 1; }
        if options.include_images && cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                total_items += entries.flatten().filter(|e| e.path().is_file() && e.file_name().to_string_lossy().starts_with("custom_")).count();
            }
        }
        
        let mut current_item = 0;
        let mut report_progress = |inc: u32, app: &AppHandle| {
            current_item += inc;
            if total_items > 0 {
                let p = (current_item as f64 / total_items as f64 * 100.0) as u32;
                let _ = app.emit("export-progress", p);
            }
        };

        // 1. Stations JSON
        if options.include_radios {
            let stations_p = data_dir.join("custom_stations.json");
            if stations_p.exists() {
                let mut content = std::fs::read(&stations_p).unwrap_or_default();
                if !options.include_images {
                    if let Ok(mut list) = serde_json::from_slice::<Vec<stations::Station>>(&content) {
                        for s in list.iter_mut() {
                            if s.favicon.starts_with("file:///") { s.favicon = String::new(); }
                        }
                        if let Ok(updated) = serde_json::to_vec_pretty(&list) { content = updated; }
                    }
                }
                zip.start_file("custom_stations.json", options_zip).map_err(|e| AppError::Settings(e.to_string()))?;
                zip.write_all(&content).map_err(|e| AppError::Settings(e.to_string()))?;
            }
            report_progress(1, &app);
        }

        // 2. Identified Songs JSON
        if options.include_songs {
            let identified_p = data_dir.join("identified_songs.json");
            if identified_p.exists() {
                let content = std::fs::read(&identified_p).unwrap_or_default();
                zip.start_file("identified_songs.json", options_zip).map_err(|e| AppError::Settings(e.to_string()))?;
                zip.write_all(&content).map_err(|e| AppError::Settings(e.to_string()))?;
            }
            report_progress(1, &app);
        }

        // 3. Images folder
        if options.include_images && cache_dir.exists() {
            for entry in std::fs::read_dir(&cache_dir).map_err(|e| AppError::Settings(e.to_string()))?.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("custom_") {
                            let content = std::fs::read(&p).unwrap_or_default();
                            zip.start_file(format!("images/{}", name), options_zip).map_err(|e| AppError::Settings(e.to_string()))?;
                            zip.write_all(&content).map_err(|e| AppError::Settings(e.to_string()))?;
                            report_progress(1, &app);
                        }
                    }
                }
            }
        }

        zip.finish().map_err(|e| AppError::Settings(e.to_string()))?;
        let _ = app.emit("export-progress", 100);
    }

    let save_path = rfd::AsyncFileDialog::new()
        .set_title("Save Backup")
        .add_filter("Zip", &["zip"])
        .set_file_name("radiko_backup.zip")
        .save_file()
        .await;

    let path = match save_path {
        Some(p) => p.path().to_path_buf(),
        None => return Ok(()),
    };

    std::fs::copy(temp.path(), &path).map_err(|e| AppError::Settings(e.to_string()))?;
    
    info!("Backup exported to {:?}", path);
    Ok(())
}

#[tauri::command]
pub async fn import_backup(path: String, options: BackupOptions, app: AppHandle) -> Result<(), AppError> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(AppError::Settings("Backup file no longer exists".into()));
    }

    let data_dir = app_data_dir(&app).map_err(|e| AppError::Settings(e.to_string()))?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::create_dir_all(&cache_dir).unwrap_or_default();

    let file = File::open(&path).map_err(|e| AppError::Settings(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| AppError::Settings(e.to_string()))?;
    let total_files = archive.len();

    for i in 0..total_files {
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;

        let result: Result<(), AppError> = {
            let progress = ((i + 1) as f64 / total_files as f64 * 100.0) as u32;
            let _ = app.emit("import-progress", progress);

            let mut file = archive.by_index(i).map_err(|e| AppError::Settings(e.to_string()))?;
            let outpath = match file.enclosed_name() {
                Some(path) => path.to_owned(),
                None => return Ok(()),
            };

            if file.is_dir() {
                Ok(())
            } else if outpath.to_string_lossy() == "custom_stations.json" && options.include_radios {
                let mut content = String::new();
                file.read_to_string(&mut content).map_err(|e| AppError::Settings(format!("Read error: {}", e)))?;
                
                let mut list: Vec<stations::Station> = serde_json::from_str(&content).unwrap_or_default();
                for s in list.iter_mut() {
                    if s.favicon.starts_with("file:///") {
                        if let Some(name) = PathBuf::from(&s.favicon.replace("file:///", "")).file_name().and_then(|n| n.to_str()) {
                            let new_p = cache_dir.join(name);
                            s.favicon = path_to_file_url(&new_p);
                        }
                    }
                }
                let updated_json = serde_json::to_string_pretty(&list).map_err(|e| AppError::Settings(e.to_string()))?;
                
                let temp_p = data_dir.join("custom_stations.json.tmp");
                let target_p = data_dir.join("custom_stations.json");
                std::fs::write(&temp_p, updated_json).map_err(|e| AppError::Settings(format!("Could not write temporary file: {}", e)))?;
                std::fs::rename(&temp_p, &target_p).map_err(|e| AppError::Settings(format!("Radio list could not be updated (Access Denied/Antivirus?): {}", e)))?;
                Ok(())

            } else if outpath.to_string_lossy() == "identified_songs.json" && options.include_songs {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).map_err(|e| AppError::Settings(format!("Read error: {}", e)))?;
                
                let temp_p = data_dir.join("identified_songs.json.tmp");
                let target_p = data_dir.join("identified_songs.json");
                std::fs::write(&temp_p, buf).map_err(|e| AppError::Settings(format!("Could not write temporary file: {}", e)))?;
                std::fs::rename(&temp_p, &target_p).map_err(|e| AppError::Settings(format!("Song list could not be updated: {}", e)))?;
                Ok(())

            } else if outpath.starts_with("images/") && options.include_images {
                let name = outpath.strip_prefix("images/").unwrap_or(&outpath);
                let target = cache_dir.join(name);
                let temp_target = target.with_extension("tmp_img");
                {
                    let mut outfile = File::create(&temp_target).map_err(|e| AppError::Settings(format!("Could not create image file: {}", e)))?;
                    std::io::copy(&mut file, &mut outfile).map_err(|e| AppError::Settings(format!("Could not copy image: {}", e)))?;
                }
                let _ = std::fs::rename(&temp_target, &target);
                Ok(())
            } else {
                Ok(())
            }
        };

        result?;
    }

    info!("Backup imported from {:?}", path);
    Ok(())
}

//! Music recognition (Shazam-style) commands.

use tracing::{info, error};
use tauri::{Emitter, Manager};

use crate::error::AppError;
use crate::state::AppState;

static IDENTIFYING_NOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
pub fn get_identified_songs(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = data_dir.join("identified_songs.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => vec![],
    }
}

#[tauri::command]
pub fn save_identified_song(app: tauri::AppHandle, song: serde_json::Value) -> Result<(), AppError> {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = data_dir.join("identified_songs.json");
    let mut songs: Vec<serde_json::Value> = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => vec![],
    };

    let mut new_song = song.clone();
    let current_source_name = new_song["source"].as_str().unwrap_or("Unknown").to_string();
    let current_link = new_song["song_link"].as_str().unwrap_or("").to_string();

    // Ensure it has a sources array
    if new_song["sources"].is_null() {
        new_song["sources"] = serde_json::json!([{
            "name": current_source_name,
            "link": current_link
        }]);
    }

    // Advanced Merging Logic
    if let Some(last_song) = songs.get_mut(0) {
        if last_song["artist"] == new_song["artist"] && last_song["title"] == new_song["title"] {
            if let Some(sources) = last_song["sources"].as_array_mut() {
                let already_has_source = sources.iter().any(|s| s["name"] == current_source_name);
                if already_has_source {
                    return Ok(());
                } else {
                    sources.push(serde_json::json!({
                        "name": current_source_name,
                        "link": current_link
                    }));
                    
                    let old_source = last_song["source"].as_str().unwrap_or("");
                    if !old_source.contains(&current_source_name) {
                        last_song["source"] = serde_json::Value::String(format!("{} + {}", old_source, current_source_name));
                    }
                    
                    if last_song["song_link"].as_str().unwrap_or("").is_empty() {
                        last_song["song_link"] = serde_json::Value::String(current_link);
                    }
                    
                    last_song["found_at"] = new_song["found_at"].clone();
                }
            } else {
                let old_source = last_song["source"].as_str().unwrap_or("").to_string();
                let old_link = last_song["song_link"].as_str().unwrap_or("").to_string();
                
                if old_source == current_source_name {
                    return Ok(());
                }

                last_song["sources"] = serde_json::json!([
                    { "name": old_source.clone(), "link": old_link },
                    { "name": current_source_name.clone(), "link": current_link }
                ]);
                last_song["source"] = serde_json::Value::String(format!("{} + {}", old_source, current_source_name));
            }
        } else {
            let exists_already = songs.iter().take(10).any(|item| {
                item["artist"] == new_song["artist"] && 
                item["title"] == new_song["title"] &&
                item["source"].as_str().unwrap_or("").contains(&current_source_name)
            });

            if exists_already {
                return Ok(());
            }

            songs.insert(0, new_song);
        }
    } else {
        songs.insert(0, new_song);
    }
    // Keep at most 100 entries
    songs.truncate(100);
    std::fs::create_dir_all(&data_dir).map_err(|e| AppError::Settings(e.to_string()))?;
    let content = serde_json::to_string_pretty(&songs).map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::write(&path, content).map_err(|e| AppError::Settings(e.to_string()))?;
    
    // Emit event for real-time UI refresh
    let _ = app.emit("history-updated", ());
    
    Ok(())
}

#[tauri::command]
pub fn clear_identified_songs(app: tauri::AppHandle) -> Result<(), AppError> {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = data_dir.join("identified_songs.json");
    std::fs::write(&path, "[]").map_err(|e| AppError::Settings(e.to_string()))?;
    let _ = app.emit("history-updated", ());
    Ok(())
}

#[tauri::command]
pub fn delete_identified_song(app: tauri::AppHandle, song_to_delete: serde_json::Value) -> Result<(), AppError> {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let path = data_dir.join("identified_songs.json");
    let mut songs: Vec<serde_json::Value> = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => vec![],
    };

    let initial_len = songs.len();
    songs.retain(|s| {
        s["title"] != song_to_delete["title"] || 
        s["artist"] != song_to_delete["artist"] || 
        s["found_at"] != song_to_delete["found_at"]
    });

    if songs.len() != initial_len {
        let content = serde_json::to_string_pretty(&songs).map_err(|e| AppError::Settings(e.to_string()))?;
        std::fs::write(&path, content).map_err(|e| AppError::Settings(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn identify_song(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Option<serde_json::Value>, AppError> {
    use std::sync::atomic::Ordering;
    use crate::player::decoder::{CAPTURE_ACTIVE, CAPTURED_SAMPLES, RECOGNITION_SAMPLE_RATE};
    use tauri::Emitter;

    if IDENTIFYING_NOW.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return Ok(Some(serde_json::json!({
            "_error": true,
            "_error_type": "already_running",
            "_message": "An identification process is already in progress."
        })));
    }

    let max_attempts = 3;

    for attempt in 1..=max_attempts {
        if attempt == 1 {
            let _ = app.emit("identify_phase", "recording");
            info!("Music identification: starting capture (attempt {}/{})", attempt, max_attempts);
        } else {
            let _ = app.emit("identify_phase", "retrying");
            info!("Music identification: retrying capture (attempt {}/{})", attempt, max_attempts);
        }

        {
            let mut samples = CAPTURED_SAMPLES.lock().unwrap();
            samples.clear();
        }
        CAPTURE_ACTIVE.store(true, Ordering::Relaxed);

        tokio::time::sleep(std::time::Duration::from_secs(7)).await;

        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        
        let samples = {
            let mut s = CAPTURED_SAMPLES.lock().unwrap();
            std::mem::take(&mut *s)
        };

        if samples.is_empty() {
            if attempt == max_attempts {
                IDENTIFYING_NOW.store(false, Ordering::Release);
                return Err(AppError::InvalidState("No audio samples captured. Is the radio playing?".into()));
            } else {
                continue;
            }
        }

        let sample_rate = RECOGNITION_SAMPLE_RATE.load(Ordering::Relaxed);
        info!("Music identification: captured {} samples at {}Hz", samples.len(), sample_rate);

        let _ = app.emit("identify_phase", "encoding");

        let shazam_pcm = if sample_rate != 16000 {
            info!("Resampling to 16000Hz for SongRec (Shazam)...");
            let ratio = sample_rate as f32 / 16000.0;
            let mut resampled = Vec::new();
            let mut i = 0.0;
            while (i as usize) < samples.len() {
                resampled.push(samples[i as usize]);
                i += ratio;
            }
            resampled
        } else {
            samples.clone()
        };

        let mut pcm_i16 = Vec::with_capacity(shazam_pcm.len() + 128);
        for &sample in &shazam_pcm {
            pcm_i16.push((sample.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        
        let remainder = pcm_i16.len() % 128;
        if remainder != 0 {
            let padding = 128 - remainder;
            pcm_i16.extend(std::iter::repeat_n(0, padding));
        }

        info!("Music identification: using FREE SHAZAM (SongRec)...");
        let _ = app.emit("identify_phase", "sending");

        let shazam_result_thread = tauri::async_runtime::spawn_blocking(move || {
            let config = songrec::Config::default();
            let songrec_client = songrec::SongRec::new(config);
            songrec_client.recognize_from_samples(&pcm_i16, 16000)
        }).await;
        
        let shazam_result_thread = match shazam_result_thread {
            Ok(res) => res,
            Err(e) => {
                IDENTIFYING_NOW.store(false, Ordering::Release);
                return Err(AppError::Network(format!("Thread error: {}", e)));
            }
        };

        match shazam_result_thread {
            Ok(rec_result) => {
                info!("Shazam MATCH FOUND: {} - {}", rec_result.artist_name, rec_result.song_name);
                
                let raw = rec_result.raw_response;
                let images = raw.get("track").and_then(|t: &serde_json::Value| t.get("images"));
                let cover = images.and_then(|i: &serde_json::Value| i.get("coverarthq").or(i.get("coverart")).or(i.get("background")))
                    .and_then(|v: &serde_json::Value| v.as_str())
                    .unwrap_or("")
                    .to_string();
                    
                let share_url = raw.get("track").and_then(|t: &serde_json::Value| t.get("share"))
                    .and_then(|v: &serde_json::Value| v.get("href"))
                    .and_then(|v: &serde_json::Value| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let artist = rec_result.artist_name.clone();
                let title = rec_result.song_name.clone();

                {
                    let mut ps = state.inner.lock().unwrap();
                    ps.stream_metadata = Some(crate::player::types::StreamMetadata {
                        title: Some(format!("{} - {}", artist, title)),
                        icy_name: ps.stream_metadata.as_ref().and_then(|m| m.icy_name.clone()),
                        icy_genre: ps.stream_metadata.as_ref().and_then(|m| m.icy_genre.clone()),
                        icy_url: ps.stream_metadata.as_ref().and_then(|m| m.icy_url.clone()),
                        icy_br: ps.stream_metadata.as_ref().and_then(|m| m.icy_br.clone()),
                        icy_listeners: ps.stream_metadata.as_ref().and_then(|m| m.icy_listeners.clone()),
                    });
                }

                IDENTIFYING_NOW.store(false, Ordering::Release);
                return Ok(Some(serde_json::json!({
                    "artist": artist,
                    "title": title,
                    "album": rec_result.album_name.unwrap_or_default(),
                    "release_date": rec_result.release_year.unwrap_or_default(),
                    "cover": cover,
                    "song_link": share_url,
                    "is_shazam": true
                })));
            },
            Err(e) => {
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("no track found") || error_str.contains("unrecognized") || error_str.contains("not found") || error_str.contains("fingerprint error") {
                    info!("Shazam (SongRec) attempt {} failed: No match.", attempt);
                } else {
                    error!("Shazam request error on attempt {}: {}", attempt, error_str);
                }
            }
        }
    }

    info!("Shazam failed after {} attempts.", max_attempts);
    
    IDENTIFYING_NOW.store(false, Ordering::Release);

    Ok(Some(serde_json::json!({
        "_error": true,
        "_error_type": "no_match",
        "_message": "Song could not be identified. Please try again later."
    })))
}

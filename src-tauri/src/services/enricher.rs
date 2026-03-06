use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info};

pub async fn enrich_metadata_background(app: AppHandle, title: String, station_name: String) {
    if title.trim().is_empty() {
        return;
    }

    info!("Enriching metadata in background: {}", title);

    if title.contains(" - ") {
        let client = reqwest::Client::new();
        let query = [("term", title.as_str()), ("limit", "1"), ("media", "music")];

        match client
            .get("https://itunes.apple.com/search")
            .query(&query)
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(result) = json["results"].as_array().and_then(|a| a.first()) {
                        let artist = result["artistName"].as_str().unwrap_or("").to_string();
                        let track = result["trackName"].as_str().unwrap_or("").to_string();
                        let album = result["collectionName"].as_str().unwrap_or("").to_string();
                        let cover = result["artworkUrl100"].as_str().unwrap_or("").to_string();
                        let big_cover = cover.replace("100x100", "600x600");
                        let link = result["trackViewUrl"].as_str().unwrap_or("").to_string();

                        debug!("Enricher: found {} - {}", artist, track);

                        // Emit to frontend
                        let enriched_result = serde_json::json!({
                            "artist": artist,
                            "title": track,
                            "album": album,
                            "cover": big_cover,
                            "song_link": link.clone(),
                            "original_title": title.clone(),
                            "station_name": station_name.clone(),
                            "found_at": chrono::Local::now().to_rfc3339(),
                            "source": "iTunes",
                            "sources": [
                                { "name": "iTunes", "link": link }
                            ]
                        });

                        let _ = app.emit("metadata-enriched", enriched_result.clone());

                        // Save to history automatically
                        let app_handle_clone = app.clone();
                        let history_entry = enriched_result.clone();
                        tokio::spawn(async move {
                            let data_dir = app_handle_clone
                                .path()
                                .app_data_dir()
                                .unwrap_or_else(|_| std::path::PathBuf::from("."));
                            let history_path = data_dir.join("identified_songs.json");

                            // Load current history
                            let mut history: Vec<Value> =
                                if let Ok(content) = std::fs::read_to_string(&history_path) {
                                    serde_json::from_str(&content).unwrap_or_default()
                                } else {
                                    Vec::new()
                                };

                            let current_source_name = "iTunes";
                            let current_link = history_entry["song_link"]
                                .as_str()
                                .unwrap_or("")
                                .to_string();

                            // Advanced Merging Logic
                            let mut was_saved = false;
                            if let Some(last_song) = history.get_mut(0) {
                                if last_song["artist"] == history_entry["artist"]
                                    && last_song["title"] == history_entry["title"]
                                {
                                    // Merge if possible
                                    if let Some(sources) = last_song["sources"].as_array_mut() {
                                        let has_source = sources
                                            .iter()
                                            .any(|s| s["name"] == current_source_name);
                                        if !has_source {
                                            sources.push(serde_json::json!({ "name": current_source_name, "link": current_link }));

                                            // Update display strings
                                            let old_src =
                                                last_song["source"].as_str().unwrap_or("");
                                            if !old_src.contains(current_source_name) {
                                                last_song["source"] =
                                                    serde_json::Value::String(format!(
                                                        "{} + {}",
                                                        old_src, current_source_name
                                                    ));
                                            }
                                            was_saved = true;
                                        }
                                    } else {
                                        // Legacy entry
                                        let old_src =
                                            last_song["source"].as_str().unwrap_or("").to_string();
                                        if old_src != current_source_name {
                                            last_song["sources"] = serde_json::json!([
                                                { "name": old_src.clone(), "link": last_song["song_link"].as_str().unwrap_or("") },
                                                { "name": current_source_name, "link": current_link }
                                            ]);
                                            last_song["source"] = serde_json::Value::String(
                                                format!("{} + {}", old_src, current_source_name),
                                            );
                                            was_saved = true;
                                        }
                                    }
                                }
                            }

                            if !was_saved {
                                // Check dedupe in top 10
                                let exists = history.iter().take(10).any(|item| {
                                    item["artist"] == history_entry["artist"]
                                        && item["title"] == history_entry["title"]
                                        && item["source"]
                                            .as_str()
                                            .unwrap_or("")
                                            .contains(current_source_name)
                                });

                                if !exists {
                                    history.insert(0, history_entry);
                                    history.truncate(100);
                                    was_saved = true;
                                }
                            }

                            if was_saved {
                                let _ = std::fs::create_dir_all(&data_dir);
                                if let Ok(content) = serde_json::to_string_pretty(&history) {
                                    let _ = std::fs::write(&history_path, content);
                                    let _ = app_handle_clone.emit("history-updated", ());
                                }
                            }
                        });

                        // Found on iTunes, return early so we don't run fallback
                    }
                }
            }
            Err(e) => {
                error!("Enricher error (iTunes): {}", e);
            }
        }
    }

    // FALLBACK removed per user request. If iTunes doesn't find it, we do nothing.
}

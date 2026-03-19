//! Favicon / cover image commands: download, upload, batch cache, image search.

use tauri::{AppHandle, Emitter, Manager};
use tracing::info;

use crate::error::AppError;

use super::{app_data_dir, path_to_file_url};

/// Download and cache a cover image, converting to PNG for SMTC compatibility.
/// Used internally by the player command and also exposed for batch operations.
pub(crate) async fn download_cover(url: String, app: AppHandle) -> Result<String, String> {
    if url.starts_with("file:///") {
        return Ok(url);
    }

    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    // Use a hash of the URL as filename — always save as PNG for SMTC compatibility
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();

    let file_path = cache_dir.join(format!("cover_{}.png", hash));

    // If already converted and valid PNG, reuse
    if file_path.exists() {
        if let Ok(header) = std::fs::read(&file_path) {
            if header.len() > 8 && header[..4] == [0x89, 0x50, 0x4E, 0x47] {
                return Ok(path_to_file_url(&file_path));
            }
        }
        let _ = std::fs::remove_file(&file_path);
    }

    let bytes = if url.starts_with("data:image/") {
        // Handle data URL (base64)
        if let Some(comma_pos) = url.find(',') {
            let base64_str = &url[comma_pos + 1..];
            use base64::{engine::general_purpose, Engine as _};
            general_purpose::STANDARD
                .decode(base64_str)
                .map_err(|e| format!("base64 decode failed: {}", e))?
        } else {
            return Err("invalid data url".into());
        }
    } else {
        // Download regular URL
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("download failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.bytes()
            .await
            .map_err(|e| format!("read body failed: {}", e))?
            .to_vec()
    };

    if bytes.is_empty() {
        return Err("empty image data".into());
    }

    // Decode the image (supports png, jpg, webp, gif, ico) and re-encode as PNG
    let img = image::load_from_memory(&bytes).map_err(|e| format!("image decode failed: {}", e))?;

    img.save_with_format(&file_path, image::ImageFormat::Png)
        .map_err(|e| format!("png save failed: {}", e))?;

    info!(
        "download_cover: converted to PNG at {:?} (original {} bytes)",
        file_path,
        bytes.len()
    );

    let result = path_to_file_url(&file_path);

    info!("download_cover: returning {}", result);
    Ok(result)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FaviconEntry {
    pub uuid: String,
    pub url: String,
}

#[tauri::command]
pub async fn batch_cache_favicons(
    entries: Vec<FaviconEntry>,
    app: AppHandle,
) -> Result<std::collections::HashMap<String, String>, AppError> {
    use futures_util::stream::{self, StreamExt};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let total = entries.len() as u32;
    let done = Arc::new(AtomicU32::new(0));

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Connection(e.to_string()))?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| AppError::Connection(e.to_string()))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let results = stream::iter(entries)
        .map(|entry| {
            let client = client.clone();
            let cache_dir = cache_dir.clone();
            let done = done.clone();
            let app = app.clone();
            async move {
                let result = async {
                    if entry.url.is_empty() {
                        return None;
                    }
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    entry.url.hash(&mut hasher);
                    let hash = hasher.finish();
                    let file_path = cache_dir.join(format!("cover_{}.png", hash));

                    // Check cache
                    if file_path.exists() {
                        if let Ok(header) = std::fs::read(&file_path) {
                            if header.len() > 8 && header[..4] == [0x89, 0x50, 0x4E, 0x47] {
                                let p = path_to_file_url(&file_path);
                                return Some((entry.uuid.clone(), p));
                            }
                        }
                        let _ = std::fs::remove_file(&file_path);
                    }

                    let resp = client.get(&entry.url).send().await.ok()?;
                    if !resp.status().is_success() {
                        return None;
                    }
                    let bytes = resp.bytes().await.ok()?;
                    if bytes.is_empty() {
                        return None;
                    }

                    let img = image::load_from_memory(&bytes).ok()?;
                    let thumb = img.thumbnail(64, 64);
                    thumb
                        .save_with_format(&file_path, image::ImageFormat::Png)
                        .ok()?;

                    let p = path_to_file_url(&file_path);
                    Some((entry.uuid.clone(), p))
                }
                .await;

                // Emit progress
                let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit(
                    "favicon-progress",
                    serde_json::json!({
                        "done": current,
                        "total": total,
                    }),
                );

                result
            }
        })
        .buffer_unordered(15)
        .collect::<Vec<_>>()
        .await;

    let map: std::collections::HashMap<String, String> = results.into_iter().flatten().collect();

    info!("batch_cache_favicons: cached {}/{}", map.len(), total);
    Ok(map)
}

#[tauri::command]
pub async fn upload_custom_favicon(
    app: AppHandle,
    bytes: Vec<u8>,
    ext: String,
) -> Result<String, AppError> {
    let dir = app_data_dir(&app)?;
    let p = dir.join(format!(
        "custom_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        ext
    ));
    std::fs::write(&p, bytes).map_err(|e| AppError::Settings(e.to_string()))?;
    Ok(format!(
        "file:///{}",
        p.to_string_lossy().replace("\\", "/")
    ))
}

#[tauri::command]
pub async fn download_custom_favicon(app: AppHandle, url: String) -> Result<String, AppError> {
    // Handle base64 data URIs directly (from Google Image search)
    if url.starts_with("data:image/") {
        // Parse: data:image/png;base64,iVBORw0KGgo...
        let ext = if url.starts_with("data:image/png") {
            "png"
        } else if url.starts_with("data:image/webp") {
            "webp"
        } else {
            "jpg"
        };

        let base64_data = url
            .find(",")
            .map(|pos| &url[pos + 1..])
            .ok_or_else(|| AppError::Settings("Invalid data URI".into()))?;

        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64_data)
            .map_err(|e| AppError::Settings(format!("Base64 decode error: {}", e)))?;

        if bytes.len() < 100 {
            return Err(AppError::Settings("Image too small".into()));
        }

        let dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| AppError::Settings(e.to_string()))?;
        std::fs::create_dir_all(&dir).map_err(|e| AppError::Settings(e.to_string()))?;
        let p = dir.join(format!(
            "custom_dl_{}.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            ext
        ));
        std::fs::write(&p, bytes).map_err(|e| AppError::Settings(e.to_string()))?;
        return Ok(format!(
            "file:///{}",
            p.to_string_lossy().replace("\\", "/")
        ));
    }

    // Regular URL download
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| AppError::Settings(e.to_string()))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Settings(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(AppError::Settings(format!(
            "Download failed: {}",
            resp.status()
        )));
    }

    // Guess extension
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ext = if content_type.contains("png") {
        "png"
    } else if content_type.contains("webp") {
        "webp"
    } else if content_type.contains("gif") {
        "gif"
    } else if content_type.contains("svg") {
        "svg"
    } else {
        "jpg"
    };

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Settings(e.to_string()))?;

    // Ignore truly tiny images (tracking pixels)
    if bytes.len() < 100 {
        return Err(AppError::Settings("Image too small".into()));
    }

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Settings(e.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Settings(e.to_string()))?;
    let p = dir.join(format!(
        "custom_dl_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        ext
    ));
    std::fs::write(&p, bytes).map_err(|e| AppError::Settings(e.to_string()))?;
    Ok(format!(
        "file:///{}",
        p.to_string_lossy().replace("\\", "/")
    ))
}

#[tauri::command]
pub async fn search_images_internal(encoded_query: String) -> Result<Vec<String>, AppError> {
    info!("Searching images for: {}", encoded_query);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
        .build()
        .map_err(|e: reqwest::Error| AppError::Settings(e.to_string()))?;

    let url = format!("https://www.google.com/search?q={}&udm=2&imgar=s&hl=tr", encoded_query);

    let resp = client.get(&url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8")
        .header("Accept-Language", "tr-TR,tr;q=0.9")
        .header("sec-ch-ua", "\"Not:A-Brand\";v=\"99\", \"Google Chrome\";v=\"145\", \"Chromium\";v=\"145\"")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"macOS\"")
        .header("sec-ch-ua-platform-version", "\"13.2.1\"")
        .header("sec-ch-ua-arch", "\"arm\"")
        .header("sec-ch-ua-bitness", "\"64\"")
        .header("sec-fetch-dest", "document")
        .header("sec-fetch-mode", "navigate")
        .header("sec-fetch-site", "none")
        .header("referer", "https://www.google.com/")
        .header("upgrade-insecure-requests", "1")
        .header("cookie", "NID=529=TFkj1-6yb_eR_BwWBnVfQ1EyS1nOwzA1hx7HHWDa_J17AErbXUk33DLNCnTp_-12J9W0YRiPytJsAJg8vQ-bzYHuc8pt-pCGUDPG1lSVhx-9FLSZsvMpS1v6-wtNtrpdmLsmU2oTRu7LGPW5QUCurmAGP6xlOkuzcuaeEVFc4bB2rW5KUQZqdqTSeDXMkgEua8GNZV8IrumWQ1gLDRCkYFa62wqh44997IrDyABwwvFW1cyM6jni54G3mJ89sKc")
        .send().await
        .map_err(|e: reqwest::Error| AppError::Settings(e.to_string()))?;

    let text = resp.text().await.map_err(|e| AppError::Settings(e.to_string()))?;

    let mut results = Vec::new();
    let mut result_ids = Vec::new();
    let mut search_idx = 0;
    while let Some(pos) = text[search_idx..].find("max-width:225px") {
        let abs = search_idx + pos;
        if let Some(img_pos) = text[abs..].find("id=\"dimg_") {
            let id_start = abs + img_pos + 4;
            if let Some(id_end) = text[id_start..].find('"') {
                let img_id = text[id_start..id_start + id_end].to_string();
                if !result_ids.contains(&img_id) { result_ids.push(img_id); }
            }
        }
        search_idx = abs + 15;
    }

    let marker = "(function(){var s='data:image/";
    let mut id_to_image = std::collections::HashMap::new();
    search_idx = 0;
    while let Some(pos) = text[search_idx..].find(marker) {
        let data_start = search_idx + pos + "(function(){var s='".len();
        if let Some(end) = text[data_start..].find("';") {
            let img_data = text[data_start..data_start + end].to_string()
                .replace("\\x3d", "=").replace("\\x26", "&").replace("\\x3f", "?")
                .replace("\\u003d", "=").replace("\\u0026", "&").replace("\\/", "/");
            if img_data.len() > 500 {
                let after = &text[data_start + end..];
                if let Some(ii_pos) = after.find("var ii=[") {
                    let ii_start = ii_pos + 8;
                    if let Some(ii_end) = after[ii_start..].find(']') {
                        for id in after[ii_start..ii_start + ii_end].split('\'').filter(|s| s.starts_with("dimg_")) {
                            id_to_image.insert(id.to_string(), img_data.clone());
                        }
                    }
                }
            }
            search_idx = data_start + end;
        } else { search_idx += pos + marker.len(); }
    }

    for rid in result_ids {
        if let Some(img_data) = id_to_image.get(&rid) {
            if !results.contains(img_data) { results.push(img_data.clone()); }
        }
        if results.len() >= 30 { break; }
    }

    Ok(results)
}

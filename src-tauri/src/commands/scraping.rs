//! Scraping commands: probe station, scrape radio URL.

use tracing::info;

use crate::error::AppError;
use crate::state::AppState;

#[derive(serde::Serialize)]
pub struct ProbeResult {
    codec: String,
    bitrate: u32,
}

#[tauri::command]
pub async fn probe_station(url: String) -> Result<ProbeResult, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Settings(e.to_string()))?;

    // HLS stream: parse manifest for bitrate/codec info
    if url.contains(".m3u8") {
        let resp = client
            .get(&url)
            .header("User-Agent", "Radiko/1.0")
            .send()
            .await
            .map_err(|e| AppError::Settings(e.to_string()))?;

        let manifest = resp
            .text()
            .await
            .map_err(|e| AppError::Settings(e.to_string()))?;

        let mut bitrate: u32 = 0;
        let mut codec = String::new();

        for line in manifest.lines() {
            if line.starts_with("#EXT-X-STREAM-INF") {
                if let Some(bw_start) = line.find("BANDWIDTH=") {
                    let after = &line[bw_start + 10..];
                    let bw_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(bw) = bw_str.parse::<u32>() {
                        bitrate = bw / 1000;
                    }
                }
                if let Some(codecs_start) = line.find("CODECS=\"") {
                    let after = &line[codecs_start + 8..];
                    if let Some(end) = after.find('"') {
                        let codecs_str = &after[..end];
                        codec = if codecs_str.contains("mp4a") {
                            "AAC".to_string()
                        } else if codecs_str.contains("mp3") || codecs_str.contains("mp4a.40.34") {
                            "MP3".to_string()
                        } else if codecs_str.contains("opus") {
                            "OPUS".to_string()
                        } else {
                            codecs_str.to_string()
                        };
                    }
                }
            }
        }

        // If no STREAM-INF found (media playlist), try to estimate from a segment
        if bitrate == 0 {
            let base_url = {
                let mut u = url.clone();
                if let Some(pos) = u.rfind('/') {
                    u.truncate(pos + 1);
                }
                u
            };
            let mut seg_duration: f64 = 0.0;
            let mut seg_url: Option<String> = None;

            for line in manifest.lines() {
                if line.starts_with("#EXTINF:") {
                    let dur_str: String = line
                        .trim_start_matches("#EXTINF:")
                        .chars()
                        .take_while(|c| *c != ',')
                        .collect();
                    seg_duration = dur_str.parse().unwrap_or(0.0);
                } else if !line.starts_with('#') && !line.is_empty() && seg_duration > 0.0 {
                    seg_url = Some(if line.starts_with("http") {
                        line.to_string()
                    } else {
                        format!("{}{}", base_url, line)
                    });
                    break;
                }
            }

            if let (Some(seg), dur) = (seg_url, seg_duration) {
                if dur > 0.0 {
                    if let Ok(resp) = client
                        .get(&seg)
                        .header("User-Agent", "Radiko/1.0")
                        .send()
                        .await
                    {
                        if let Ok(bytes) = resp.bytes().await {
                            bitrate = ((bytes.len() as f64 * 8.0) / dur / 1000.0) as u32;
                        }
                    }
                }
            }
        }

        if codec.is_empty() {
            codec = "AAC".to_string(); // HLS default
        }

        return Ok(ProbeResult { codec, bitrate });
    }

    // Standard ICY stream probe
    let req = client
        .get(&url)
        .header("Icy-MetaData", "1")
        .send()
        .await
        .map_err(|e| AppError::Settings(e.to_string()))?;

    let content_type = req
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let icy_br = req
        .headers()
        .get("icy-br")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let codec = if content_type.contains("mpeg") || content_type.contains("mp3") {
        "MP3".to_string()
    } else if content_type.contains("aacp") || content_type.contains("aac+") {
        "AAC+".to_string()
    } else if content_type.contains("aac") {
        "AAC".to_string()
    } else if content_type.contains("ogg") {
        "OGG".to_string()
    } else if content_type.contains("flac") {
        "FLAC".to_string()
    } else if content_type.contains("opus") {
        "OPUS".to_string()
    } else {
        String::new()
    };

    let bitrate: u32 = icy_br.parse().unwrap_or(0);

    Ok(ProbeResult { codec, bitrate })
}

#[derive(serde::Serialize)]
pub struct ScrapeResult {
    pub name: String,
    pub stream_urls: Vec<String>,
    pub favicon: String,
    pub page_url: String,
}

#[tauri::command]
pub async fn scrape_radio_url(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<ScrapeResult, AppError> {
    scrape_radio_url_internal(state.proxy_port, url).await
}

pub async fn scrape_radio_url_internal(
    proxy_port: u16,
    url: String,
) -> Result<ScrapeResult, AppError> {
    info!("Scraping radio URL via proxy: {}", url);
    let proxy_url = format!(
        "http://127.0.0.1:{}/proxy?url={}",
        proxy_port,
        urlencoding::encode(&url)
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Settings(e.to_string()))?;

    // Derive base URL for resolving relative paths
    let base_url = {
        if let Some(idx) = url.find("://") {
            let after = &url[idx + 3..];
            if let Some(slash) = after.find('/') {
                url[..idx + 3 + slash].to_string()
            } else {
                url.clone()
            }
        } else {
            url.clone()
        }
    };

    let resp = client
        .get(&proxy_url)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9,tr;q=0.8")
        .send()
        .await
        .map_err(|e| AppError::Settings(format!("Page could not be loaded: {}", e)))?;

    // Check if the URL itself is a direct stream
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if content_type.contains("audio/")
        || content_type.contains("application/ogg")
        || content_type.contains("mpegurl")
        || content_type.contains("x-scpls")
    {
        return Ok(ScrapeResult {
            name: String::new(),
            stream_urls: vec![url.clone()],
            favicon: String::new(),
            page_url: url,
        });
    }

    let html = resp
        .text()
        .await
        .map_err(|e| AppError::Settings(format!("Page could not be read: {}", e)))?;

    let html_lower = html.to_lowercase();
    let mut stream_urls: Vec<String> = Vec::new();
    let mut name = String::new();
    let mut favicon = String::new();

    // Helper: resolve relative URL
    let resolve_url = |u: &str| -> String {
        if u.starts_with("http://") || u.starts_with("https://") {
            u.to_string()
        } else if u.starts_with("//") {
            format!("https:{}", u)
        } else if u.starts_with('/') {
            format!("{}{}", base_url, u)
        } else {
            format!("{}/{}", base_url, u)
        }
    };

    // --- Extract stream URLs ---
    let audio_exts = [
        ".mp3", ".m3u8", ".ogg", ".opus", ".pls", ".m3u", ".flac", ".wav", ".m4a",
    ];
    let mut candidate_urls: Vec<String> = Vec::new();

    // 1. Collect potential candidates (src attributes)
    for tag in ["src=\"", "src='"] {
        let mut idx = 0;
        while let Some(pos) = html_lower[idx..].find(tag) {
            let abs = idx + pos + tag.len();
            let quote = if tag.ends_with('"') { '"' } else { '\'' };
            if let Some(end) = html[abs..].find(quote) {
                let src = html[abs..abs + end].trim();
                let resolved = resolve_url(src);
                if resolved.starts_with("http") && !candidate_urls.contains(&resolved) {
                    candidate_urls.push(resolved);
                }
            }
            idx = abs + 1;
            if candidate_urls.len() > 30 {
                break;
            }
        }
    }

    // 2. Scan for URLs in JS/JSON content
    for ext in &audio_exts {
        let mut idx = 0;
        while let Some(pos) = html_lower[idx..].find(ext) {
            let end_abs = idx + pos + ext.len();
            let start_guess = idx + pos.saturating_sub(250);
            let segment = &html[start_guess..end_abs];

            if let Some(quote_idx) = segment.rfind('"').or_else(|| segment.rfind('\'')) {
                let url_start = start_guess + quote_idx + 1;
                candidate_urls.push(html[url_start..end_abs].to_string());
            } else if let Some(href_idx) = segment.rfind("href=") {
                let url_start = start_guess + href_idx + 5;
                candidate_urls.push(html[url_start..end_abs].to_string());
            }

            idx = end_abs;
            if idx >= html_lower.len() {
                break;
            }
        }
    }

    // 3. PARALLEL TECHNICAL PROBE: Check candidates for real audio/stream signatures
    let probe_client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15")
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    let mut probe_tasks = Vec::new();
    for c_url in &candidate_urls {
        let l = c_url.to_lowercase();
        if l.ends_with(".js")
            || l.ends_with(".css")
            || l.ends_with(".png")
            || l.ends_with(".jpg")
            || l.contains("google")
        {
            continue;
        }

        let client = probe_client.clone();
        let c_url = c_url.clone();
        probe_tasks.push(tauri::async_runtime::spawn(async move {
            if let Ok(res) = client.head(&c_url).send().await {
                let h = res.headers();
                let ct = h
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                let is_audio_ct = ct.starts_with("audio/")
                    || ct.contains("mpegurl")
                    || ct.contains("x-scpls")
                    || ct.contains("application/ogg")
                    || ct.contains("audio/mpeg")
                    || ct.contains("audio/aac");

                let has_icy = h.contains_key("icy-name")
                    || h.contains_key("icy-metaint")
                    || h.contains_key("icy-br")
                    || h.contains_key("x-audiocast-name");

                let is_garbage = ct.starts_with("image/")
                    || ct.starts_with("video/")
                    || ct.starts_with("font/")
                    || ct.contains("javascript")
                    || ct.contains("json")
                    || ct.contains("xml")
                    || ct.contains("pdf");

                if (is_audio_ct || has_icy) && !is_garbage {
                    return Some(c_url);
                }
            }
            None
        }));
        if probe_tasks.len() > 15 {
            break;
        }
    }

    // Collect verified streams
    for task in probe_tasks {
        if let Ok(Some(verified_url)) = task.await {
            if !stream_urls.contains(&verified_url) {
                stream_urls.push(verified_url);
            }
        }
    }

    // 4. Deep Iframe Scrape (Recursive Fallback)
    if stream_urls.is_empty() {
        for tag in ["<iframe", "<frame", "<embed"] {
            let mut idx = 0;
            while let Some(pos) = html_lower[idx..].find(tag) {
                let segment = &html_lower[idx + pos..];
                if let Some(src_pos) = segment.find("src=\"").or_else(|| segment.find("src='")) {
                    let start = idx + pos + src_pos + 5;
                    let quote = if segment[src_pos + 4..].starts_with('"') {
                        '"'
                    } else {
                        '\''
                    };
                    if let Some(end) = html[start..].find(quote) {
                        let inner_url = resolve_url(html[start..start + end].trim());
                        if inner_url.starts_with("http") && inner_url != url {
                            if let Ok(sub) =
                                Box::pin(scrape_radio_url_internal(proxy_port, inner_url)).await
                            {
                                for s in sub.stream_urls {
                                    if !stream_urls.contains(&s) {
                                        stream_urls.push(s);
                                    }
                                }
                            }
                        }
                    }
                }
                idx += pos + 10;
                if stream_urls.len() > 3 || idx >= html_lower.len() {
                    break;
                }
            }
        }
    }

    // --- Extract station name ---
    if let Some(pos) = html_lower.find("og:title") {
        if let Some(content_pos) = html_lower[pos..].find("content=\"") {
            let abs = pos + content_pos + 9;
            if let Some(end) = html[abs..].find('"') {
                name = html[abs..abs + end].trim().to_string();
            }
        }
    }
    if name.is_empty() {
        if let Some(pos) = html_lower.find("<title") {
            if let Some(start) = html[pos..].find('>') {
                let abs = pos + start + 1;
                if let Some(end) = html_lower[abs..].find("</title") {
                    name = html[abs..abs + end].trim().to_string();
                }
            }
        }
    }
    if name.is_empty() {
        if let Some(pos) = html_lower.find("<h1") {
            if let Some(start) = html[pos..].find('>') {
                let abs = pos + start + 1;
                if let Some(end) = html_lower[abs..].find("</h1") {
                    let raw = html[abs..abs + end].trim().to_string();
                    let mut clean = String::new();
                    let mut in_tag = false;
                    for c in raw.chars() {
                        if c == '<' {
                            in_tag = true;
                        } else if c == '>' {
                            in_tag = false;
                        } else if !in_tag {
                            clean.push(c);
                        }
                    }
                    name = clean.trim().to_string();
                }
            }
        }
    }

    // --- Extract favicon/logo ---
    if let Some(pos) = html_lower.find("og:image") {
        if let Some(content_pos) = html_lower[pos..].find("content=\"") {
            let abs = pos + content_pos + 9;
            if let Some(end) = html[abs..].find('"') {
                let img = html[abs..abs + end].trim();
                if !img.is_empty() {
                    favicon = resolve_url(img);
                }
            }
        }
    }
    if favicon.is_empty() {
        if let Some(pos) = html_lower.find("apple-touch-icon") {
            if let Some(href_pos) = html_lower[pos..].find("href=\"") {
                let abs = pos + href_pos + 6;
                if let Some(end) = html[abs..].find('"') {
                    let href = html[abs..abs + end].trim();
                    if !href.is_empty() {
                        favicon = resolve_url(href);
                    }
                }
            }
        }
    }
    if favicon.is_empty() {
        if let Some(pos) = html_lower.find("rel=\"icon\"") {
            if let Some(href_pos) = html_lower[pos..].find("href=\"") {
                let abs = pos + href_pos + 6;
                if let Some(end) = html[abs..].find('"') {
                    let href = html[abs..abs + end].trim();
                    if !href.is_empty() {
                        favicon = resolve_url(href);
                    }
                }
            }
        }
    }
    if favicon.is_empty() {
        favicon = format!("{}/favicon.ico", base_url);
    }

    // Decode HTML entities in name
    name = name
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'");

    info!(
        "Scraped: name='{}', streams={}, favicon='{}'",
        name,
        stream_urls.len(),
        favicon
    );

    Ok(ScrapeResult {
        name,
        stream_urls,
        favicon,
        page_url: url,
    })
}

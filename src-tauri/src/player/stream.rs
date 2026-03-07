//! HTTP streaming with ICY metadata extraction.
//!
//! Connects to an Icecast/Shoutcast stream, separates interleaved
//! ICY metadata from audio bytes, and pushes audio data through a channel
//! to the decode thread.

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tracing::{debug, info};

use crate::events;
use crate::player::types::{PlayerMessage, StreamMetadata};

/// Configuration for a single stream connection attempt.
pub struct StreamConfig {
    pub url: String,
    pub original_url: String,
    pub audio_tx: SyncSender<PlayerMessage>,
    pub shutdown: Arc<AtomicBool>,
    pub app_handle: AppHandle,
    pub emit_events: bool,
    pub skip_ads: bool,
}

/// Opens an HTTP connection and streams audio bytes to `audio_tx`.
/// Returns `Ok(())` on clean shutdown, `Err(msg)` on failure.
pub async fn run_stream(config: &StreamConfig) -> Result<(), String> {
    // Detect HLS streams
    if config.url.contains(".m3u8") {
        return run_hls_stream(config).await;
    }

    run_icy_stream(config).await
}

/// HLS (m3u8) live stream: fetch manifest → download segments → feed audio.
async fn run_hls_stream(config: &StreamConfig) -> Result<(), String> {
    let client = Client::new();
    // Use a common browser user-agent to look more 'legit' to some CDNs
    let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36";

    // Resolve master playlist first if needed
    let mut stream_url = config.url.clone();

    // Experimental: If URL contains known ad-session parameters, maybe try stripping them if manifest fails?
    // For now we use it as-is.

    // Derive base URL
    let mut base_url = stream_url.clone();
    if let Some(pos) = base_url.rfind('/') {
        base_url.truncate(pos + 1);
    }

    // Check if this is a master playlist
    let manifest_bytes = client
        .get(&stream_url)
        .header("User-Agent", user_agent)
        .send()
        .await
        .map_err(|e| format!("HLS manifest fetch failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("HLS manifest read failed: {}", e))?;

    let manifest_text = decode_icy_text(&manifest_bytes);

    let lines: Vec<&str> = manifest_text.lines().collect();
    let has_stream_inf = lines.iter().any(|l| l.starts_with("#EXT-X-STREAM-INF"));

    if has_stream_inf {
        // Master playlist: pick the first variant stream URL
        let variant_url = lines
            .iter()
            .find(|l| !l.starts_with('#') && !l.is_empty())
            .ok_or("No variant streams found in HLS master playlist")?;

        stream_url = if variant_url.starts_with("http") {
            variant_url.to_string()
        } else {
            format!("{}{}", base_url, variant_url)
        };

        info!(
            "HLS master playlist detected, using variant: {}",
            stream_url
        );

        // Update session cache so we skip the master playlist/ads next time
        if let Some(state) = config.app_handle.try_state::<crate::state::AppState>() {
            let mut cache = state.hls_session_cache.lock().unwrap();
            cache.insert(config.original_url.clone(), stream_url.clone());
        }

        // Update base URL for the variant
        base_url = stream_url.clone();
        if let Some(pos) = base_url.rfind('/') {
            base_url.truncate(pos + 1);
        }
    }

    let mut seen_segments: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut last_title: Option<String> = None;
    let mut was_ad_only_mode = false;

    loop {
        if config.shutdown.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Fetch the media playlist
        let manifest_bytes = client
            .get(&stream_url)
            .header("User-Agent", user_agent)
            .send()
            .await
            .map_err(|e| format!("HLS manifest fetch failed: {}", e))?
            .bytes()
            .await
            .map_err(|e| format!("HLS manifest read failed: {}", e))?;

        let manifest_text = decode_icy_text(&manifest_bytes);

        let lines: Vec<&str> = manifest_text.lines().collect();
        let mut segments = Vec::new();
        let mut target_duration: f64 = 6.0;

        let mut current_title: Option<String> = None;
        let mut is_ad_segment = false;

        for line in &lines {
            // Match 'adwData' or common ad patterns like '/ad_' in URLs or tags
            let line_low = line.to_lowercase();
            if line_low.contains("adwdata") || line_low.contains("/ad_") {
                is_ad_segment = true;
            }

            if line.starts_with("#EXT-X-TARGETDURATION:") {
                if let Ok(d) = line
                    .trim_start_matches("#EXT-X-TARGETDURATION:")
                    .parse::<f64>()
                {
                    target_duration = d;
                }
            } else if line.starts_with("#EXTINF:") {
                if let Some(comma_pos) = line.find(',') {
                    let title = line[comma_pos + 1..].trim();
                    if !title.is_empty() {
                        current_title = Some(title.to_string());
                    }
                }
            } else if !line.starts_with('#') && !line.is_empty() {
                let seg_url = if line.starts_with("http") {
                    line.to_string()
                } else {
                    format!("{}{}", base_url, line)
                };
                
                // Final check: adwData or /ad_ in segment URL itself
                let url_low = seg_url.to_lowercase();
                if url_low.contains("adwdata") || url_low.contains("/ad_") {
                    is_ad_segment = true;
                }

                segments.push((seg_url, current_title.take(), is_ad_segment));
                is_ad_segment = false;
            }
        }

        let is_live = !lines.iter().any(|l| l.contains("#EXT-X-ENDLIST"));

        // Check if we have at least one real (non-ad) segment in this manifest
        let ad_segments: Vec<_> = segments.iter().filter(|(_, _, is_ad)| *is_ad).collect();
        let real_segments: Vec<_> = segments.iter().filter(|(_, _, is_ad)| !*is_ad).collect();
        let has_real_content = !real_segments.is_empty();

        if !segments.is_empty() {
            info!(
                "HLS manifest: {} real segments, {} ad segments found. Mode: {}", 
                real_segments.len(), 
                ad_segments.len(),
                if has_real_content { "RADIO (skipping ads)" } else { "AD-ONLY (playing ads)" }
            );
        }

        // IMPROVEMENT: If we were playing ads and now we found radio content,
        // we should favor the new radio content as much as possible.
        if has_real_content && was_ad_only_mode {
            info!("HLS Transition: RADIO content found after AD phase. Clearing seen_segments and FLUSHING buffer.");
            // We clear seen segments here to make sure we catch the very first radio segment
            // even if it appeared in a previous manifest during the wait.
            seen_segments.clear(); 
            let _ = config.audio_tx.send(PlayerMessage::Flush);
        }
        was_ad_only_mode = !has_real_content;

        // Download new segments
        let mut audio_sent_any = false;
        let mut downloaded_any = false;

        // Iterate through segments: skip ads only if we have real material to switch to.
        for (seg_url, seg_title, is_ad) in segments {
            if config.shutdown.load(Ordering::Relaxed) {
                return Ok(());
            }

            // Skip ads if enabled and real content is available in this manifest
            if config.skip_ads && is_ad && has_real_content {
                if !seen_segments.contains(&seg_url) {
                    info!("HLS: skipping ad segment (real content found): {}", seg_url);
                    seen_segments.insert(seg_url.clone());
                }
                continue;
            }

            if seen_segments.contains(&seg_url) {
                continue;
            }
            seen_segments.insert(seg_url.clone());
            downloaded_any = true;

            // Memory limit for seen segments (prevent leak)
            if seen_segments.len() > 1000 {
                seen_segments.clear();
            }

            // Emit metadata if changed
            if let Some(title) = seg_title {
                if last_title.as_deref() != Some(title.as_str()) {
                    debug!("HLS title changed: {}", title);
                    last_title = Some(title.clone());
                    if config.emit_events {
                        events::emit_metadata(
                            &config.app_handle,
                            StreamMetadata {
                                title: Some(title),
                                icy_name: None,
                                icy_genre: None,
                                icy_url: None,
                                icy_br: None,
                                icy_listeners: None,
                            },
                        );
                    }
                }
            }

            match client
                .get(&seg_url)
                .header("User-Agent", user_agent)
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        // Buffer the entire segment so we can detect & demux MPEG-TS
                        match resp.bytes().await {
                            Ok(seg_data) => {
                                if config.shutdown.load(Ordering::Relaxed) {
                                    return Ok(());
                                }
                                let audio = extract_audio_from_segment(&seg_data);
                                if !audio.is_empty() {
                                    if config.audio_tx.send(PlayerMessage::Audio(Bytes::from(audio))).is_err() {
                                        return Ok(());
                                    }
                                    audio_sent_any = true;
                                }
                            }
                            Err(e) => {
                                debug!("HLS segment read error: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("HLS segment download failed: {}", e);
                }
            }
        }

        if !is_live {
            // VOD or finished stream
            if !downloaded_any {
                return Err("HLS stream ended".to_string());
            }
            break;
        }

        // Live stream: wait before refetching manifest
        let wait = if audio_sent_any {
            Duration::from_secs_f64(target_duration * 0.8)
        } else {
            // No real audio was sent (all segments were ads or previously seen)
            // Retry very quickly during the initial "ad-only" or "empty" phase
            Duration::from_millis(500)
        };
        tokio::time::sleep(wait).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// MPEG-TS → audio elementary stream extraction
// ---------------------------------------------------------------------------

const TS_PACKET_SIZE: usize = 188;
const TS_SYNC_BYTE: u8 = 0x47;

/// Detect format and extract audio data from an HLS segment.
/// If the segment is MPEG-TS, extracts the AAC elementary stream (PES payload).
/// Otherwise returns the raw bytes (already ADTS or other format).
fn extract_audio_from_segment(data: &[u8]) -> Vec<u8> {
    if data.len() >= TS_PACKET_SIZE && data[0] == TS_SYNC_BYTE {
        // MPEG-TS container detected
        extract_audio_from_ts(data)
    } else {
        // Already raw audio (ADTS, MP3, etc.) — pass through
        data.to_vec()
    }
}

/// Extract audio PES payload from MPEG-TS packets.
///
/// Strategy:
/// 1. Parse PAT to find PMT PID
/// 2. Parse PMT to find audio elementary stream PID (AAC = stream_type 0x0F or 0x11)
/// 3. Collect PES payloads from audio PID packets
fn extract_audio_from_ts(data: &[u8]) -> Vec<u8> {
    let packets: Vec<&[u8]> = data
        .chunks(TS_PACKET_SIZE)
        .filter(|p| p.len() == TS_PACKET_SIZE && p[0] == TS_SYNC_BYTE)
        .collect();

    if packets.is_empty() {
        return data.to_vec();
    }

    // Step 1: Find PMT PID from PAT (PID 0)
    let mut pmt_pid: Option<u16> = None;
    for pkt in &packets {
        let pid = ts_pid(pkt);
        if pid == 0 {
            pmt_pid = parse_pat(pkt);
            if pmt_pid.is_some() {
                break;
            }
        }
    }

    let pmt_pid = match pmt_pid {
        Some(p) => p,
        None => {
            // No PAT found — can't demux, try returning raw data
            debug!("TS: no PAT found, passing raw segment");
            return data.to_vec();
        }
    };

    // Step 2: Find audio PID from PMT
    let mut audio_pid: Option<u16> = None;
    for pkt in &packets {
        if ts_pid(pkt) == pmt_pid {
            audio_pid = parse_pmt_for_audio(pkt);
            if audio_pid.is_some() {
                break;
            }
        }
    }

    let audio_pid = match audio_pid {
        Some(p) => p,
        None => {
            debug!("TS: no audio stream in PMT, passing raw segment");
            return data.to_vec();
        }
    };

    debug!("TS demux: PMT PID={}, audio PID={}", pmt_pid, audio_pid);

    // Step 3: Collect audio PES payloads
    let mut audio_data = Vec::with_capacity(data.len());
    for pkt in &packets {
        if ts_pid(pkt) != audio_pid {
            continue;
        }
        if let Some(payload) = ts_payload(pkt) {
            let pusi = (pkt[1] & 0x40) != 0;
            if pusi {
                // Payload Unit Start — PES header present
                if let Some(es_data) = strip_pes_header(payload) {
                    audio_data.extend_from_slice(es_data);
                }
            } else {
                // Continuation packet — raw ES data
                audio_data.extend_from_slice(payload);
            }
        }
    }

    if audio_data.is_empty() {
        debug!("TS: extracted 0 audio bytes, passing raw segment");
        data.to_vec()
    } else {
        debug!(
            "TS: extracted {} audio bytes from {} TS packets",
            audio_data.len(),
            packets.len()
        );
        audio_data
    }
}

/// Extract 13-bit PID from a TS packet header.
#[inline]
fn ts_pid(pkt: &[u8]) -> u16 {
    ((pkt[1] as u16 & 0x1F) << 8) | pkt[2] as u16
}

/// Get the payload slice of a TS packet, accounting for the adaptation field.
fn ts_payload(pkt: &[u8]) -> Option<&[u8]> {
    let afc = (pkt[3] >> 4) & 0x03;
    match afc {
        0b01 => {
            // Payload only
            Some(&pkt[4..])
        }
        0b11 => {
            // Adaptation field + payload
            let af_len = pkt[4] as usize;
            let start = 5 + af_len;
            if start < TS_PACKET_SIZE {
                Some(&pkt[start..])
            } else {
                None
            }
        }
        _ => None, // No payload or adaptation only
    }
}

/// Parse PAT (PID 0) to find the first PMT PID.
fn parse_pat(pkt: &[u8]) -> Option<u16> {
    let payload = ts_payload(pkt)?;
    let pusi = (pkt[1] & 0x40) != 0;

    let section = if pusi {
        let pointer = payload[0] as usize;
        if 1 + pointer >= payload.len() {
            return None;
        }
        &payload[1 + pointer..]
    } else {
        payload
    };

    // PAT section: table_id(1) + flags(2) + tsid(2) + version(1) + section(1) + last_section(1) = 8 bytes header
    if section.len() < 12 {
        return None;
    }
    // table_id should be 0x00 for PAT
    if section[0] != 0x00 {
        return None;
    }

    let section_length = ((section[1] as usize & 0x0F) << 8) | section[2] as usize;
    let entries_end = std::cmp::min(3 + section_length.saturating_sub(4), section.len());
    let entries_start = 8; // after fixed header

    if entries_start >= entries_end {
        return None;
    }

    // Each entry: program_number(2) + reserved(3 bits) + PID(13 bits) = 4 bytes
    let entries = &section[entries_start..entries_end];
    for chunk in entries.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let program_number = (chunk[0] as u16) << 8 | chunk[1] as u16;
        if program_number != 0 {
            // Non-NIT entry — this is a PMT PID
            let pid = ((chunk[2] as u16 & 0x1F) << 8) | chunk[3] as u16;
            return Some(pid);
        }
    }

    None
}

/// Parse PMT to find the first audio elementary stream PID.
/// Looks for stream_type 0x0F (AAC ADTS), 0x11 (AAC LATM), or 0x03/0x04 (MPEG audio).
fn parse_pmt_for_audio(pkt: &[u8]) -> Option<u16> {
    let payload = ts_payload(pkt)?;
    let pusi = (pkt[1] & 0x40) != 0;

    let section = if pusi {
        let pointer = payload[0] as usize;
        if 1 + pointer >= payload.len() {
            return None;
        }
        &payload[1 + pointer..]
    } else {
        payload
    };

    // PMT: table_id(1) + section_length(2) + program_number(2) + version(1) + section_num(1) + last_section(1) + PCR_PID(2) + program_info_length(2) = 12 bytes
    if section.len() < 12 {
        return None;
    }
    // table_id should be 0x02 for PMT
    if section[0] != 0x02 {
        return None;
    }

    let section_length = ((section[1] as usize & 0x0F) << 8) | section[2] as usize;
    let program_info_length = ((section[10] as usize & 0x0F) << 8) | section[11] as usize;

    let streams_start = 12 + program_info_length;
    let streams_end = std::cmp::min(3 + section_length.saturating_sub(4), section.len());

    if streams_start >= streams_end {
        return None;
    }

    let mut pos = streams_start;
    while pos + 5 <= streams_end {
        let stream_type = section[pos];
        let es_pid = ((section[pos + 1] as u16 & 0x1F) << 8) | section[pos + 2] as u16;
        let es_info_length = ((section[pos + 3] as usize & 0x0F) << 8) | section[pos + 4] as usize;

        // Audio stream types:
        // 0x03 = MPEG-1 Audio (MP3)
        // 0x04 = MPEG-2 Audio
        // 0x0F = AAC ADTS
        // 0x11 = AAC LATM
        // 0x81 = AC-3 (Dolby)
        if matches!(stream_type, 0x03 | 0x04 | 0x0F | 0x11 | 0x81) {
            return Some(es_pid);
        }

        pos += 5 + es_info_length;
    }

    None
}

/// Strip the PES header from a PES packet payload, returning the elementary stream data.
fn strip_pes_header(data: &[u8]) -> Option<&[u8]> {
    // PES start code: 0x00 0x00 0x01
    if data.len() < 9 || data[0] != 0x00 || data[1] != 0x00 || data[2] != 0x01 {
        // Not a PES start — treat as continuation data
        return Some(data);
    }

    // PES header: start_code(3) + stream_id(1) + PES_length(2) + flags(2) + header_data_length(1)
    let header_data_length = data[8] as usize;
    let es_start = 9 + header_data_length;

    if es_start <= data.len() {
        Some(&data[es_start..])
    } else {
        None
    }
}

/// Traditional ICY/Icecast/Shoutcast stream.
async fn run_icy_stream(config: &StreamConfig) -> Result<(), String> {
    let client = Client::new();

    info!("Connecting to stream: {}", config.url);

    let response = client
        .get(&config.url)
        .header("Icy-MetaData", "1")
        .header("User-Agent", "Radiko/1.0")
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    // Parse icy-metaint (number of audio bytes between metadata blocks)
    let metaint: Option<usize> = response
        .headers()
        .get("icy-metaint")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());

    let get_header = |key: &str| -> Option<String> {
        // Many older SHOUTcast/Icecast servers send latin-1 headers, which might fail standard to_str()
        // We do a robust read here:
        response
            .headers()
            .get(key)
            .map(|v| {
                let bytes = v.as_bytes();
                decode_icy_text(bytes).trim().to_string()
            })
            .filter(|s| !s.is_empty())
    };

    let icy_name = get_header("icy-name");
    let icy_genre = get_header("icy-genre");
    let icy_url = get_header("icy-url");
    let icy_br = get_header("icy-br");
    let icy_listeners = get_header("icy-listeners").or_else(|| get_header("ice-listeners"));

    info!("Connected. ICY metaint: {:?}", metaint);

    // Initial emit (if no title is present yet, at least push the station capabilities)
    if config.emit_events {
        events::emit_metadata(
            &config.app_handle,
            StreamMetadata {
                title: None,
                icy_name: icy_name.clone(),
                icy_genre: icy_genre.clone(),
                icy_url: icy_url.clone(),
                icy_br: icy_br.clone(),
                icy_listeners: icy_listeners.clone(),
            },
        );
    }

    let mut byte_stream = response.bytes_stream();
    let mut icy_parser = IcyParser::new(metaint, config.audio_tx.clone(), config.skip_ads);

    while !config.shutdown.load(Ordering::Relaxed) {
        match byte_stream.next().await {
            Some(Ok(chunk)) => {
                let new_title = icy_parser.process(&chunk)?;

                if let Some(title) = new_title {
                    if config.emit_events {
                        events::emit_metadata(
                            &config.app_handle,
                            StreamMetadata {
                                title: Some(title),
                                icy_name: icy_name.clone(),
                                icy_genre: icy_genre.clone(),
                                icy_url: icy_url.clone(),
                                icy_br: icy_br.clone(),
                                icy_listeners: icy_listeners.clone(),
                            },
                        );
                    }
                }
            }
            Some(Err(e)) => {
                return Err(format!("Stream read error: {}", e));
            }
            None => {
                return Err("Stream ended".to_string());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ICY metadata parser
// ---------------------------------------------------------------------------

/// Parses the interleaved ICY metadata protocol.
///
/// Icecast/Shoutcast streams embed metadata every `metaint` audio bytes:
///   [audio bytes (metaint)] [1 byte: meta_len * 16] [metadata string] [repeat]
struct IcyParser {
    metaint: Option<usize>,
    bytes_until_meta: usize,
    meta_buf: Vec<u8>,
    meta_remaining: usize,
    in_metadata: bool,
    last_title: Option<String>,
    skip_audio: bool,
    skip_ads_config: bool,
    audio_tx: SyncSender<PlayerMessage>,
}

impl IcyParser {
    fn new(metaint: Option<usize>, audio_tx: SyncSender<PlayerMessage>, skip_ads_config: bool) -> Self {
        Self {
            metaint,
            bytes_until_meta: metaint.unwrap_or(0),
            meta_buf: Vec::with_capacity(4096),
            meta_remaining: 0,
            in_metadata: false,
            last_title: None,
            skip_audio: false,
            skip_ads_config,
            audio_tx,
        }
    }

    /// Process a raw chunk from the HTTP response.
    /// Returns optional_new_title.
    fn process(&mut self, chunk: &[u8]) -> Result<Option<String>, String> {
        let metaint = match self.metaint {
            Some(m) => m,
            None => {
                // No ICY metadata — entire chunk is audio
                if !self.skip_audio && !chunk.is_empty() {
                    if self.audio_tx.send(PlayerMessage::Audio(Bytes::copy_from_slice(chunk))).is_err() {
                        return Err("audio_tx send failed".into());
                    }
                }
                return Ok(None);
            }
        };

        let mut new_title = None;
        let mut pos = 0;

        while pos < chunk.len() {
            if self.in_metadata {
                let to_read = std::cmp::min(self.meta_remaining, chunk.len() - pos);
                self.meta_buf.extend_from_slice(&chunk[pos..pos + to_read]);
                self.meta_remaining -= to_read;
                pos += to_read;

                if self.meta_remaining == 0 {
                    self.in_metadata = false;
                    self.bytes_until_meta = metaint;

                    let metadata_text = decode_icy_text(&self.meta_buf);
                    self.skip_audio = self.skip_ads_config && metadata_text.contains("adwData");
                    
                    if let Some(title) = parse_icy_title(&self.meta_buf) {
                        if self.skip_audio {
                            info!("ICY: adwData detected in metadata, skipping audio: {}", title);
                        } else if self.last_title.as_deref() != Some(&title) {
                            debug!("ICY title changed: {}", title);
                            self.last_title = Some(title.clone());
                            new_title = Some(title);
                        }
                    } else if self.skip_audio {
                        info!("ICY: adwData detected in raw metadata string, skipping audio");
                    }
                    self.meta_buf.clear();
                }
            } else if self.bytes_until_meta == 0 {
                // Read the metadata length byte
                let meta_length = chunk[pos] as usize * 16;
                pos += 1;

                if meta_length == 0 {
                    self.bytes_until_meta = metaint;
                } else {
                    self.in_metadata = true;
                    self.meta_remaining = meta_length;
                    self.meta_buf.clear();
                }
            } else {
                // Read audio bytes
                let to_read = std::cmp::min(self.bytes_until_meta, chunk.len() - pos);
                if !self.skip_audio {
                    let audio_chunk = Bytes::copy_from_slice(&chunk[pos..pos + to_read]);
                    if !audio_chunk.is_empty() {
                        if self.audio_tx.send(PlayerMessage::Audio(audio_chunk)).is_err() {
                            return Err("audio_tx send failed".into());
                        }
                    }
                }
                self.bytes_until_meta -= to_read;
                pos += to_read;
            }
        }

        Ok(new_title)
    }
}

/// Extract `StreamTitle` from ICY metadata string.
/// Format: `StreamTitle='Artist - Song';StreamUrl='...';`
///
/// ICY metadata can be UTF-8, ISO-8859-9 (Turkish Latin-5), or Windows-1254.
/// We try UTF-8 first, then fall back to Windows-1254 which covers Turkish chars.
fn parse_icy_title(metadata: &[u8]) -> Option<String> {
    let text = decode_icy_text(metadata);
    let text = text.trim_end_matches('\0');

    for part in text.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("StreamTitle='") {
            if let Some(title) = value.strip_suffix('\'') {
                let title = title.trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
        }
    }

    None
}

/// Decode ICY metadata bytes to a String.
/// Tries UTF-8 first; if invalid bytes are found, falls back to Windows-1254
/// (superset of ISO-8859-9, commonly used by Turkish radio stations).
fn decode_icy_text(bytes: &[u8]) -> String {
    // Try standard UTF-8 first
    if let Ok(s) = std::str::from_utf8(bytes) {
        // Detect globally "Double-Encoded UTF-8" algorithmically:
        // By checking if ALL chars map to ISO-8859-1 (u32 <= 255), we know it
        // was misread by an older native ISO stream server. If repacking them
        // creates VALID multi-byte UTF-8, it's irrefutably double-encoded!
        let is_all_latin1 = s.chars().all(|c| (c as u32) <= 255);
        if is_all_latin1 {
            let packed: Vec<u8> = s.chars().map(|c| c as u8).collect();
            if let Ok(fixed) = std::str::from_utf8(&packed) {
                // If the repack produced non-ASCII characters, it was actually UTF-8!
                if !fixed.is_ascii() {
                    return fixed.to_string();
                }
            }
        }
        return s.to_string();
    }

    // String is totally invalid UTF-8. Use Mozilla's chardetng library to intelligently
    // analyze the byte-frequencies and guess the native charset (Turkish, Russian, etc.)
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true); // true = this is the last chunk

    // Pass None for TLD, and true for allowing non-utf8 (it's already proven not utf8)
    let encoding = detector.guess(None, true);
    let (decoded, _, _) = encoding.decode(bytes);

    decoded.into_owned()
}

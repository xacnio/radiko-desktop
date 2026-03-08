use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Instant;

use crate::player::types::{PlaybackStatus, StreamMetadata};
use crate::player::PlayerHandle;
use crate::services::discord_rpc::DiscordRpc;

pub struct AppState {
    pub inner: Arc<Mutex<PlayerState>>,
    pub proxy_port: u16,
    /// Async mutex to serialize play() calls and prevent race conditions
    pub play_lock: tokio::sync::Mutex<()>,
    /// Maps original master playlist URL -> last known valid variant URL (with session ID)
    pub hls_session_cache: Arc<Mutex<HashMap<String, String>>>,
    /// Last time device change restart was triggered (for debouncing)
    pub last_device_restart: Arc<Mutex<Option<Instant>>>,
    /// Discord Rich Presence client
    pub discord_rpc: Arc<DiscordRpc>,
}

pub struct PlayerState {
    pub status: PlaybackStatus,
    pub current_url: Option<String>,
    pub station_name: Option<String>,
    pub station_image: Option<String>, // Local file:/// URL for display
    pub default_cover: Option<String>,
    pub enriched_cover: Option<String>, // iTunes/enriched cover for Discord
    pub enriched_album: Option<String>, // Album name from iTunes
    pub volume: f32,
    pub stream_metadata: Option<StreamMetadata>,
    pub handle: Option<PlayerHandle>,
    pub preview_handle: Option<PlayerHandle>,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub output_device: Option<String>,
    pub skip_ads: bool,
}

impl PlayerState {
    pub fn new(volume: f32, last_url: Option<String>, minimize_to_tray: bool, close_to_tray: bool, output_device: Option<String>, skip_ads: bool) -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            current_url: last_url,
            station_name: None,
            station_image: None,
            default_cover: None,
            enriched_cover: None,
            enriched_album: None,
            volume,
            stream_metadata: None,
            handle: None,
            preview_handle: None,
            minimize_to_tray,
            close_to_tray,
            output_device,
            skip_ads,
        }
    }
}

impl AppState {
    pub fn new(volume: f32, last_url: Option<String>, minimize_to_tray: bool, close_to_tray: bool, output_device: Option<String>, skip_ads: bool, discord_enabled: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PlayerState::new(volume, last_url, minimize_to_tray, close_to_tray, output_device, skip_ads))),
            proxy_port: 0,
            play_lock: tokio::sync::Mutex::new(()),
            hls_session_cache: Arc::new(Mutex::new(HashMap::new())),
            last_device_restart: Arc::new(Mutex::new(None)),
            discord_rpc: Arc::new(DiscordRpc::new(discord_enabled)),
        }
    }
}

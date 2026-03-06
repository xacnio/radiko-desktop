use std::sync::{Arc, Mutex};

use crate::player::types::{PlaybackStatus, StreamMetadata};
use crate::player::PlayerHandle;

pub struct AppState {
    pub inner: Arc<Mutex<PlayerState>>,
    pub proxy_port: u16,
    /// Async mutex to serialize play() calls and prevent race conditions
    pub play_lock: tokio::sync::Mutex<()>,
}

pub struct PlayerState {
    pub status: PlaybackStatus,
    pub current_url: Option<String>,
    pub station_name: Option<String>,
    pub station_image: Option<String>,
    pub default_cover: Option<String>,
    pub volume: f32,
    pub stream_metadata: Option<StreamMetadata>,
    pub handle: Option<PlayerHandle>,
    pub preview_handle: Option<PlayerHandle>,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
}

impl PlayerState {
    pub fn new(volume: f32, last_url: Option<String>, minimize_to_tray: bool, close_to_tray: bool) -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            current_url: last_url,
            station_name: None,
            station_image: None,
            default_cover: None,
            volume,
            stream_metadata: None,
            handle: None,
            preview_handle: None,
            minimize_to_tray,
            close_to_tray,
        }
    }
}

impl AppState {
    pub fn new(volume: f32, last_url: Option<String>, minimize_to_tray: bool, close_to_tray: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PlayerState::new(volume, last_url, minimize_to_tray, close_to_tray))),
            proxy_port: 0,
            play_lock: tokio::sync::Mutex::new(()),
        }
    }
}

use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    Connecting,
    Reconnecting,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    pub title: Option<String>,
    pub icy_name: Option<String>,
    pub icy_genre: Option<String>,
    pub icy_url: Option<String>,
    pub icy_br: Option<String>,
    pub icy_listeners: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: PlaybackStatus,
    pub url: Option<String>,
    pub volume: f32,
    pub metadata: Option<StreamMetadata>,
    pub station_name: Option<String>,
    pub station_image: Option<String>,
}

/// Messages sent from stream task to decode thread
pub enum PlayerMessage {
    Audio(Bytes),
    Flush,
}

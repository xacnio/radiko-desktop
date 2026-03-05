use serde::Serialize;

/// All error variants that can cross the Tauri command boundary.
/// Serialized as a plain string for the frontend.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Audio output error: {0}")]
    AudioOutput(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Settings error: {0}")]
    Settings(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

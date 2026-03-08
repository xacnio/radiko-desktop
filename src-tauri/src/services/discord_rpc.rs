//! Discord Rich Presence integration for showing currently playing radio station.

use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde_json;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

const DISCORD_APP_ID: &str = "1480236926794465362"; // TODO: Replace with actual Discord App ID

pub struct DiscordRpc {
    client: Arc<Mutex<Option<DiscordIpcClient>>>,
    enabled: Arc<Mutex<bool>>,
}

impl DiscordRpc {
    pub fn new(enabled: bool) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            enabled: Arc::new(Mutex::new(enabled)),
        }
    }

    /// Connect to Discord RPC
    pub fn connect(&self) -> Result<(), String> {
        let enabled = *self.enabled.lock().unwrap();
        if !enabled {
            return Ok(());
        }

        let mut client_guard = self.client.lock().unwrap();
        
        // Disconnect existing client if any
        if let Some(mut old_client) = client_guard.take() {
            let _ = old_client.close();
        }

        // Create new client
        let mut client = DiscordIpcClient::new(DISCORD_APP_ID)
            .map_err(|e| format!("Failed to create Discord client: {}", e))?;

        client
            .connect()
            .map_err(|e| format!("Failed to connect to Discord: {}", e))?;

        *client_guard = Some(client);
        info!("Connected to Discord RPC");
        Ok(())
    }

    /// Disconnect from Discord RPC
    pub fn disconnect(&self) {
        let mut client_guard = self.client.lock().unwrap();
        if let Some(mut client) = client_guard.take() {
            // Clear presence before disconnecting
            let _ = client.clear_activity();
            let _ = client.close();
            info!("Disconnected from Discord RPC");
        }
    }

    /// Update presence with currently playing station
    pub fn update_presence(
        &self,
        station_name: &str,
        metadata: Option<&str>,
        enriched_cover: Option<&str>,
        album_name: Option<&str>,
    ) {
        let enabled = *self.enabled.lock().unwrap();
        if !enabled {
            return;
        }

        let mut client_guard = self.client.lock().unwrap();
        
        // Try to reconnect if not connected
        if client_guard.is_none() {
            drop(client_guard);
            if let Err(e) = self.connect() {
                warn!("Failed to reconnect to Discord: {}", e);
                return;
            }
            client_guard = self.client.lock().unwrap();
        }

        if let Some(client) = client_guard.as_mut() {
            let start_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            let details = if let Some(meta) = metadata {
                meta.to_string()
            } else {
                "Listening to radio".to_string()
            };

            let state_text = station_name.to_string();
            let version = env!("CARGO_PKG_VERSION");
            let small_text = format!("Radiko Desktop v{}", version);

            // Build activity JSON manually to include buttons
            let mut activity_json = serde_json::json!({
                "details": details,
                "state": state_text,
                "type": 2, // Listening
                "timestamps": {
                    "start": start_timestamp
                },
                "buttons": [
                    {
                        "label": "View on GitHub",
                        "url": "https://github.com/xacnio/radiko-desktop"
                    }
                ]
            });

            // Add images based on what's available
            if let Some(cover_url) = enriched_cover {
                info!("Discord RPC: Using enriched cover: {}", cover_url);
                let large_text = album_name.unwrap_or("Album Cover");
                activity_json["assets"] = serde_json::json!({
                    "large_image": cover_url,
                    "large_text": large_text,
                    "small_image": "https://raw.githubusercontent.com/xacnio/radiko-desktop/refs/heads/master/src-tauri/icons/icon.png",
                    "small_text": small_text
                });
            } else {
                info!("Discord RPC: No enriched cover, using default logo");
                activity_json["assets"] = serde_json::json!({
                    "large_image": "radiko_logo",
                    "large_text": "Radiko Desktop"
                });
            }

            // Send raw SET_ACTIVITY command
            let payload = serde_json::json!({
                "cmd": "SET_ACTIVITY",
                "args": {
                    "pid": std::process::id(),
                    "activity": activity_json
                },
                "nonce": format!("{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())
            });

            if let Err(e) = client.send(payload, 1) {
                error!("Failed to update Discord presence: {}", e);
                // Disconnect on error
                *client_guard = None;
            }
        }
    }

    /// Clear presence (when stopped)
    pub fn clear_presence(&self) {
        let enabled = *self.enabled.lock().unwrap();
        if !enabled {
            return;
        }

        info!("Discord RPC: Clearing presence");

        let mut client_guard = self.client.lock().unwrap();
        if let Some(client) = client_guard.as_mut() {
            match client.clear_activity() {
                Ok(_) => {
                    info!("Discord presence cleared successfully");
                }
                Err(e) => {
                    error!("Failed to clear Discord presence: {}", e);
                    // Disconnect and reconnect to ensure clean state
                    *client_guard = None;
                }
            }
        }
    }

    /// Enable or disable Discord RPC
    pub fn set_enabled(&self, enabled: bool) {
        let mut enabled_guard = self.enabled.lock().unwrap();
        let was_enabled = *enabled_guard;
        *enabled_guard = enabled;
        drop(enabled_guard);

        if enabled {
            let _ = self.connect();
        } else {
            // Clear presence before disconnecting
            if was_enabled {
                let mut client_guard = self.client.lock().unwrap();
                if let Some(client) = client_guard.as_mut() {
                    let _ = client.clear_activity();
                }
            }
            self.disconnect();
        }
    }

    /// Check if Discord RPC is enabled
    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock().unwrap()
    }
}

impl Drop for DiscordRpc {
    fn drop(&mut self) {
        self.disconnect();
    }
}

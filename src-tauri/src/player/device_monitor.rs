//! Audio device monitoring for automatic output switching.
//!
//! On Windows, monitors the default audio output device and notifies
//! when it changes so playback can be restarted on the new device.

use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::time::Duration;
use tauri::AppHandle;
#[cfg(target_os = "windows")]
use tauri::Emitter;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Monitors the default audio output device and emits events when it changes.
pub struct DeviceMonitor {
    current_device: Arc<RwLock<Option<String>>>,
    app_handle: AppHandle,
    last_change: Arc<RwLock<Option<std::time::Instant>>>,
}

impl DeviceMonitor {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            current_device: Arc::new(RwLock::new(None)),
            app_handle,
            last_change: Arc::new(RwLock::new(None)),
        }
    }

    /// Start monitoring the default audio device (Windows only).
    /// Emits "audio-device-changed" event when the default device changes.
    pub async fn start(self: Arc<Self>) {
        #[cfg(target_os = "windows")]
        {
            info!("Starting audio device monitor");
            
            // Initialize with current default device
            if let Some(device_name) = Self::get_default_device_name() {
                *self.current_device.write().await = Some(device_name.clone());
                info!("Initial default audio device: {}", device_name);
            }

            // Poll for changes every 2 seconds
            let monitor = Arc::clone(&self);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    
                    if let Some(new_device) = Self::get_default_device_name() {
                        let mut current = monitor.current_device.write().await;
                        
                        // Check if device changed
                        let changed = match &*current {
                            Some(old) => old != &new_device,
                            None => true,
                        };
                        
                        if changed {
                            // Debounce: Only emit if at least 3 seconds passed since last change
                            let mut last_change = monitor.last_change.write().await;
                            let now = std::time::Instant::now();
                            let should_emit = match *last_change {
                                Some(last) => now.duration_since(last) > Duration::from_secs(3),
                                None => true,
                            };
                            
                            if should_emit {
                                info!("Default audio device changed: {} -> {}", 
                                    current.as_ref().unwrap_or(&"None".to_string()), 
                                    new_device);
                                
                                *current = Some(new_device.clone());
                                *last_change = Some(now);
                                
                                // Emit event to frontend
                                let _ = monitor.app_handle.emit("audio-device-changed", new_device);
                            } else {
                                // Still update current device but don't emit
                                *current = Some(new_device.clone());
                            }
                        }
                    }
                }
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            info!("Audio device monitoring is only supported on Windows");
        }
    }

    /// Get the name of the current default audio output device.
    fn get_default_device_name() -> Option<String> {
        use rodio::cpal::traits::{DeviceTrait, HostTrait};
        
        let host = rodio::cpal::default_host();
        
        match host.default_output_device() {
            Some(device) => {
                match device.name() {
                    Ok(name) => Some(name),
                    Err(e) => {
                        warn!("Failed to get device name: {}", e);
                        None
                    }
                }
            }
            None => {
                warn!("No default output device found");
                None
            }
        }
    }
}

//! Player orchestrator — manages the streaming pipeline lifecycle.
//!
//! Responsibilities:
//! - Creating and tearing down the streaming + decode pipeline
//! - Reconnection with exponential backoff
//! - Exposing pause/resume/volume controls via the shared Sink

pub mod decoder;
#[cfg(target_os = "windows")]
pub mod device_monitor;
pub mod eq;
pub mod stream;
pub mod types;

use rodio::{OutputStream, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::error::AppError;
use crate::events;
use types::PlaybackStatus;

const CHANNEL_BOUND: usize = 8;
const MAX_RETRIES: u32 = 10;
const MAX_BACKOFF_SECS: u64 = 30;

/// Runtime resources for an active playback session.
/// Dropping this struct tears down the entire pipeline.
pub struct PlayerHandle {
    pub sink: Arc<Sink>,
    pub shutdown: Arc<AtomicBool>,
    stream_task: JoinHandle<()>,
    decode_thread: Option<std::thread::JoinHandle<()>>,
}

impl PlayerHandle {
    /// Signal shutdown and wait for all resources to clean up.
    pub fn stop(mut self) {
        info!("Stopping player");
        self.shutdown.store(true, Ordering::Relaxed);
        self.sink.stop();
        self.stream_task.abort();
        if let Some(_handle) = self.decode_thread.take() {
            // Unblocking stop: we don't wait for the thread to join to avoid locking the tokio executor
            // The thread will exit because shutdown is true and stream is aborted.
        }
    }
}

/// Start a new playback session. Returns a PlayerHandle for control.
///
/// Pipeline:
///   [async task: HTTP + ICY] --channel--> [std thread: symphonia + rodio]
pub async fn start(
    play_url: String,
    original_url: String,
    volume: f32,
    app_handle: AppHandle,
    emit_events: bool,
    _output_device: Option<String>,
    skip_ads: bool,
) -> Result<PlayerHandle, AppError> {
    let shutdown = Arc::new(AtomicBool::new(false));

    // Bounded channel: audio bytes/commands from stream task → decode thread
    let (audio_tx, audio_rx) = mpsc::sync_channel::<types::PlayerMessage>(CHANNEL_BOUND);

    // Create audio output + sink on a dedicated thread.
    // OutputStream is !Send — it must live on its creating thread.
    let (sink_tx, sink_rx) = tokio::sync::oneshot::channel::<Result<Arc<Sink>, String>>();

    let shutdown_dec = Arc::clone(&shutdown);
    let app_handle_dec = app_handle.clone();

    let decode_thread = std::thread::Builder::new()
        .name("radio-decode".into())
        .spawn(move || {
            // Create audio output on this thread.
            // OutputStream is !Send — it must stay on this thread for its lifetime.
            let (_output_stream, stream_handle) = {
                #[cfg(not(target_os = "macos"))]
                {
                    use rodio::cpal::traits::{HostTrait, DeviceTrait};
                    let mut selected = None;
                    if let Some(dev_name) = output_device.as_deref() {
                        'outer: for host_id in rodio::cpal::available_hosts() {
                            if let Ok(host) = rodio::cpal::host_from_id(host_id) {
                                if let Ok(devices) = host.output_devices() {
                                    for d in devices {
                                        if let Ok(name) = d.name() {
                                            if name == dev_name {
                                                selected = Some(d);
                                                break 'outer;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if selected.is_none() {
                            tracing::warn!("Device '{}' not found, falling back to default", dev_name);
                        }
                    }
                    match selected {
                        Some(ref device) => rodio::OutputStream::try_from_device(device)
                            .unwrap_or_else(|e| {
                                tracing::error!("Custom audio output error: {}, falling back to default", e);
                                OutputStream::try_default().expect("no audio output device")
                            }),
                        None => match OutputStream::try_default() {
                            Ok(v) => v,
                            Err(e) => { let _ = sink_tx.send(Err(format!("Audio output error: {}", e))); return; }
                        },
                    }
                }

                #[cfg(target_os = "macos")]
                match OutputStream::try_default() {
                    Ok(v) => v,
                    Err(e) => { let _ = sink_tx.send(Err(format!("Audio output error: {}", e))); return; }
                }
            };

            let sink = match Sink::try_new(&stream_handle) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    let _ = sink_tx.send(Err(format!("Sink creation error: {}", e)));
                    return;
                }
            };

            // Send Sink back to caller before entering decode loop
            let _ = sink_tx.send(Ok(Arc::clone(&sink)));

            // Run decode loop (blocks until shutdown or stream ends)
            decoder::run_decode_loop(audio_rx, sink, shutdown_dec, app_handle_dec, emit_events);

            // _output_stream drops here — audio stops
            info!("Decode thread exited");
        })
        .map_err(|e| AppError::AudioOutput(format!("Thread spawn failed: {}", e)))?;

    // Wait for the decode thread to create the Sink (non-blocking)
    let sink = sink_rx
        .await
        .map_err(|_| AppError::AudioOutput("Decode thread exited before creating sink".into()))?
        .map_err(AppError::AudioOutput)?;

    sink.set_volume(volume);

    // Spawn the async stream task (with reconnection)
    let shutdown_stream = Arc::clone(&shutdown);
    let app_handle_stream = app_handle.clone();
    let play_url_clone = play_url.clone();
    let original_url_clone = original_url.clone();

    let stream_task = tokio::spawn(async move {
        run_stream_with_reconnect(
            play_url_clone,
            original_url_clone,
            audio_tx,
            shutdown_stream,
            app_handle_stream,
            emit_events,
            skip_ads,
        )
        .await;
    });

    if emit_events {
        events::emit_status(&app_handle, PlaybackStatus::Connecting);
    }

    Ok(PlayerHandle {
        sink,
        shutdown,
        stream_task,
        decode_thread: Some(decode_thread),
    })
}

/// Stream loop with exponential backoff reconnection.
async fn run_stream_with_reconnect(
    play_url: String,
    original_url: String,
    audio_tx: mpsc::SyncSender<types::PlayerMessage>,
    shutdown: Arc<AtomicBool>,
    app_handle: AppHandle,
    emit_events: bool,
    skip_ads: bool,
) {
    let mut retry_count: u32 = 0;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Get the best URL to play: check cache first, fallback to play_url (original requested)
        let current_stream_url = if let Some(state) = app_handle.try_state::<crate::state::AppState>() {
            let cache = state.hls_session_cache.lock().unwrap();
            cache.get(&original_url).cloned().unwrap_or(play_url.clone())
        } else {
            play_url.clone()
        };

        let config = stream::StreamConfig {
            url: current_stream_url.clone(),
            original_url: original_url.clone(),
            audio_tx: audio_tx.clone(),
            shutdown: Arc::clone(&shutdown),
            app_handle: app_handle.clone(),
            emit_events,
            skip_ads,
        };

        match stream::run_stream(&config).await {
            Ok(()) => {
                // Clean shutdown (user pressed stop)
                info!("Stream closed cleanly");
                break;
            }
            Err(e) => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                // If a cached session URL failed, clear it so next attempt uses the master playlist again
                if let Some(state) = app_handle.try_state::<crate::state::AppState>() {
                    let mut cache = state.hls_session_cache.lock().unwrap();
                    if let Some(cached) = cache.get(&original_url) {
                        if cached == &current_stream_url {
                            info!("HLS session URL failed, clearing cache for {}", original_url);
                            cache.remove(&original_url);
                        }
                    }
                }

                retry_count += 1;
                if retry_count > MAX_RETRIES {
                    error!("Max retries ({}) reached. Giving up.", MAX_RETRIES);
                    if emit_events {
                        events::emit_error(
                            &app_handle,
                            &format!("Connection lost after {} retries: {}", MAX_RETRIES, e),
                        );
                        events::emit_status(&app_handle, PlaybackStatus::Stopped);
                    }
                    break;
                }

                let delay_secs = std::cmp::min(
                    1u64 << (retry_count - 1), // 1, 2, 4, 8, 16, ...
                    MAX_BACKOFF_SECS,
                );

                warn!(
                    "Stream error: {}. Reconnecting in {}s (attempt {}/{})",
                    e, delay_secs, retry_count, MAX_RETRIES
                );

                if emit_events {
                    events::emit_status(&app_handle, PlaybackStatus::Reconnecting);
                }
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;

                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                if emit_events {
                    events::emit_status(&app_handle, PlaybackStatus::Connecting);
                }
            }
        }
    }
}

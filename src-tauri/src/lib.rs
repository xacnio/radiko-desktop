mod commands;
mod error;
mod events;
mod platform;
mod player;
mod services;
mod settings;
mod state;
use services::proxy;
mod setup;

use settings::Settings;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tracing::info!("Radiko starting");

    // 1. PENDING RESET CHECK
    setup::check_pending_reset();

    // 2. NATIVE SPLASH SCREEN (WINDOWS)
    let splash_handle = platform::splash::SplashScreen::show();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();
            }
        }))
        .setup(move |app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));

            let settings = Settings::load(&data_dir);
            tracing::info!(
                "Loaded settings: volume={}, last_url={:?}",
                settings.volume,
                settings.last_url
            );

            // 3. STATE INITIALIZATION & PROXY START
            let proxy_state = proxy::start_proxy(app.handle().clone());
            let port = proxy_state.port;

            let mut state = AppState::new(
                settings.volume,
                settings.last_url,
                settings.minimize_to_tray,
                settings.close_to_tray,
                settings.output_device,
                settings.skip_ads,
                settings.discord_rpc
            );
            state.proxy_port = port;
            app.manage(state);

            // 4. DEFAULT COVER IMAGE GENERATION
            setup::generate_default_cover(app);

            // 5. HTML SPLASH SCREEN (MACOS / LINUX)
            #[cfg(not(target_os = "windows"))]
            setup::setup_html_splash(app, settings.theme.as_deref());

            // 6. OS MEDIA CONTROLS & EVENT LISTENERS
            setup::setup_os_media_controls(app);

            // 7. AWAIT FRONTEND & CLOSE SPLASH
            setup::await_frontend_and_close_splash(app.handle().clone(), splash_handle);

            // 8. TRAY ICON
            if let Err(e) = setup::setup_tray(app) {
                tracing::error!("Failed to initialize tray: {}", e);
            }

            // 11. OFF-SCREEN FIX (Windows only)
            // Win+D and multi-monitor changes can leave the window off-screen.
            // Listen for focus events and re-center if needed.
            #[cfg(target_os = "windows")]
            if let Some(main_win) = app.get_webview_window("main") {
                let win = main_win.clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(true) = event {
                        if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                            let monitors = win.available_monitors().unwrap_or_default();
                            let on_screen = monitors.iter().any(|m| {
                                let mp = m.position();
                                let ms = m.size();
                                pos.x < mp.x + ms.width as i32
                                    && pos.x + size.width as i32 > mp.x
                                    && pos.y < mp.y + ms.height as i32
                                    && pos.y + size.height as i32 > mp.y
                            });
                            if !on_screen {
                                tracing::warn!("Window focused but off-screen, centering");
                                let _ = win.center();
                            }
                        }
                    }
                });
            }

            // 9. AUDIO DEVICE MONITOR (Windows only)
            #[cfg(target_os = "windows")]
            {
                let monitor = std::sync::Arc::new(player::device_monitor::DeviceMonitor::new(app.handle().clone()));
                let monitor_clone = std::sync::Arc::clone(&monitor);
                tauri::async_runtime::spawn(async move {
                    monitor_clone.start().await;
                });
            }

            // 10. DISCORD RPC INITIALIZATION
            if let Some(app_state) = app.try_state::<AppState>() {
                if app_state.discord_rpc.is_enabled() {
                    let _ = app_state.discord_rpc.connect();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::play,
            commands::preview_play,
            commands::preview_stop,
            commands::stop,
            commands::pause,
            commands::resume,
            commands::set_volume,
            commands::get_status,
            commands::re_enrich,
            commands::get_audio_level,
            commands::get_eq_gains,
            commands::set_eq_gains,
            commands::get_eq_enabled,
            commands::set_eq_enabled,
            commands::search_stations,
            commands::get_top_stations,
            commands::get_countries,
            commands::get_languages,
            commands::get_tags,
            commands::get_states,
            commands::get_all_country_stations,
            commands::batch_cache_favicons,
            commands::get_custom_stations,
            commands::save_custom_station,
            commands::save_custom_stations_batch,
            commands::delete_custom_station,
            commands::clear_missing_favicon,
            commands::toggle_favorite,
            commands::update_station_indices,
            commands::open_browser_url,
            commands::open_link_window,
            commands::open_link_view_in_browser,
            commands::update_link_view_width,
            commands::set_link_view_interaction,
            commands::close_link_view,
            commands::upload_custom_favicon,
            commands::download_custom_favicon,
            commands::probe_station,
            commands::search_images_internal,
            commands::reset_setup,
            commands::scrape_radio_url,
            commands::open_radio_browser,
            commands::browser_back,
            commands::browser_forward,
            commands::browser_reload,
            commands::browser_stop,
            commands::browser_navigate,
            commands::browser_get_url,
            commands::send_radio_detect,
            commands::close_browser_window,
            commands::fetch_live_listeners,
            commands::get_os,
            commands::minimize_browser_window,
            commands::maximize_browser_window,
            commands::drag_window,
            commands::start_window_resize,
            commands::send_radio_detect_sidebar,
            commands::probe_and_add_stream_from_js,
            commands::get_proxy_url,
            commands::identify_song,
            commands::get_identified_songs,
            commands::save_identified_song,
            commands::clear_identified_songs,
            commands::delete_identified_song,
            commands::get_settings,
            commands::save_sort_order,
            commands::save_language,
            commands::save_theme,
            commands::save_tray_settings,
            commands::save_skip_ads,
            commands::get_audio_devices,
            commands::set_audio_device,
            commands::restart_on_device_change,
            commands::set_discord_rpc,
            commands::get_discord_rpc_status,
            commands::export_backup,
            commands::import_backup,
            commands::analyze_backup,
        ])
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .run(tauri::generate_context!())
        .expect("error while running Radiko");
}

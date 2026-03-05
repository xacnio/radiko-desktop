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

            let mut state = AppState::new(settings.volume, settings.last_url);
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
            commands::drag_browser_window,
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
            commands::export_backup,
            commands::import_backup,
            commands::analyze_backup,
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .run(tauri::generate_context!())
        .expect("error while running Radiko");
}

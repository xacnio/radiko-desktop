use tauri::{App, Manager};

use crate::platform;
use crate::services::media::MediaSession;
use crate::state::AppState;

/// Checks for the '.pending_reset' flag and cleans up WebView/cache directories if found.
pub fn check_pending_reset() {
    let app_data = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .ok()
            .map(|d| std::path::PathBuf::from(d).join("dev.xacnio.radikodesktop"))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME").ok().map(|d| {
            std::path::PathBuf::from(d).join("Library/Application Support/dev.xacnio.radikodesktop")
        })
    } else {
        std::env::var("HOME")
            .ok()
            .map(|d| std::path::PathBuf::from(d).join(".local/share/dev.xacnio.radikodesktop"))
    };

    if let Some(data_dir) = app_data {
        let flag = data_dir.join(".pending_reset");
        if flag.exists() {
            tracing::info!("Pending reset detected, cleaning WebView data...");
            let _ = std::fs::remove_file(&flag);

            // Delete EBWebView (Windows WebView2 data)
            let webview_dir = data_dir.join("EBWebView");
            if webview_dir.exists() {
                tracing::info!("Deleting WebView data: {:?}", webview_dir);
                let _ = std::fs::remove_dir_all(&webview_dir);
            }

            // Delete cache directory
            let cache_dir = if cfg!(target_os = "windows") {
                std::env::var("LOCALAPPDATA")
                    .ok()
                    .map(|d| std::path::PathBuf::from(d).join("dev.xacnio.radikodesktop"))
            } else if cfg!(target_os = "macos") {
                std::env::var("HOME").ok().map(|d| {
                    std::path::PathBuf::from(d).join("Library/Caches/dev.xacnio.radikodesktop")
                })
            } else {
                std::env::var("HOME")
                    .ok()
                    .map(|d| std::path::PathBuf::from(d).join(".cache/dev.xacnio.radikodesktop"))
            };

            if let Some(cache) = cache_dir {
                if cache.exists() {
                    tracing::info!("Deleting cache: {:?}", cache);
                    let _ = std::fs::remove_dir_all(&cache);
                }
            }
        }
    }
}

/// Creates a default cover image ("default_cover_v2.png") for stations without a favicon.
pub fn generate_default_cover(app: &App) {
    if let Ok(cache_dir) = app.path().app_cache_dir() {
        let _ = std::fs::create_dir_all(&cache_dir);
        let default_cover_path = cache_dir.join("default_cover_v2.png");

        if !default_cover_path.exists() {
            // Generate a beautiful vinyl record aesthetic cover
            let img = image::RgbaImage::from_fn(256, 256, |x, y| {
                let cx = (x as f32 - 128.0) / 128.0;
                let cy = (y as f32 - 128.0) / 128.0;
                let d = (cx * cx + cy * cy).sqrt();

                if d > 0.95 {
                    // Background corner gradient
                    let bg = (15.0 * (1.5 - d).max(0.0)) as u8;
                    image::Rgba([bg, bg, bg, 255])
                } else if d > 0.35 {
                    // Vinyl grooves with slight light reflection
                    let angle = cy.atan2(cx);
                    let reflection = (angle * 2.0).sin().powi(4) * 15.0; // Shiny diagonal light
                    let groove = (d * 180.0).sin() * 6.0;
                    let base = (22.0 + groove + reflection).clamp(0.0, 255.0) as u8;
                    image::Rgba([base, base, base, 255])
                } else if d > 0.06 {
                    // Center label (Dynamic Accent Green)
                    let green_shade = (185.0 - (d * 80.0)).clamp(0.0, 255.0) as u8;
                    image::Rgba([29, green_shade, 84, 255])
                } else {
                    // Center hole
                    image::Rgba([10, 10, 10, 255])
                }
            });
            let _ = img.save(&default_cover_path);
        }

        let path_str = default_cover_path.to_string_lossy().replace('\\', "/");
        let cover_str = if path_str.starts_with('/') {
            format!("file://{}", path_str)
        } else {
            format!("file:///{}", path_str)
        };
        if let Some(app_state) = app.try_state::<AppState>() {
            if let Ok(mut ps) = app_state.inner.lock() {
                ps.default_cover = Some(cover_str);
            }
        }
    }
}

/// On macOS/Linux, creates a static HTML splash window (Tauri-managed).
#[cfg(not(target_os = "windows"))]
pub fn setup_html_splash(app: &mut App, theme: Option<&str>) {
    let is_light = match theme {
        Some("light") => true,
        Some("dark") => false,
        _ => dark_light::detect() == dark_light::Mode::Light,
    };

    let bg_color = if is_light {
        (245, 245, 245, 255)
    } else {
        (13, 13, 13, 255)
    };

    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "splash",
        tauri::WebviewUrl::App("splash.html".into()),
    )
    .title("Radiko Desktop")
    .inner_size(340.0, 148.0)
    .center()
    .decorations(false)
    .background_color(bg_color.into())
    .always_on_top(true)
    .resizable(false)
    .build();
}

/// Initialises OS media transport controls (Windows SMTC or macOS Now Playing) and event listeners.
pub fn setup_os_media_controls(app: &App) {
    let window = app.get_webview_window("main");
    #[allow(unused_variables)]
    if let Some(window) = window.clone() {
        #[cfg(target_os = "windows")]
        {
            // Set AppUserModelID so Windows SMTC shows "Radiko" instead of "Unknown app"
            extern "system" {
                fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
            }
            let app_id: Vec<u16> = "dev.xacnio.radikodesktop\0".encode_utf16().collect();
            unsafe {
                SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
            }

            // Create Start Menu shortcut with AppUserModelID
            // so that SMTC can resolve the app name from the shortcut
            platform::shortcut::ensure_start_menu_shortcut(
                "dev.xacnio.radikodesktop",
                "Radiko Desktop",
            );

            if let Ok(hwnd) = window.hwnd() {
                let hwnd_ptr = hwnd.0;
                if let Some(session) = MediaSession::new(hwnd_ptr, app.handle().clone()) {
                    app.manage(session);
                }
                // Add thumbnail toolbar buttons (prev/play-pause/next)
                platform::thumbbar::setup_thumb_buttons(hwnd.0 as isize, app.handle().clone());
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(session) = MediaSession::new(std::ptr::null_mut(), app.handle().clone()) {
                app.manage(session);
            }
        }

        let win_clone = window.clone();
        
        // macOS: Use native NSWindowWillMiniaturizeNotification instead of polling
        #[cfg(target_os = "macos")]
        {
            let app_handle = app.handle().clone();
            // Small delay to ensure the window is fully initialized
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                crate::platform::macos::register_miniaturize_observer(&app_handle);
                crate::platform::macos::register_default_device_listener(app_handle);
                tracing::info!("macOS observers registered");
            });
        }
        
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::Resized(_) => {
                #[cfg(not(target_os = "macos"))]
                {
                    let win = win_clone.clone();
                    let is_min = win.is_minimized().unwrap_or(false);
                    
                    if is_min {
                        let state = win.state::<crate::state::AppState>();
                        let minimize_to_tray = state.inner.lock().unwrap().minimize_to_tray;
                        
                        if minimize_to_tray {
                            tracing::info!("Window minimized, hiding to tray");
                            let _ = win.hide();
                        }
                    }
                }
                crate::commands::internal_layout_link_view(&win_clone);
            }
            tauri::WindowEvent::Focused(focused) => {
                // When window gains focus, restore Regular policy on macOS
                // so it appears in Dock and Cmd+Tab
                #[cfg(target_os = "macos")]
                if *focused && win_clone.is_visible().unwrap_or(false) {
                    tracing::info!("Window focused, setting Regular policy");
                    let _ = win_clone.app_handle().set_activation_policy(tauri::ActivationPolicy::Regular);
                }
            }
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                crate::commands::internal_layout_link_view(&win_clone);
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let state = win_clone.state::<crate::state::AppState>();
                let close_to_tray = state.inner.lock().unwrap().close_to_tray;

                if close_to_tray {
                    tracing::info!("DEBUG: Close requested, hiding to tray");
                    #[cfg(target_os = "macos")]
                    let _ = win_clone.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                    let _ = win_clone.hide();
                    api.prevent_close();
                } else {
                    let _ = win_clone.hide();
                    let handle = win_clone.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        for win in handle.webview_windows().values() {
                            let _ = win.destroy();
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                        handle.exit(0);
                    });
                    api.prevent_close();
                }
            }
            _ => {}
        });
    }
}

/// Polls until the frontend window appears, then hides all splash screens.
pub fn await_frontend_and_close_splash(
    app_handle: tauri::AppHandle,
    splash_handle: Option<platform::splash::SplashScreen>,
) {
    let poll_win = app_handle.get_webview_window("main");
    if poll_win.is_none() {
        if let Some(s) = splash_handle {
            s.close();
        }
        return;
    }
    let poll_win = poll_win.unwrap();

    std::thread::spawn(move || {
        let show_and_close_splash = |handle: &tauri::AppHandle| {
            // Show main window and grab focus FIRST
            let _ = poll_win.show();
            let _ = poll_win.set_focus();
            // Small delay to let the window actually appear before closing splash
            std::thread::sleep(std::time::Duration::from_millis(50));
            // THEN close splashes
            if let Some(s) = splash_handle {
                s.close();
            }
            if let Some(sw) = handle.get_webview_window("splash") {
                let _ = sw.close();
            }
        };

        // Wait for frontend to become visible or timeout after 10s
        for i in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if poll_win.is_visible().unwrap_or(false) {
                tracing::info!("Frontend ready after {}ms", i * 100);
                show_and_close_splash(&app_handle);
                return;
            }
        }
        tracing::warn!("Frontend didn't show window within 10s, forcing show");
        show_and_close_splash(&app_handle);
    });
}

use include_dir::{include_dir, Dir};
static LOCALES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../src/locales");

fn get_tray_text(lang: &str, key: &str) -> String {
    let file_name = format!("{}.json", lang);
    let file = LOCALES_DIR.get_file(&file_name)
        .or_else(|| LOCALES_DIR.get_file("en.json"));
    
    if let Ok(json_str) = file.and_then(|f| f.contents_utf8()).ok_or(()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(text) = v["tray"][key].as_str() {
                return text.to_string();
            }
        }
    }
    key.to_string()
}

fn update_tray_menu(app: &tauri::AppHandle, lang: &str) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem, Submenu, PredefinedMenuItem};
    use crate::player::types::PlaybackStatus;

    let state = app.state::<crate::state::AppState>();
    let (status, station_name, song_title) = {
        let inner = state.inner.lock().unwrap();
        let s_name = inner.station_name.clone().unwrap_or_else(|| "Radiko".to_string());
        let s_title = inner.stream_metadata.as_ref().and_then(|m| m.title.clone());
        (inner.status.clone(), s_name, s_title)
    };

    let info_text = if let Some(t) = &song_title {
        if t.trim().is_empty() {
            format!("📻 {}", station_name)
        } else {
            let mut display = format!("{} - {}", station_name, t);
            if display.chars().count() > 36 {
                display = display.chars().take(33).collect::<String>() + "...";
            }
            format!("📻 {}", display)
        }
    } else if station_name != "Radiko" && !station_name.trim().is_empty() {
        format!("📻 {}", station_name)
    } else {
        "Radiko Desktop".to_string()
    };

    let info_i = MenuItem::with_id(app, "info", info_text, false, None::<&str>)?;
    
    let status_text = match status {
        PlaybackStatus::Playing | PlaybackStatus::Connecting => format!("✨ {}", get_tray_text(lang, "playing")),
        PlaybackStatus::Paused => format!("⏸ {}", get_tray_text(lang, "paused")),
        _ => format!("⏹ {}", get_tray_text(lang, "stopped")),
    };
    let status_i = MenuItem::with_id(app, "status", status_text, false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    let play_label = match status {
        PlaybackStatus::Playing | PlaybackStatus::Connecting => format!("⏸ {}", get_tray_text(lang, "pause")),
        _ => format!("▶ {}", get_tray_text(lang, "play")),
    };
    let play_pause_i = MenuItem::with_id(app, "play_pause", play_label, true, None::<&str>)?;
    
    let next_i = MenuItem::with_id(app, "next", format!("⏭ {}", get_tray_text(lang, "next")), true, None::<&str>)?;
    let prev_i = MenuItem::with_id(app, "prev", format!("⏮ {}", get_tray_text(lang, "prev")), true, None::<&str>)?;
    
    let vol_m = Submenu::with_id(app, "vol_menu", format!("🔊 {}", get_tray_text(lang, "volume")), true)?;
    let v0 = MenuItem::with_id(app, "vol_0", get_tray_text(lang, "mute"), true, None::<&str>)?;
    let v20 = MenuItem::with_id(app, "vol_20", "20%", true, None::<&str>)?;
    let v50 = MenuItem::with_id(app, "vol_50", "50%", true, None::<&str>)?;
    let v80 = MenuItem::with_id(app, "vol_80", "80%", true, None::<&str>)?;
    let v100 = MenuItem::with_id(app, "vol_100", "100%", true, None::<&str>)?;
    let _ = vol_m.append_items(&[&v0, &v20, &v50, &v80, &v100])?;
    
    let sep2 = PredefinedMenuItem::separator(app)?;

    let quit_i = MenuItem::with_id(app, "quit", get_tray_text(lang, "quit"), true, None::<&str>)?;
    let show_i = MenuItem::with_id(app, "show", get_tray_text(lang, "mainWindow"), true, None::<&str>)?;
    let mini_i = MenuItem::with_id(app, "mini", get_tray_text(lang, "miniPlayer"), true, None::<&str>)?;
    
    let menu = Menu::with_items(app, &[
        &info_i, &status_i, &sep1, 
        &play_pause_i, &next_i, &prev_i, 
        &vol_m, &sep2, 
        &mini_i, &show_i, &quit_i
    ])?;

    if let Some(tray) = app.tray_by_id("main_tray") {
        let _ = tray.set_menu(Some(menu));
    }
    
    Ok(())
}

pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{Emitter, Manager};
    #[cfg(not(target_os = "linux"))]
    use tauri_plugin_positioner::{WindowExt, Position};

    let dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let settings = crate::settings::Settings::load(&dir);
    let initial_lang = settings.language.unwrap_or_else(|| "en".to_string());
    let lang_ref = std::sync::Arc::new(std::sync::Mutex::new(initial_lang.clone()));

    // Pre-create the tray window
    #[allow(unused_mut)]
    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        "tray",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Radiko Mini Player")
    .inner_size(320.0, 88.0)
    .decorations(false)
    .always_on_top(true)
    .resizable(false)
    .visible(false);

    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.transparent(true).skip_taskbar(true);
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, transparent() requires macos-private-api feature or tauri.conf.json configuration.
        // skip_taskbar(true) is also unsupported on macOS via WebviewWindowBuilder.
    }

    let _tray_win = builder.build()?;

    // Clone for event handler (only used on Linux/macOS)
    #[cfg(not(target_os = "windows"))]
    let tray_win_clone = _tray_win.clone();

    // Hide tray window when it loses focus (Linux/macOS)
    #[cfg(not(target_os = "windows"))]
    _tray_win.on_window_event(move |event| match event {
        tauri::WindowEvent::Focused(focused) => {
            if !*focused {
                #[cfg(target_os = "linux")]
                {
                    tracing::info!("Tray Window: Lost focus, waiting before hiding...");
                    let w = tray_win_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        // Avoid hiding if it quickly regained focus
                        if !w.is_focused().unwrap_or(false) && w.is_visible().unwrap_or(false) {
                            tracing::info!("Tray Window: Still unfocused, hiding.");
                            let _ = w.hide();
                        }
                    });
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = tray_win_clone.hide();
                }
            } else {
                #[cfg(target_os = "linux")]
                tracing::info!("Tray Window: Gained focus");
            }
        }
        _ => {}
    });

    let _tray = TrayIconBuilder::with_id("main_tray")
        .icon(app.default_window_icon().unwrap().clone())
        .show_menu_on_left_click(false)
        .tooltip("Radiko Desktop")
        .on_menu_event(move |app: &tauri::AppHandle, event| {
            match event.id.as_ref() {
                "mini" => {
                    println!("TRAY: 'mini' menu item clicked");
                    tracing::info!("Tray: 'mini' menu item clicked");
                    if let Some(window) = app.get_webview_window("tray") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            #[cfg(target_os = "linux")]
                            {
                                if let Ok(cursor_pos) = app.cursor_position() {
                                    let size = window.inner_size().unwrap_or(tauri::PhysicalSize { width: 320, height: 88 });
                                    let monitor = app.monitor_from_point(cursor_pos.x, cursor_pos.y).ok().flatten()
                                        .or_else(|| window.primary_monitor().ok().flatten());
                                    
                                    if let Some(m) = monitor {
                                        let m_pos = m.position();
                                        let m_size = m.size();
                                        let rel_x = cursor_pos.x - m_pos.x as f64;
                                        let rel_y = cursor_pos.y - m_pos.y as f64;
                                        let is_top = rel_y < (m_size.height as f64 / 2.0);
                                        let is_right = rel_x > (m_size.width as f64 / 2.0);
                                        
                                        let margin_x = 12;
                                        let margin_y = 36;
                                        
                                        let x = if is_right {
                                            m_pos.x + m_size.width as i32 - size.width as i32 - margin_x
                                        } else {
                                            m_pos.x + margin_x
                                        };
                                        let y = if is_top {
                                            m_pos.y + margin_y
                                        } else {
                                            m_pos.y + (m_size.height as i32) - (size.height as i32) - margin_y
                                        };
                                        
                                        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
                                    }
                                }
                            }
                            #[cfg(not(target_os = "linux"))]
                            let _ = window.move_window(Position::TrayCenter);
                            
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                            
                            // Extra focus-force for Linux to ensure hide-on-blur works
                            #[cfg(target_os = "linux")]
                            {
                                let w = window.clone();
                                tauri::async_runtime::spawn(async move {
                                    for delay in [50, 150, 300, 600] {
                                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                                        if !w.is_visible().unwrap_or(false) { break; }
                                        if w.is_focused().unwrap_or(false) { break; }
                                        
                                        let _ = w.unminimize();
                                        let _ = w.set_focus();
                                        let _ = w.set_always_on_top(false);
                                        let _ = w.set_always_on_top(true);
                                    }
                                });
                            }
                            
                            let _ = window.emit("tray-opened", ());
                        }
                    }
                }
                "quit" => {
                    println!("TRAY: 'quit' menu item clicked");
                    let h = app.clone();
                    
                    tauri::async_runtime::spawn(async move {
                        // Destroy ALL windows (Main, Tray, Browser views etc.)
                        for win in h.webview_windows().values() {
                            let _ = win.destroy();
                        }
                        
                        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                        h.exit(0);
                    });
                }
                "show" => {
                    println!("TRAY: 'show' menu item clicked");
                    if let Some(window) = app.get_webview_window("main") {
                        #[cfg(target_os = "macos")]
                        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "play_pause" => {
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "toggle");
                    });
                }
                "next" => {
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "next");
                    });
                }
                "prev" => {
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = h.emit("media-key", "previous");
                    });
                }
                "vol_0" => { 
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::set_volume(0.0, h.clone(), h.state()).await;
                    });
                }
                "vol_20" => { 
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::set_volume(0.2, h.clone(), h.state()).await;
                    });
                }
                "vol_50" => { 
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::set_volume(0.5, h.clone(), h.state()).await;
                    });
                }
                "vol_80" => { 
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::set_volume(0.8, h.clone(), h.state()).await;
                    });
                }
                "vol_100" => { 
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::set_volume(1.0, h.clone(), h.state()).await;
                    });
                }

                _ => {}
            }
        })
        .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event| {
            #[cfg(target_os = "linux")]
            println!("TRAY: Icon Event: {:?}", event);
            
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            
            match event {
                TrayIconEvent::Click {
                    button,
                    button_state,
                    position,
                    ..
                } if button == MouseButton::Left && (cfg!(target_os = "linux") || button_state == MouseButtonState::Up) => {
                    #[cfg(target_os = "windows")]
                    {
                        let last_hide = crate::platform::mouse_hook::LAST_HIDE_TIME.load(std::sync::atomic::Ordering::SeqCst);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or(std::time::Duration::from_millis(0))
                            .as_millis() as u64;
                        
                        // If the window was hidden less than 200ms ago by the global mouse hook, 
                        // this click event is the same tray icon click finishing its MouseUp phase.
                        if now > 0 && now.saturating_sub(last_hide) < 200 {
                            return;
                        }
                    }

                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("tray") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            #[cfg(target_os = "linux")]
                            {
                                // Screen-aware positioning for Linux tray clicks
                                let size = window.inner_size().unwrap_or(tauri::PhysicalSize { width: 320, height: 88 });
                                
                                // Identify which monitor the click happened on
                                let monitor = app.monitor_from_point(position.x, position.y).ok().flatten()
                                    .or_else(|| window.primary_monitor().ok().flatten());
                                
                                if let Some(m) = monitor {
                                    let m_pos = m.position();
                                    let m_size = m.size();
                                    let rel_x = position.x - m_pos.x as f64;
                                    let rel_y = position.y - m_pos.y as f64;
                                    let is_top = rel_y < (m_size.height as f64 / 2.0);
                                    let is_right = rel_x > (m_size.width as f64 / 2.0);
                                    
                                    let margin_x = 12;
                                    let margin_y = 36;
                                    
                                    let x = if is_right {
                                        m_pos.x + m_size.width as i32 - size.width as i32 - margin_x
                                    } else {
                                        m_pos.x + margin_x
                                    };
                                    let y = if is_top {
                                        m_pos.y + margin_y
                                    } else {
                                        m_pos.y + (m_size.height as i32) - (size.height as i32) - margin_y
                                    };
                                    
                                    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
                                }
                            }
                            #[cfg(not(target_os = "linux"))]
                            let _ = window.move_window(Position::TrayCenter);
                            
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.emit("tray-opened", ());
                            
                            #[cfg(target_os = "windows")]
                            {
                                let hwnd_val = if let Ok(hwnd) = window.hwnd() {
                                    hwnd.0 as isize
                                } else {
                                    0
                                };
                                
                                // 1. Give focus best-effort
                                if hwnd_val != 0 {
                                    tauri::async_runtime::spawn(async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                        extern "system" { fn SetForegroundWindow(hwnd: isize) -> i32; }
                                        unsafe { SetForegroundWindow(hwnd_val); }
                                    });
                                }
                                
                                // 2. The ONLY bulletproof way on Windows for frameless tray apps:
                                // Use a global mouse hook (WH_MOUSE_LL) to detect left/right clicks
                                // that happen outside of our window.
                                let app_handle = window.app_handle().clone();
                                tauri::async_runtime::spawn(async move {
                                    // A small delay to avoid catching the initial tray icon click
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    
                                    // Start the hook logic on a dedicated background thread since
                                    // Windows hooks require a message loop or need to block.
                                    std::thread::spawn(move || {
                                        crate::platform::mouse_hook::start_mouse_hook(app_handle, hwnd_val);
                                    });
                                });
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                let _ = window.set_focus();
                                #[cfg(target_os = "linux")]
                                {
                                    let w = window.clone();
                                    tauri::async_runtime::spawn(async move {
                                        for delay in [50, 150, 300, 600] {
                                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                                            if !w.is_visible().unwrap_or(false) { break; }
                                            if w.is_focused().unwrap_or(false) { break; }
                                            
                                            let _ = w.unminimize();
                                            let _ = w.set_focus();
                                            let _ = w.set_always_on_top(false);
                                            let _ = w.set_always_on_top(true);
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    if let Some(main) = tray.app_handle().get_webview_window("main") {
                        let is_visible = main.is_visible().unwrap_or(false);
                        let is_minimized = main.is_minimized().unwrap_or(false);
                        
                        if is_visible && !is_minimized {
                            let _ = main.hide();
                            #[cfg(target_os = "macos")]
                            let _ = tray.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                        } else {
                            #[cfg(target_os = "macos")]
                            {
                                tracing::info!("Tray double-click: showing window, setting Regular policy");
                                let _ = tray.app_handle().set_activation_policy(tauri::ActivationPolicy::Regular);
                            }
                            if is_minimized {
                                let _ = main.unminimize();
                            }
                            let _ = main.show();
                            let _ = main.set_focus();
                        }
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    // Initialize menu
    let _ = update_tray_menu(app.handle(), &initial_lang);

    // Dynamic tray menu logic
    {
        use tauri::Listener;
        let lang_ref_listen = lang_ref.clone();
        let app_handle = app.handle().clone();
        let app_handle_inner = app_handle.clone();
        app_handle.listen("playback-status", move |_| {
            let current_lang = lang_ref_listen.lock().unwrap().clone();
            let _ = update_tray_menu(&app_handle_inner, &current_lang);
        });

        // Listen for language changes
        let lang_ref_inner = lang_ref.clone();
        let app_handle_lang = app.handle().clone();
        let app_handle_lang_inner = app_handle_lang.clone();
        app_handle_lang.listen("language-changed", move |event: tauri::Event| {
            if let Ok(new_lang) = serde_json::from_str::<String>(event.payload()) {
                let mut lang = lang_ref_inner.lock().unwrap();
                *lang = new_lang.clone();
                let _ = update_tray_menu(&app_handle_lang_inner, &new_lang);
            }
        });

        // Listen for metadata updates
        let lang_ref_meta = lang_ref.clone();
        let app_handle_meta = app.handle().clone();
        let app_handle_meta_inner = app_handle_meta.clone();
        app_handle_meta.listen("stream-metadata", move |_| {
            let current_lang = lang_ref_meta.lock().unwrap().clone();
            let _ = update_tray_menu(&app_handle_meta_inner, &current_lang);
        });
    }

    Ok(())
}

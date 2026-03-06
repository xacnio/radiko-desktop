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

        let cover_str = format!(
            "file:///{}",
            default_cover_path.to_string_lossy().replace('\\', "/")
        );
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
    let window = app.get_window("main");
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
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::Resized(_) => {
                if let Ok(is_min) = win_clone.is_minimized() {
                    if is_min {
                        let state = win_clone.state::<crate::state::AppState>();
                        let minimize_to_tray = state.inner.lock().unwrap().minimize_to_tray;
                        if minimize_to_tray {
                            let _ = win_clone.hide();
                        }
                    }
                }
                crate::commands::internal_layout_link_view(&win_clone);
            }
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                crate::commands::internal_layout_link_view(&win_clone);
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let state = win_clone.state::<crate::state::AppState>();
                let close_to_tray = state.inner.lock().unwrap().close_to_tray;
                if close_to_tray {
                    let _ = win_clone.hide();
                    api.prevent_close();
                } else if win_clone.label() == "main" {
                    win_clone.app_handle().exit(0);
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
    let poll_win = app_handle.get_window("main");
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

        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if poll_win.is_visible().unwrap_or(false) {
                show_and_close_splash(&app_handle);
                return;
            }
        }
        tracing::warn!("Frontend didn't show window within 5s, forcing show");
        show_and_close_splash(&app_handle);
    });
}

pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri_plugin_positioner::{WindowExt, Position};
    use tauri::Emitter;

    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show_i = MenuItem::with_id(app, "show", "Show Main Window", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    // Pre-create the tray window
    let _tray_win = tauri::WebviewWindowBuilder::new(
        app,
        "tray",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Radiko Mini Player")
    .inner_size(320.0, 120.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .build()?;

    let tray_win_clone = _tray_win.clone();

    // Hide tray window when it loses focus
    _tray_win.on_window_event(move |event| match event {
        tauri::WindowEvent::Focused(focused) => {
            tracing::info!("Tray window focused: {}", focused);
            if !focused {
                let _ = tray_win_clone.hide();
            }
        }
        _ => {}
    });

    if let Some(icon) = app.default_window_icon() {
        let _tray = TrayIconBuilder::new()
            .menu(&menu)
            .tooltip("Radiko Desktop")
            .icon(icon.clone())
            .on_menu_event(|app, event| match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                
                match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
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
                                let _ = window.move_window(Position::TrayCenter);
                                let _ = window.show();
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
                                }
                            }
                        }
                    }
                    TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => {
                        if let Some(main) = tray.app_handle().get_webview_window("main") {
                            if main.is_visible().unwrap_or(false) {
                                let _ = main.hide();
                            } else {
                                let _ = main.show();
                                let _ = main.unminimize();
                                let _ = main.set_focus();
                            }
                        }
                    }
                    _ => {}
                }
            })
            .build(app)?;
    }

    Ok(())
}

//! Browser commands: link view, radio browser window, navigation, detection.

use tauri::{Emitter, Manager};
use tracing::info;

use crate::error::AppError;
use crate::state::AppState;

use super::scraping::scrape_radio_url_internal;

static LINK_VIEW_WIDTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(400);

pub fn internal_layout_link_view(w: &tauri::WebviewWindow) {
    use tauri::Manager;
    if let Some(lv) = w.get_webview("link-view") {
        layout_link_view_inner(&lv, w.scale_factor().unwrap_or(1.0), w.inner_size().unwrap_or(tauri::PhysicalSize::new(1200, 800)));
    }
}

fn layout_link_view_inner(lv: &tauri::Webview<impl tauri::Runtime>, sf: f64, size: tauri::PhysicalSize<u32>) {
    let width_px = LINK_VIEW_WIDTH.load(std::sync::atomic::Ordering::Relaxed);
    let resizer_gutter = (7.0_f64 * sf).round() as u32;
    let link_view_w = (width_px as f64 * sf).round() as u32;
    let titlebar_h = (38.0_f64 * sf).round() as u32;
    let header_h = (40.0_f64 * sf).round() as u32;
    let mini_player_h = (100.0_f64 * sf).round() as u32;

    let view_x = size.width.saturating_sub(link_view_w) + resizer_gutter;
    let view_y = titlebar_h + header_h;
    let view_h = size.height.saturating_sub(view_y + mini_player_h);
    let final_w = link_view_w.saturating_sub(resizer_gutter);

    let _ = lv.set_position(tauri::PhysicalPosition::new(view_x, view_y));
    let _ = lv.set_size(tauri::PhysicalSize::new(final_w, view_h));
}

#[tauri::command]
pub fn update_link_view_width(app: tauri::AppHandle, width: u32) {
    use tauri::Manager;
    LINK_VIEW_WIDTH.store(width, std::sync::atomic::Ordering::Relaxed);
    if let Some(lv) = app.get_webview("link-view") {
        let parent = lv.window();
        let sf = parent.scale_factor().unwrap_or(1.0);
        let size = parent.inner_size().unwrap_or(tauri::PhysicalSize::new(1200, 800));
        layout_link_view_inner(&lv, sf, size);
    }
}

#[tauri::command]
pub fn set_link_view_interaction(app: tauri::AppHandle, enabled: bool) {
    use tauri::Manager;
    if let Some(lv) = app.get_webview("link-view") {
        let js = if enabled {
            "document.documentElement.style.pointerEvents = 'auto'"
        } else {
            "document.documentElement.style.pointerEvents = 'none'"
        };
        let _ = lv.eval(js);
    }
}

#[tauri::command]
pub async fn open_link_view_in_browser(app: tauri::AppHandle) -> Result<(), AppError> {
    use tauri::Manager;
    if let Some(lv) = app.get_webview("link-view") {
        if let Ok(url) = lv.url() {
            super::settings_cmd::open_browser_url(url.to_string()).await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn open_link_window(app: tauri::AppHandle, url: String) -> Result<(), AppError> {
    use tauri::{Emitter, Manager, WebviewBuilder, WebviewUrl};

    let parsed_url = tauri::Url::parse(&url).map_err(|e| AppError::Settings(e.to_string()))?;

    let scrollbar_css = r#"
        (function() {
            const css = `
                * {
                    scrollbar-width: none !important;
                }
                ::-webkit-scrollbar {
                    display: none !important;
                    width: 0 !important;
                    height: 0 !important;
                }
            `;
            const style = document.createElement('style');
            style.textContent = css;
            document.documentElement.appendChild(style);
            
            // Force mobile viewport
            let meta = document.querySelector('meta[name="viewport"]');
            if(!meta) {
                meta = document.createElement('meta');
                meta.name = 'viewport';
                document.getElementsByTagName('head')[0].appendChild(meta);
            }
            meta.content = 'width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no';
        })();
    "#;

    let scrollbar_css_clone = scrollbar_css.to_string();

    // If the link view is already open, just navigate it
    if let Some(existing) = app.get_webview("link-view") {
        let _ = existing.set_zoom(0.8);
        let _ = existing.navigate(parsed_url);
        let _ = existing.eval(scrollbar_css);
        let _ = app.emit("link-view-show", url.clone());
        let _ = app.emit("link-view-navigate", url);
        return Ok(());
    }

    // Get the main window to add a child webview
    let main_win = app
        .get_window("main")
        .ok_or_else(|| AppError::Settings("Main window not found".into()))?;

    let sf = main_win.scale_factor().unwrap_or(1.0);
    let size = main_win
        .inner_size()
        .unwrap_or(tauri::PhysicalSize::new(1200, 800));

    let width_px = LINK_VIEW_WIDTH.load(std::sync::atomic::Ordering::Relaxed);
    let resizer_gutter = (6.0_f64 * sf).round() as u32;
    let link_view_w = (width_px as f64 * sf).round() as u32;
    let titlebar_h = (38.0_f64 * sf).round() as u32;
    let header_h = (40.0_f64 * sf).round() as u32;
    let mini_player_h = (100.0_f64 * sf).round() as u32;

    let view_x = size.width.saturating_sub(link_view_w) + resizer_gutter;
    let view_y = titlebar_h + header_h;
    let view_h = size.height.saturating_sub(view_y + mini_player_h);

    let _app_handle = app.clone();
    let autoplay_disabler_js = r#"
        (function() {
            const prevent = (el) => {
                if(!el) return;
                el.autoplay = false;
                el.removeAttribute('autoplay');
                el.setAttribute('preload', 'none');
            };
            document.querySelectorAll('audio, video').forEach(prevent);
            new MutationObserver(m => m.forEach(res => res.addedNodes.forEach(n => {
                if(n.nodeName === 'AUDIO' || n.nodeName === 'VIDEO') prevent(n);
                else if(n.querySelectorAll) n.querySelectorAll('audio, video').forEach(prevent);
            }))).observe(document.documentElement, { childList: true, subtree: true });
        })();
    "#;

    let _link_view = main_win
        .add_child(
            WebviewBuilder::new("link-view", WebviewUrl::External(parsed_url))
                .initialization_script(autoplay_disabler_js)
                .on_navigation(move |_url| {
                    // Allow all navigation, URL update happens on page load
                    true
                })
                .on_page_load(move |webview, payload| {
                    if payload.event() == tauri::webview::PageLoadEvent::Finished {
                        let _ = webview.set_zoom(0.8);
                        let _ = webview.eval(&scrollbar_css_clone);
                        // Emit the actual final URL after page load (avoids intermediate resource URLs)
                        if let Ok(url) = webview.url() {
                            let url_str = url.to_string();
                            if url_str.starts_with("http://") || url_str.starts_with("https://") {
                                let _ = webview.app_handle().emit("link-view-navigate", url_str);
                            }
                        }
                    }
                }),
            tauri::PhysicalPosition::new(view_x, view_y),
            tauri::PhysicalSize::new(link_view_w.saturating_sub(resizer_gutter), view_h),
        )
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

    // Tell the frontend to show the backdrop
    let _ = app.emit("link-view-show", url);

    Ok(())
}

#[tauri::command]
pub fn close_link_view(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    if let Some(wv) = app.get_webview("link-view") {
        let _ = wv.close();
    }
    let _ = app.emit("link-view-hide", ());
}

#[tauri::command]
pub fn send_radio_detect(app: tauri::AppHandle, stream: serde_json::Value) {
    let _ = app.emit("radio-browser-detected", stream);
}

#[tauri::command]
pub fn send_radio_detect_sidebar(
    app: tauri::AppHandle,
    stream_url: String,
    name: String,
    favicon: String,
) {
    use tauri::Manager;

    // Technical sanity check only: Block data URLs and obvious system pages
    if stream_url.starts_with("data:") || stream_url.starts_with("blob:") || stream_url.is_empty() {
        return;
    }

    if let Some(sidebar) = app.get_webview("sidebar-view") {
        let json_data = serde_json::json!({
            "url": stream_url,
            "name": name,
            "favicon": favicon,
        });
        let _ = sidebar.eval(format!(
            "if(window.addStream) window.addStream({json_data});"
        ));
    }
}

#[tauri::command]
pub async fn probe_and_add_stream_from_js(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    url: String,
    name: String,
    favicon: String,
    _force: bool,
) -> Result<(), AppError> {
    let proxy_port = state.proxy_port;
    tauri::async_runtime::spawn(async move {
        // 2. DISCOVERY VIA PROXY
        let proxy_url = format!(
            "http://127.0.0.1:{}/proxy?url={}",
            proxy_port,
            urlencoding::encode(&url)
        );

        if let Ok(resp) = reqwest::get(&proxy_url).await {
            let h = resp.headers();
            let ct = h
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_lowercase();

            let is_audio_ct = ct.starts_with("audio/")
                || ct.contains("mpegurl")
                || ct.contains("x-scpls")
                || ct.contains("application/ogg");

            let has_icy = h.contains_key("icy-name")
                || h.contains_key("icy-metaint")
                || h.contains_key("icy-br")
                || h.contains_key("x-audiocast-name");

            // Absolute Blacklist
            let is_garbage = ct.starts_with("image/")
                || ct.starts_with("video/")
                || ct.starts_with("font/")
                || ct.contains("javascript")
                || ct.contains("json")
                || ct.contains("xml")
                || ct.contains("pdf");

            if !is_garbage && (is_audio_ct || has_icy) {
                info!("Detected real radio stream: {} (CT: {})", url, ct);
                send_radio_detect_sidebar(app.clone(), url, name, favicon);
            } else if !is_garbage && ct.contains("text/html") {
                if let Ok(result) = scrape_radio_url_internal(proxy_port, url.clone()).await {
                    for s_url in result.stream_urls {
                        send_radio_detect_sidebar(
                            app.clone(),
                            s_url,
                            result.name.clone(),
                            result.favicon.clone(),
                        );
                    }
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn browser_back(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(parent) = app.get_window("radio-browser-window") {
        if let Some(w) = parent.get_webview("browser-view") {
            let _ = w.eval("history.back();");
        }
    }
}

#[tauri::command]
pub fn browser_forward(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(parent) = app.get_window("radio-browser-window") {
        if let Some(w) = parent.get_webview("browser-view") {
            let _ = w.eval("history.forward();");
        }
    }
}

#[tauri::command]
pub fn browser_reload(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(parent) = app.get_window("radio-browser-window") {
        if let Some(w) = parent.get_webview("browser-view") {
            let _ = w.eval("location.reload();");
        }
    }
}

#[tauri::command]
pub fn browser_stop(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(parent) = app.get_window("radio-browser-window") {
        if let Some(w) = parent.get_webview("browser-view") {
            let _ = w.eval("window.stop();");
        }
    }
}

#[tauri::command]
pub fn browser_navigate(app: tauri::AppHandle, url: String) {
    use tauri::Manager;
    if let Some(parent) = app.get_window("radio-browser-window") {
        if let Some(w) = parent.get_webview("browser-view") {
            let nav_js = format!("location.href = '{}';", url.replace("'", "\\'"));
            let _ = w.eval(&nav_js);
        }
    }
}

#[tauri::command]
pub async fn browser_get_url(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;
    
    // browser-view is a child webview of radio-browser-window
    if let Some(parent_win) = app.get_window("radio-browser-window") {
        if let Some(w) = parent_win.get_webview("browser-view") {
            let url = w.url().map_or(String::new(), |u| u.to_string());
            return Ok(url);
        }
    }
    
    // Fallback: try global search
    if let Some(w) = app.get_webview("browser-view") {
        let url = w.url().map_or(String::new(), |u| u.to_string());
        Ok(url)
    } else {
        tracing::warn!("browser_get_url: browser-view not found");
        Err("Browser view not found".to_string())
    }
}

#[tauri::command]
pub fn close_browser_window(app: tauri::AppHandle) {
    use tauri::Manager;
    
    tracing::info!("close_browser_window called");
    
    // Close browser window first (it's a Window, not WebviewWindow)
    if let Some(w) = app.get_window("radio-browser-window") {
        tracing::info!("Closing radio-browser-window");
        let _ = w.close();
    } else {
        tracing::warn!("radio-browser-window not found!");
    }
    
    // Explicitly close toolbar window (in case it didn't close automatically)
    if let Some(tb) = app.get_webview_window("toolbar-view") {
        tracing::info!("Closing toolbar-view");
        let _ = tb.close();
    } else {
        tracing::warn!("toolbar-view not found!");
    }
    
    // Focus main window
    if let Some(main) = app.get_webview_window("main") {
        tracing::info!("Focusing main window");
        let _ = main.set_focus();
        let _ = main.unminimize();
    }
}

#[tauri::command]
pub async fn open_radio_browser(app: tauri::AppHandle) -> Result<(), AppError> {
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return Err(AppError::Settings("LINUX_NOT_SUPPORTED_YET".to_string()));
    }

    #[cfg(not(target_os = "linux"))]
    {
        use tauri::{Manager, WebviewBuilder, WebviewUrl, WebviewWindowBuilder, WindowBuilder};

        // Check if the window is already open
        if let Some(win) = app.get_window("radio-browser-window") {
            let _ = win.set_focus();
            return Ok(());
        }

    // Determine theme from settings
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let settings = crate::settings::Settings::load(&data_dir);
    let theme_str = settings.theme.unwrap_or_else(|| "system".to_string());

    let is_light = match theme_str.as_str() {
        "light" => true,
        "dark" => false,
        _ => app
            .get_webview_window("main")
            .and_then(|w| w.theme().ok())
            .map(|t| matches!(t, tauri::Theme::Light))
            .unwrap_or(false),
    };

    let bg = if is_light {
        (245u8, 245u8, 245u8, 255u8)
    } else {
        (17u8, 17u8, 17u8, 255u8)
    };

    // 1. Create the base native Window (no decorations, no default webview)
    #[cfg(not(target_os = "macos"))]
    let win_builder = WindowBuilder::new(&app, "radio-browser-window")
        .title("Radiko Browser")
        .inner_size(1200.0, 700.0)
        .decorations(false)
        .resizable(true)
        .shadow(true)
        .background_color(bg.into());

    #[cfg(target_os = "macos")]
    let mut win_builder = WindowBuilder::new(&app, "radio-browser-window")
        .title("Radiko Browser")
        .inner_size(1200.0, 700.0)
        .decorations(false)
        .resizable(true)
        .shadow(true)
        .background_color(bg.into());

    #[cfg(target_os = "macos")]
    {
        win_builder = win_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }

    let window = win_builder
        .build()
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

    // macOS: hide native traffic lights
    #[cfg(target_os = "macos")]
    {
        use objc::{msg_send, sel, sel_impl, runtime::Object};
        let win_for_lights = window.clone();
        let _ = win_for_lights.clone().run_on_main_thread(move || {
            if let Ok(ptr) = win_for_lights.ns_window() {
                let ns_win = ptr as *mut Object;
                unsafe {
                    for btn_type in [0u64, 1u64, 2u64] {
                        let btn: *mut Object = msg_send![ns_win, standardWindowButton: btn_type];
                        if !btn.is_null() {
                            let _: () = msg_send![btn, setHidden: objc::runtime::YES];
                        }
                    }
                }
            }
        });
    }

    // Fixed Layout Constants (Logical)
    let sidebar_w_log = 260.0_f64;
    let toolbar_h_log = 70.0_f64;

    let sf = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .unwrap_or(tauri::PhysicalSize::new(1200, 700));

    let sidebar_w_phys = (sidebar_w_log * sf).round() as u32;
    let toolbar_h_phys = (toolbar_h_log * sf).round() as u32;

    // Get absolute window position for child window positioning
    let win_pos = window
        .outer_position()
        .unwrap_or(tauri::PhysicalPosition::new(0, 0));

    // 2. Add Loading overlay (child webview — no IPC needed)
    let _loading_view = window
        .add_child(
            WebviewBuilder::new(
                "loading-view",
                WebviewUrl::App("/browser-loading.html".into()),
            )
            .background_color(bg.into()),
            tauri::PhysicalPosition::new(0, 0),
            size,
        )
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

    // 3. Toolbar as a proper WebviewWindow so it gets Tauri IPC injected
    //    Position it absolutely over the parent window's toolbar area
    let toolbar_abs_x = win_pos.x;
    let toolbar_abs_y = win_pos.y;
    let toolbar_w = size.width;

    #[cfg(not(target_os = "macos"))]
    let tb_builder = WebviewWindowBuilder::new(
        &app,
        "toolbar-view",
        WebviewUrl::App("/browser-toolbar.html".into()),
    )
    .decorations(false)
    .resizable(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .background_color(bg.into())
    .inner_size(
        toolbar_w as f64 / sf,
        toolbar_h_log,
    )
    .position(
        toolbar_abs_x as f64 / sf,
        toolbar_abs_y as f64 / sf,
    );

    #[cfg(target_os = "macos")]
    let mut tb_builder = WebviewWindowBuilder::new(
        &app,
        "toolbar-view",
        WebviewUrl::App("/browser-toolbar.html".into()),
    )
    .decorations(false)
    .resizable(false)
    .shadow(false)
    .always_on_top(false)
    .skip_taskbar(true)
    .background_color(bg.into())
    .inner_size(
        toolbar_w as f64 / sf,
        toolbar_h_log,
    )
    .position(
        toolbar_abs_x as f64 / sf,
        toolbar_abs_y as f64 / sf,
    );

    #[cfg(target_os = "macos")]
    {
        tb_builder = tb_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }

    let _toolbar_win = tb_builder
        .build()
        .map_err(|e: tauri::Error| {
            tracing::error!("Failed to create toolbar window: {}", e);
            AppError::Settings(e.to_string())
        })?;
    
    tracing::info!("Toolbar window created successfully");

    // macOS: hide toolbar window's native traffic lights too
    // AND attach toolbar as child window of browser window so it moves/minimizes together
    #[cfg(target_os = "macos")]
    {
        use objc::{msg_send, sel, sel_impl, runtime::Object};
        let tb_win = _toolbar_win.clone();
        let parent_win = window.clone();
        let _ = tb_win.clone().run_on_main_thread(move || {
            // Hide traffic lights on toolbar window
            if let Ok(ptr) = tb_win.ns_window() {
                let ns_tb = ptr as *mut Object;
                unsafe {
                    for btn_type in [0u64, 1u64, 2u64] {
                        let btn: *mut Object = msg_send![ns_tb, standardWindowButton: btn_type];
                        if !btn.is_null() {
                            let _: () = msg_send![btn, setHidden: objc::runtime::YES];
                        }
                    }
                    // Attach toolbar as child of browser window
                    // NSWindowOrderingMode: NSWindowAbove = 1
                    if let Ok(parent_ptr) = parent_win.ns_window() {
                        let ns_parent = parent_ptr as *mut Object;
                        let _: () = msg_send![ns_parent, addChildWindow: ns_tb ordered: 1i64];
                    }
                }
            }
        });
    }

    // Scanner for capturing streams
    let passive_scanner_js = r#"
        (function() {
            const I = window.__TAURI_INTERNALS__;
            if (!I) return;
            console.log("%c[Radiko] Scanner Active", "color: #6c63ff; font-weight: bold;");

            const MEDIA_REGEX = /\.(m3u8|m3u|mp3|aac|aacp|accp|pls|ts|flac|ogg|wav|m4a)(\?|#|$)/i;

            function isNoise(url) {
                if (!url || typeof url !== 'string' || url.length < 5) return true;
                const u = url.toLowerCase();
                if (!u.startsWith('http') && !u.startsWith('/')) return true; 
                if (u.startsWith('data:') || u.startsWith('blob:') || u.startsWith('javascript:')) return true;
                return false;
            }

            function reportUrl(url, src) {
                if (!url || typeof url !== 'string') return;
                
                try { 
                    url = new URL(url, window.location.href).href; 
                } catch(e) { 
                    return; 
                }

                if (isNoise(url)) return;

                if (window._rx_seen && window._rx_seen[url]) return;
                window._rx_seen = window._rx_seen || {};
                window._rx_seen[url] = true;

                console.log("%c[Radiko Signature Candidate] " + src + " -> " + url, "color: #10b981; font-weight: bold;");
                
                const title = document.title || 'Detected Stream';
                const favEl = document.querySelector('link[rel*="icon"]');
                const fav = favEl ? favEl.href : (location.origin + "/favicon.ico");

                if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                    window.__TAURI_INTERNALS__.invoke('probe_and_add_stream_from_js', { url, name: title, favicon: fav, force: false });
                }
            }

            // 1. Direct Hooks
            try {
                const origPlay = HTMLMediaElement.prototype.play;
                HTMLMediaElement.prototype.play = function() {
                    const url = this.src || this.currentSrc || (this.querySelector('source') && this.querySelector('source').src);
                    if (url) reportUrl(url, "MediaElement");
                    return origPlay.apply(this, arguments);
                };
                
                const origFetch = window.fetch;
                window.fetch = function() {
                    if (arguments[0]) {
                        var _u = typeof arguments[0] === 'string' ? arguments[0] : (arguments[0].url || '');
                        reportUrl(_u, "Fetch");
                    }
                    return origFetch.apply(this, arguments);
                };

                const origXHROpen = XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open = function(method, url) {
                    if (url) reportUrl(url, "XHR");
                    return origXHROpen.apply(this, arguments);
                };
            } catch(e) {}
            
            // 2. Performance Observer (Network Traffic)
            if (window.PerformanceObserver) {
                try {
                    const obs = new PerformanceObserver((list) => {
                        list.getEntries().forEach((entry) => {
                            if (entry.name && entry.name.startsWith('http')) {
                                if (MEDIA_REGEX.test(entry.name) || /playlist|stream|m3u8|mp3|aac/.test(entry.name.toLowerCase())) {
                                    reportUrl(entry.name, "Network");
                                }
                            }
                        });
                    });
                    obs.observe({ entryTypes: ['resource'] });
                } catch(e) {}
            }

            // 3. Interval Scanner
            setInterval(() => {
                document.querySelectorAll('audio, video, source').forEach(el => reportUrl(el.src || el.currentSrc, "Interval"));
                document.querySelectorAll('iframe, frame, embed').forEach(el => {
                   if (el.src && el.src.startsWith('http') && !isNoise(el.src)) {
                       reportUrl(el.src, "Iframe");
                   }
                });
            }, 5000);

            // 4. Autoplay Disabler
            try {
                const disableMedia = (el) => {
                    if (!el) return;
                    el.autoplay = false;
                    el.removeAttribute('autoplay');
                    el.setAttribute('preload', 'none');
                };
                
                document.querySelectorAll('audio, video').forEach(disableMedia);

                const mediaObserver = new MutationObserver((mutations) => {
                    for (const mutation of mutations) {
                        for (const node of mutation.addedNodes) {
                            if (node.nodeName === 'AUDIO' || node.nodeName === 'VIDEO') {
                                disableMedia(node);
                            } else if (node.querySelectorAll) {
                                node.querySelectorAll('audio, video').forEach(disableMedia);
                            }
                        }
                    }
                });
                mediaObserver.observe(document.documentElement, { childList: true, subtree: true });
            } catch(e) {}
        })();
    "#;

    // 3. Add Browser Webview BEFORE sidebar
    let app_clone = app.clone();
    let _browser_view = window
        .add_child(
            WebviewBuilder::new(
                "browser-view",
                WebviewUrl::External("https://www.google.com".parse().unwrap()),
            )
            .incognito(false)
            .initialization_script(passive_scanner_js)
            .on_navigation(move |url| {
                let app_handle = app_clone.clone();
                if let Some(tb) = app_handle.get_webview("toolbar-view") {
                    let _ = tb.eval("if(window.setLoading) window.setLoading(true);");
                }
                let _ = app_handle.emit("browser-loading-started", ());
                if url.scheme() == "radiko" {
                    let pairs: std::collections::HashMap<String, String> =
                        url.query_pairs().into_owned().collect();
                    let stream_url = pairs.get("url").cloned().unwrap_or_default();
                    let name = pairs.get("name").cloned().unwrap_or_default();
                    let favicon = pairs.get("favicon").cloned().unwrap_or_default();

                    if let Some(sidebar) = app_handle.get_webview("sidebar-view") {
                        let json_data = serde_json::json!({
                            "url": stream_url,
                            "name": name,
                            "favicon": favicon,
                        });
                        let _ = sidebar.eval(format!(
                            "if(window.addStream) window.addStream({json_data});"
                        ));
                    }
                    return false;
                }

                true
            })
            .on_page_load({
                let app_for_load = app.clone();
                move |_, _| {
                    if let Some(tb) = app_for_load.get_webview("toolbar-view") {
                        let _ = tb.eval("if(window.setLoading) window.setLoading(false);");
                    }
                    let _ = app_for_load.emit("browser-loading-finished", ());
                }
            }),
            tauri::PhysicalPosition::new(0, toolbar_h_phys as i32),
            tauri::PhysicalSize::new(
                size.width.saturating_sub(sidebar_w_phys),
                size.height.saturating_sub(toolbar_h_phys),
            ),
        )
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

    // 4. Add Sidebar Webview LAST
    let _sidebar_view = window
        .add_child(
            WebviewBuilder::new(
                "sidebar-view",
                WebviewUrl::App("/browser-sidebar.html".into()),
            )
            .background_color(bg.into()),
            tauri::PhysicalPosition::new(
                size.width.saturating_sub(sidebar_w_phys) as i32,
                toolbar_h_phys as i32,
            ),
            tauri::PhysicalSize::new(sidebar_w_phys, size.height.saturating_sub(toolbar_h_phys)),
        )
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

    // 5. Linux Background Fix: Ensure the main window background doesn't flicker or show through
    #[cfg(target_os = "linux")]
    {
        let wb = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = wb.set_background_color(Some(bg.into()));
        });
    }

    // Auto-hide loading overlay after a short delay
    {
        let app_for_loading = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            if let Some(lv) = app_for_loading.get_webview("loading-view") {
                let _ = lv.hide();
            }
        });
    }

    // 6. Layout helpers
    fn layout_with_sidebar(w: &tauri::Window, app: &tauri::AppHandle) {
        use tauri::Manager;
        let sf = w.scale_factor().unwrap_or(1.0);
        let size = w
            .inner_size()
            .unwrap_or(tauri::PhysicalSize::new(1200, 700));
        let win_pos = w.outer_position().unwrap_or(tauri::PhysicalPosition::new(0, 0));

        let sidebar_w_phys = (260.0_f64 * sf).round() as u32;
        let toolbar_h_phys = (70.0_f64 * sf).round() as u32;

        let content_w_phys = size.width.saturating_sub(sidebar_w_phys);
        let content_h_phys = size.height.saturating_sub(toolbar_h_phys);

        // Toolbar window: reposition + resize to follow parent
        if let Some(tb) = app.get_webview_window("toolbar-view") {
            let _ = tb.set_position(tauri::PhysicalPosition::new(win_pos.x, win_pos.y));
            let _ = tb.set_size(tauri::PhysicalSize::new(size.width, toolbar_h_phys));
        }

        // Browser child webview
        if let Some(bv) = w.get_webview("browser-view") {
            let _ = bv.set_position(tauri::PhysicalPosition::new(0, toolbar_h_phys as i32));
            let _ = bv.set_size(tauri::PhysicalSize::new(content_w_phys, content_h_phys));
        }

        // Sidebar child webview
        if let Some(sb) = w.get_webview("sidebar-view") {
            let _ = sb.set_position(tauri::PhysicalPosition::new(content_w_phys as i32, toolbar_h_phys as i32));
            let _ = sb.set_size(tauri::PhysicalSize::new(sidebar_w_phys, content_h_phys));
        }
    }

    // 7. Initial Layout Pass (Immediate + Delayed for stability)
    layout_with_sidebar(&window, &app);

    // Do not force focus back to the parent on macOS.
    // The toolbar is an attached child window there, and stealing focus back after
    // traffic-light interactions can leave the toolbar feeling laggy/unresponsive.

    let window_init = window.clone();
    let app_init = app.clone();
    tauri::async_runtime::spawn(async move {
        for delay in [10, 100, 300, 600, 1000] {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            layout_with_sidebar(&window_init, &app_init);
        }
    });

    // 8. Window resize/move handler — keep toolbar window in sync
    let w_ev = window.clone();
    let app_ev = app.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Resized(_)
        | tauri::WindowEvent::Moved(_)
        | tauri::WindowEvent::ScaleFactorChanged { .. } => {
            let w = w_ev.clone();
            let a = app_ev.clone();
            layout_with_sidebar(&w, &a);

            let w2 = w.clone();
            let a2 = a.clone();
            tauri::async_runtime::spawn(async move {
                for delay in [5, 50, 200, 500] {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    layout_with_sidebar(&w2, &a2);
                }
            });
        }
        tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
            // Close toolbar window when main browser window closes
            if let Some(tb) = app_ev.get_webview_window("toolbar-view") {
                let _ = tb.close();
            }
        }
        _ => {}
    });

        Ok(())
    }
}


#[tauri::command]
pub fn minimize_browser_window(app: tauri::AppHandle) {
    use tauri::Manager;

    if let Some(window) = app.get_window("radio-browser-window") {
        let _ = window.minimize();
    }
}

#[tauri::command]
pub fn maximize_browser_window(app: tauri::AppHandle) {
    use tauri::Manager;

    if let Some(window) = app.get_window("radio-browser-window") {
        if let Ok(is_maximized) = window.is_maximized() {
            if is_maximized {
                let _ = window.unmaximize();
            } else {
                let _ = window.maximize();
            }
        }

        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn drag_window(app: tauri::AppHandle, label: String) {
    use tauri::Manager;
    // Try WebviewWindow first, then Window
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.start_dragging();
    } else if let Some(window) = app.get_window(&label) {
        let _ = window.start_dragging();
    }
}

#[tauri::command]
pub fn start_window_resize(app: tauri::AppHandle, label: String, direction: String) {
    use tauri_runtime::ResizeDirection;
    if let Some(window) = app.get_window(&label) {
        let dir = match direction.as_str() {
            "top" => ResizeDirection::North,
            "bottom" => ResizeDirection::South,
            "left" => ResizeDirection::West,
            "right" => ResizeDirection::East,
            "top-left" => ResizeDirection::NorthWest,
            "top-right" => ResizeDirection::NorthEast,
            "bottom-left" => ResizeDirection::SouthWest,
            "bottom-right" => ResizeDirection::SouthEast,
            _ => return,
        };
        let _ = window.start_resize_dragging(dir);
    }
}

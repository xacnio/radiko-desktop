//! Browser commands: link view, radio browser window, navigation, detection.

use tauri::{Emitter, Manager};
use tracing::info;

use crate::error::AppError;
use crate::state::AppState;

use super::scraping::scrape_radio_url_internal;

static LINK_VIEW_WIDTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(400);

pub fn internal_layout_link_view(w: &tauri::Window) {
    use tauri::Manager;
    if let Some(lv) = w.get_webview("link-view") {
        let sf = w.scale_factor().unwrap_or(1.0);
        let size = w
            .inner_size()
            .unwrap_or(tauri::PhysicalSize::new(1200, 800));

        let width_px = LINK_VIEW_WIDTH.load(std::sync::atomic::Ordering::Relaxed);
        let resizer_gutter = (7.0 * sf).round() as u32;
        let link_view_w = (width_px as f64 * sf).round() as u32;
        let titlebar_h = (38.0 * sf).round() as u32;
        let header_h = (40.0 * sf).round() as u32;
        let mini_player_h = (100.0 * sf).round() as u32;

        let view_x = size.width.saturating_sub(link_view_w) + resizer_gutter;
        let view_y = titlebar_h + header_h;
        let view_h = size.height.saturating_sub(view_y + mini_player_h);

        let final_w = link_view_w.saturating_sub(resizer_gutter);

        let _ = lv.set_position(tauri::PhysicalPosition::new(view_x, view_y));
        let _ = lv.set_size(tauri::PhysicalSize::new(final_w, view_h));
    }
}

#[tauri::command]
pub fn update_link_view_width(app: tauri::AppHandle, width: u32) {
    use tauri::Manager;
    LINK_VIEW_WIDTH.store(width, std::sync::atomic::Ordering::Relaxed);
    if let Some(main_win) = app.get_window("main") {
        internal_layout_link_view(&main_win);
    } else {
        println!("UPDATE_WIDTH: ERROR - Main window not found!");
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
    let resizer_gutter = (6.0 * sf).round() as u32;
    let link_view_w = (width_px as f64 * sf).round() as u32;
    let titlebar_h = (38.0 * sf).round() as u32;
    let header_h = (40.0 * sf).round() as u32;
    let mini_player_h = (100.0 * sf).round() as u32;

    let view_x = size.width.saturating_sub(link_view_w) + resizer_gutter;
    let view_y = titlebar_h + header_h;
    let view_h = size.height.saturating_sub(view_y + mini_player_h);

    let app_handle = app.clone();
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
                .on_navigation(move |url| {
                    let _ = app_handle.emit("link-view-navigate", url.to_string());
                    true
                })
                .on_page_load(move |webview, payload| {
                    if payload.event() == tauri::webview::PageLoadEvent::Finished {
                        let _ = webview.set_zoom(0.8);
                        let _ = webview.eval(&scrollbar_css_clone);
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
    if let Some(w) = app.get_webview("browser-view") {
        let _ = w.eval("history.back();");
    }
}

#[tauri::command]
pub fn browser_forward(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview("browser-view") {
        let _ = w.eval("history.forward();");
    }
}

#[tauri::command]
pub fn browser_reload(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview("browser-view") {
        let _ = w.eval("location.reload();");
    }
}

#[tauri::command]
pub fn browser_stop(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview("browser-view") {
        let _ = w.eval("window.stop();");
    }
}

#[tauri::command]
pub fn browser_navigate(app: tauri::AppHandle, url: String) {
    use tauri::Manager;
    if let Some(w) = app.get_webview("browser-view") {
        let nav_js = format!("location.href = '{}';", url.replace("'", "\\'"));
        let _ = w.eval(&nav_js);
    }
}

#[tauri::command]
pub async fn browser_get_url(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview("browser-view") {
        let url = w.url().map_or(String::new(), |u| u.to_string());
        Ok(url)
    } else {
        Err("Browser view not found".to_string())
    }
}

#[tauri::command]
pub fn close_browser_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
        let _ = main.unminimize();
    }
    if let Some(w) = app.get_window("radio-browser-window") {
        let _ = w.close();
    }
}

#[tauri::command]
pub async fn open_radio_browser(app: tauri::AppHandle) -> Result<(), AppError> {
    use tauri::{Manager, WebviewBuilder, WebviewUrl, WindowBuilder};

    #[cfg(target_os = "linux")]
    {
        return Err(AppError::Settings("LINUX_NOT_SUPPORTED_YET".to_string()));
    }

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

    // Determine if the app is currently in light mode
    let is_light = match theme_str.as_str() {
        "light" => true,
        "dark" => false,
        _ => {
            // "system" — check actual system theme
            app.get_window("main")
                .and_then(|w| w.theme().ok())
                .map(|t| matches!(t, tauri::Theme::Light))
                .unwrap_or(false)
        }
    };

    let bg = if is_light {
        (245u8, 245u8, 245u8, 255u8)
    } else {
        (17u8, 17u8, 17u8, 255u8)
    };

    // 1. Create the base native Window
    let window = WindowBuilder::new(&app, "radio-browser-window")
        .title("Radiko Browser")
        .inner_size(1200.0, 700.0)
        .decorations(false) // Frameless for premium look
        .resizable(true)    // Enable resizing
        .shadow(true)      // Helps some Linux environments with hit testing
        .background_color(bg.into())
        .build()
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

    // Fixed Layout Constants (Logical)
    let sidebar_w_log = 260.0;
    let toolbar_h_log = 70.0;

    let sf = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .unwrap_or(tauri::PhysicalSize::new(1200, 700));

    let sidebar_w_phys = (sidebar_w_log * sf) as u32;
    let toolbar_h_phys = (toolbar_h_log * sf) as u32;

    // 2. Add Loading overlay FIRST
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

    // 3. Add Toolbar Webview
    let _toolbar_view = window
        .add_child(
            WebviewBuilder::new(
                "toolbar-view",
                WebviewUrl::App("/browser-toolbar.html".into()),
            )
            .background_color(bg.into()),
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(size.width, toolbar_h_phys),
        )
        .map_err(|e: tauri::Error| AppError::Settings(e.to_string()))?;

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

    // 6. macOS: Force DevTools to open in a SEPARATE WINDOW
    #[cfg(target_os = "macos")]
    {
        let w_detach = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let _ = w_detach.run_on_main_thread(move || {
                use objc::runtime::Object;
                use objc::{class, msg_send, sel, sel_impl};
                unsafe {
                    let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                    let windows: *mut Object = msg_send![ns_app, windows];
                    let win_count: u64 = msg_send![windows, count];
                    info!("[macOS Inspector Detach] Found {} windows", win_count);

                    let wk_class = class!(WKWebView);

                    for i in 0..win_count {
                        let ns_win: *mut Object = msg_send![windows, objectAtIndex: i];
                        let content_view: *mut Object = msg_send![ns_win, contentView];
                        if content_view.is_null() {
                            continue;
                        }

                        detach_inspector_recursive(content_view, wk_class, 0);
                    }
                }
            });
        });
    }

    // 6. Layout helpers
    fn layout_with_sidebar(w: &tauri::Window) {
        let sf = w.scale_factor().unwrap_or(1.0);
        let size = w
            .inner_size()
            .unwrap_or(tauri::PhysicalSize::new(1200, 700));
        
        let sidebar_w_log = 260.0;
        let toolbar_h_log = 70.0;
        
        let sidebar_w_phys = (sidebar_w_log * sf).round() as u32;
        let toolbar_h_phys = (toolbar_h_log * sf).round() as u32;
        
        let content_w_phys = size.width.saturating_sub(sidebar_w_phys);
        let content_h_phys = size.height.saturating_sub(toolbar_h_phys);

        use tauri::Manager;
        
        // 1. Toolbar: Full Width
        if let Some(tb) = w.get_webview("toolbar-view") {
            let _ = tb.set_position(tauri::PhysicalPosition::new(0, 0));
            let _ = tb.set_size(tauri::PhysicalSize::new(size.width, toolbar_h_phys));
        }
        
        // 2. Browser: Left Side
        if let Some(bv) = w.get_webview("browser-view") {
            let _ = bv.set_position(tauri::PhysicalPosition::new(0, toolbar_h_phys as i32));
            let _ = bv.set_size(tauri::PhysicalSize::new(content_w_phys, content_h_phys));
        }
        
        // 3. Sidebar: Right Side
        if let Some(sb) = w.get_webview("sidebar-view") {
            let _ = sb.set_position(tauri::PhysicalPosition::new(content_w_phys as i32, toolbar_h_phys as i32));
            let _ = sb.set_size(tauri::PhysicalSize::new(sidebar_w_phys, content_h_phys));
        }
    }

    // 7. Initial Layout Pass (Immediate + Delayed for stability)
    layout_with_sidebar(&window);
    
    let window_init = window.clone();
    tauri::async_runtime::spawn(async move {
        for delay in [10, 100, 300, 600, 1000] {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            layout_with_sidebar(&window_init);
        }
    });

    // 8. Window resize handler
    let w_ev = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {
            let w = w_ev.clone();
            layout_with_sidebar(&w);

            let w_delayed = w.clone();
            tauri::async_runtime::spawn(async move {
                for delay in [5, 50, 200, 500] {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    layout_with_sidebar(&w_delayed);
                }
            });
        }
        _ => {}
    });

    Ok(())
}

/// macOS: Recursively walk NSView hierarchy, find WKWebViews, detach their inspector.
#[cfg(target_os = "macos")]
unsafe fn detach_inspector_recursive(
    view: *mut objc::runtime::Object,
    wk_class: &objc::runtime::Class,
    depth: usize,
) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    if view.is_null() || depth > 10 {
        return;
    }

    let name = (*view).class().name().to_string();

    let is_wk: bool = msg_send![view, isKindOfClass: wk_class];
    if is_wk {
        info!(
            "[Inspector Detach] Found WKWebView '{}' at depth {}",
            name, depth
        );

        let has_attach: bool =
            msg_send![view, respondsToSelector: sel!(_setInspectorAttachmentView:)];
        info!(
            "[Inspector Detach]   respondsTo _setInspectorAttachmentView: {}",
            has_attach
        );
        if has_attach {
            let nil: *mut Object = std::ptr::null_mut();
            let _: () = msg_send![view, _setInspectorAttachmentView: nil];
            info!("[Inspector Detach]   ✓ Done");
        }
    } else if depth < 4 {
        info!("[Inspector Detach] depth {}: {}", depth, name);
    }

    let subviews: *mut Object = msg_send![view, subviews];
    let count: u64 = msg_send![subviews, count];
    for i in 0..count {
        let subview: *mut Object = msg_send![subviews, objectAtIndex: i];
        detach_inspector_recursive(subview, wk_class, depth + 1);
    }
}

#[tauri::command]
pub fn minimize_browser_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_window("radio-browser-window") {
        let _ = window.minimize();
    }
}

#[tauri::command]
pub fn maximize_browser_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_window("radio-browser-window") {
        if let Ok(is_maximized) = window.is_maximized() {
            if is_maximized {
                let _ = window.unmaximize();
            } else {
                let _ = window.maximize();
            }
        }
    }
}

#[tauri::command]
pub fn drag_window(app: tauri::AppHandle, label: String) {
    if let Some(window) = app.get_window(&label) {
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

use std::thread;
use tauri::AppHandle;
use tiny_http::{Header, Response, Server};
use tracing::info;

pub struct ProxyState {
    pub port: u16,
}

pub fn start_proxy(app: AppHandle) -> ProxyState {
    let server = Server::http("127.0.0.1:0").expect("Failed to start proxy server");
    #[allow(unreachable_patterns)]
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
        _ => 0,
    };

    let app_clone = app.clone();

    thread::spawn(move || {
        for request in server.incoming_requests() {
            debug_proxy_request(&request);
            let ac = app_clone.clone();

            thread::spawn(move || {
                let url_str = request.url().to_string();

                if url_str.starts_with("/proxy?") || url_str.starts_with("/stream?") {
                    let query = url_str.split_once('?').map(|x| x.1).unwrap_or("");
                    let mut target_url = String::new();

                    for pair in query.split('&') {
                        let mut parts = pair.splitn(2, '=');
                        if parts.next() == Some("url") {
                            if let Some(val) = parts.next() {
                                target_url =
                                    urlencoding::decode(val).unwrap_or(val.into()).to_string();
                            }
                        }
                    }

                    if target_url.is_empty() {
                        let _ = request
                            .respond(Response::from_string("Missing url").with_status_code(400));
                        return;
                    }

                    // Use blocking reqwest to avoid async runtime issues in this thread
                    let client = reqwest::blocking::Client::builder()
                        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15")
                        .timeout(std::time::Duration::from_secs(10))
                        .build()
                        .unwrap();

                    match client.get(&target_url).send() {
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            let content_type = resp
                                .headers()
                                .get(reqwest::header::CONTENT_TYPE)
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("application/octet-stream")
                                .to_string();

                            // 100% TECHNICAL HEADER-BASED DETECTION (No extensions)
                            let h = resp.headers();
                            let is_audio_ct = content_type.starts_with("audio/")
                                || content_type.contains("mpegurl")
                                || content_type.contains("x-scpls")
                                || content_type.contains("application/ogg")
                                || content_type.contains("audio/mpeg")
                                || content_type.contains("audio/aac")
                                || content_type.contains("audio/ogg")
                                || content_type.contains("aacp")
                                || content_type.contains("accp");

                            let has_icy = h.contains_key("icy-name")
                                || h.contains_key("icy-metaint")
                                || h.contains_key("icy-br")
                                || h.contains_key("x-audiocast-name");
                            let is_shoutcast = h
                                .get("Server")
                                .and_then(|v| v.to_str().ok())
                                .map(|s| s.to_lowercase().contains("shoutcast"))
                                .unwrap_or(false);

                            // Absolute Blacklist: Block scripts and images even if misidentified
                            let is_garbage = content_type.starts_with("image/")
                                || content_type.starts_with("video/")
                                || content_type.starts_with("text/")
                                || content_type.starts_with("font/")
                                || content_type.contains("javascript")
                                || content_type.contains("json")
                                || content_type.contains("xml")
                                || content_type.contains("pdf");

                            if url_str.starts_with("/proxy?")
                                && (is_audio_ct || has_icy || is_shoutcast)
                                && !is_garbage
                            {
                                info!(
                                    "Detected radio stream via proxy headers: {} (CT: {})",
                                    target_url, content_type
                                );
                                crate::commands::send_radio_detect_sidebar(
                                    ac.clone(),
                                    target_url.clone(),
                                    "Proxy Detection".to_string(),
                                    "".to_string(),
                                );
                            }

                            // Respond with the body
                            let response = Response::new(
                                tiny_http::StatusCode(status),
                                vec![Header::from_bytes(
                                    &b"Content-Type"[..],
                                    content_type.as_bytes(),
                                )
                                .unwrap()],
                                resp, // reqwest::blocking::Response implements Read
                                None,
                                None,
                            );
                            let _ = request.respond(response);
                        }
                        Err(e) => {
                            let _ = request.respond(
                                Response::from_string(format!("Proxy Error: {}", e))
                                    .with_status_code(502),
                            );
                        }
                    }
                } else {
                    let _ = request.respond(
                        Response::from_string("Radiko Proxy Active").with_status_code(200),
                    );
                }
            });
        }
    });

    ProxyState { port }
}

fn debug_proxy_request(request: &tiny_http::Request) {
    info!("Proxy Request: {} {}", request.method(), request.url());
}

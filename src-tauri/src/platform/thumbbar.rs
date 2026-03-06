//! Windows Taskbar Thumbnail Toolbar Buttons (Previous / Play-Pause / Next)
//!
//! Uses ITaskbarList3 COM to add media buttons to the taskbar thumbnail.
//! Dynamically updates Play ↔ Pause icon based on playback state.

#![allow(clippy::upper_case_acronyms)]

use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

static LOCALES_DIR: include_dir::Dir =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../src/locales");

// ── Constants ────────────────────────────────────────────────────────
const WM_COMMAND: u32 = 0x0111;
const WM_USER: u32 = 0x0400;
const WM_THUMBBAR_UPDATE: u32 = WM_USER + 0x700;
const THBN_CLICKED: u16 = 0x1800;

const BTN_PREV: u32 = 100;
const BTN_PLAYPAUSE: u32 = 101;
const BTN_NEXT: u32 = 102;

const THB_ICON: u32 = 0x02;
const THB_TOOLTIP: u32 = 0x04;
const THB_FLAGS: u32 = 0x08;
const THBF_ENABLED: u32 = 0x00;

const GWLP_WNDPROC: i32 = -4;
const SZ: i32 = 20; // 20×20 for better quality

// ── Win32 / COM types ────────────────────────────────────────────────
#[repr(C)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const CLSID_TASKBAR_LIST: GUID = GUID {
    data1: 0x56FDF344,
    data2: 0xFD6D,
    data3: 0x11D0,
    data4: [0x95, 0x8A, 0x00, 0x60, 0x97, 0xC9, 0xA0, 0x90],
};
const IID_ITASKBARLIST3: GUID = GUID {
    data1: 0xEA1AFB91,
    data2: 0x9E28,
    data3: 0x4B86,
    data4: [0x90, 0xE9, 0x9E, 0x9F, 0x8A, 0x5E, 0xEF, 0xAF],
};

type HWND = isize;
type HRESULT = i32;

#[repr(C)]
#[derive(Clone)]
struct THUMBBUTTON {
    dw_mask: u32,
    i_id: u32,
    i_bitmap: u32,
    h_icon: isize,
    sz_tip: [u16; 260],
    dw_flags: u32,
}

#[repr(C)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_ppm: i32,
    bi_y_ppm: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct ICONINFO {
    f_icon: i32,
    x_hotspot: u32,
    y_hotspot: u32,
    hbm_mask: isize,
    hbm_color: isize,
}

#[repr(C)]
struct ITaskbarList3Vtbl {
    query_interface: usize,
    add_ref: usize,
    release: usize,
    hr_init: usize,
    add_tab: usize,
    delete_tab: usize,
    activate_tab: usize,
    set_active_alt: usize,
    mark_fullscreen_window: usize,
    set_progress_value: usize,
    set_progress_state: usize,
    register_tab: usize,
    unregister_tab: usize,
    set_tab_order: usize,
    set_tab_active: usize,
    thumb_bar_add_buttons:
        unsafe extern "system" fn(*mut ITaskbarList3, HWND, u32, *const THUMBBUTTON) -> HRESULT,
    thumb_bar_update_buttons:
        unsafe extern "system" fn(*mut ITaskbarList3, HWND, u32, *const THUMBBUTTON) -> HRESULT,
}

#[repr(C)]
struct ITaskbarList3 {
    vtbl: *const ITaskbarList3Vtbl,
}

extern "system" {
    fn CoInitializeEx(reserved: *const std::ffi::c_void, flags: u32) -> HRESULT;
    fn CoCreateInstance(
        rclsid: *const GUID,
        outer: *const std::ffi::c_void,
        ctx: u32,
        riid: *const GUID,
        ppv: *mut *mut std::ffi::c_void,
    ) -> HRESULT;
    fn SetWindowLongPtrW(hwnd: HWND, index: i32, new_long: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: HWND, index: i32) -> isize;
    fn CallWindowProcW(prev: isize, hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn CreateIconIndirect(icon_info: *const ICONINFO) -> isize;
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn DeleteDC(hdc: isize) -> i32;
    fn DeleteObject(obj: isize) -> i32;
    fn CreateDIBSection(
        hdc: isize,
        pbmi: *const BITMAPINFOHEADER,
        usage: u32,
        ppv_bits: *mut *mut u8,
        h_section: isize,
        offset: u32,
    ) -> isize;
    fn CreateBitmap(w: i32, h: i32, planes: u32, bpp: u32, bits: *const u8) -> isize;
    fn PostMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> i32;
}

// ── Global state ─────────────────────────────────────────────────────
struct ThumbBarState {
    tbl: *mut ITaskbarList3,
    hwnd: HWND,
    is_playing: bool,
    icon_play: isize,
    icon_pause: isize,
    tip_play: [u16; 260],
    tip_pause: [u16; 260],
    ready: bool,
}
unsafe impl Send for ThumbBarState {}

static THUMB_STATE: Mutex<Option<ThumbBarState>> = Mutex::new(None);
static mut ORIGINAL_WNDPROC: isize = 0;
static mut APP_HANDLE: Option<AppHandle> = None;

unsafe extern "system" fn thumb_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_COMMAND {
        let id = (wparam & 0xFFFF) as u32;
        let code = ((wparam >> 16) & 0xFFFF) as u16;
        if code == THBN_CLICKED {
            if let Some(ref handle) = APP_HANDLE {
                match id {
                    BTN_PREV => {
                        let _ = handle.emit("media-key", "previous");
                    }
                    BTN_PLAYPAUSE => {
                        let is_playing = THUMB_STATE
                            .lock()
                            .ok()
                            .and_then(|s| s.as_ref().map(|ts| ts.is_playing))
                            .unwrap_or(false);
                        if is_playing {
                            let _ = handle.emit("media-key", "pause");
                        } else {
                            let _ = handle.emit("media-key", "toggle");
                        }
                    }
                    BTN_NEXT => {
                        let _ = handle.emit("media-key", "next");
                    }
                    _ => {}
                }
            }
            return 0;
        }
    }
    // Handle update request posted from set_playing()
    if msg == WM_THUMBBAR_UPDATE {
        do_update_button();
        return 0;
    }
    CallWindowProcW(ORIGINAL_WNDPROC, hwnd, msg, wparam, lparam)
}

/// Actually performs the COM update — runs on the window's message loop thread
fn do_update_button() {
    if let Ok(guard) = THUMB_STATE.lock() {
        if let Some(ref state) = *guard {
            if !state.ready {
                return;
            }
            let mask = THB_ICON | THB_TOOLTIP | THB_FLAGS;
            let button = THUMBBUTTON {
                dw_mask: mask,
                i_id: BTN_PLAYPAUSE,
                i_bitmap: 0,
                h_icon: if state.is_playing {
                    state.icon_pause
                } else {
                    state.icon_play
                },
                sz_tip: if state.is_playing {
                    state.tip_pause
                } else {
                    state.tip_play
                },
                dw_flags: THBF_ENABLED,
            };
            unsafe {
                let vtbl = &*(*state.tbl).vtbl;
                (vtbl.thumb_bar_update_buttons)(state.tbl, state.hwnd, 1, &button);
            }
        }
    }
}

// ── Icon drawing (20×20, 32-bit ARGB top-down, anti-aliased) ─────────

const PIXELS: usize = (SZ * SZ) as usize;

unsafe fn make_icon(pixels: &[u32; PIXELS]) -> isize {
    let hdc = CreateCompatibleDC(0);
    let bmi = BITMAPINFOHEADER {
        bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        bi_width: SZ,
        bi_height: -SZ,
        bi_planes: 1,
        bi_bit_count: 32,
        bi_compression: 0,
        bi_size_image: 0,
        bi_x_ppm: 0,
        bi_y_ppm: 0,
        bi_clr_used: 0,
        bi_clr_important: 0,
    };
    let mut bits: *mut u8 = std::ptr::null_mut();
    let hbm_color = CreateDIBSection(hdc, &bmi, 0, &mut bits, 0, 0);
    if !bits.is_null() {
        std::ptr::copy_nonoverlapping(pixels.as_ptr() as *const u8, bits, PIXELS * 4);
    }
    let mask_bytes = ((SZ + 31) / 32 * 4 * SZ) as usize; // WORD-aligned
    let mask = vec![0u8; mask_bytes];
    let hbm_mask = CreateBitmap(SZ, SZ, 1, 1, mask.as_ptr());
    let info = ICONINFO {
        f_icon: 1,
        x_hotspot: 0,
        y_hotspot: 0,
        hbm_mask,
        hbm_color,
    };
    let icon = CreateIconIndirect(&info);
    DeleteObject(hbm_color);
    DeleteObject(hbm_mask);
    DeleteDC(hdc);
    icon
}

/// Set pixel with full opacity
fn px(buf: &mut [u32; PIXELS], x: i32, y: i32) {
    if (0..SZ).contains(&x) && (0..SZ).contains(&y) {
        buf[(y * SZ + x) as usize] = 0xFFFFFFFF;
    }
}

/// Set pixel with specific alpha (0-255) for anti-aliasing
fn px_aa(buf: &mut [u32; PIXELS], x: i32, y: i32, alpha: u8) {
    if (0..SZ).contains(&x) && (0..SZ).contains(&y) {
        let a = alpha as u32;
        buf[(y * SZ + x) as usize] = (a << 24) | (a << 16) | (a << 8) | a; // premultiplied white
    }
}

/// Draw a right-pointing triangle ▶ with anti-aliased edges
fn tri_right_aa(buf: &mut [u32; PIXELS], cx: f32, cy: f32, half_h: f32) {
    // Triangle vertices: left-base-top, left-base-bottom, right-tip
    // base_x = cx - half_h, tip_x = cx + half_h
    let base_x = cx - half_h;
    let tip_x = cx + half_h;
    let top_y = cy - half_h;
    let bot_y = cy + half_h;

    for py in 0..SZ {
        for ppx in 0..SZ {
            let fx = ppx as f32 + 0.5;
            let fy = py as f32 + 0.5;

            // Check if point is inside triangle
            // The triangle has: left edge at base_x, right tip at tip_x
            // Top edge: from (base_x, top_y) to (tip_x, cy)
            // Bottom edge: from (base_x, bot_y) to (tip_x, cy)

            if fx < base_x - 0.5 || fx > tip_x + 0.5 {
                continue;
            }
            if fy < top_y - 0.5 || fy > bot_y + 0.5 {
                continue;
            }

            // At x position fx, the triangle spans from top_edge_y to bot_edge_y
            let t = if (tip_x - base_x).abs() < 0.001 {
                0.0
            } else {
                (fx - base_x) / (tip_x - base_x)
            };
            let t = t.clamp(0.0, 1.0);
            let edge_top = top_y + t * (cy - top_y);
            let edge_bot = bot_y + t * (cy - bot_y);

            // Distance from edges for anti-aliasing
            let inside_left = fx - base_x;
            let inside_right = tip_x - fx;
            let inside_top = fy - edge_top;
            let inside_bot = edge_bot - fy;

            let min_dist = inside_left
                .min(inside_right)
                .min(inside_top)
                .min(inside_bot);

            if min_dist >= 0.5 {
                px(buf, ppx, py);
            } else if min_dist > -0.5 {
                let alpha = ((min_dist + 0.5) * 255.0) as u8;
                if alpha > 10 {
                    px_aa(buf, ppx, py, alpha);
                }
            }
        }
    }
}

/// Draw a left-pointing triangle ◀ with anti-aliased edges
fn tri_left_aa(buf: &mut [u32; PIXELS], cx: f32, cy: f32, half_h: f32) {
    let tip_x = cx - half_h;
    let base_x = cx + half_h;
    let top_y = cy - half_h;
    let bot_y = cy + half_h;

    for py in 0..SZ {
        for ppx in 0..SZ {
            let fx = ppx as f32 + 0.5;
            let fy = py as f32 + 0.5;

            if fx < tip_x - 0.5 || fx > base_x + 0.5 {
                continue;
            }
            if fy < top_y - 0.5 || fy > bot_y + 0.5 {
                continue;
            }

            let t = if (base_x - tip_x).abs() < 0.001 {
                0.0
            } else {
                (base_x - fx) / (base_x - tip_x)
            };
            let t = t.clamp(0.0, 1.0);
            let edge_top = top_y + t * (cy - top_y);
            let edge_bot = bot_y + t * (cy - bot_y);

            let inside_left = fx - tip_x;
            let inside_right = base_x - fx;
            let inside_top = fy - edge_top;
            let inside_bot = edge_bot - fy;

            let min_dist = inside_left
                .min(inside_right)
                .min(inside_top)
                .min(inside_bot);

            if min_dist >= 0.5 {
                px(buf, ppx, py);
            } else if min_dist > -0.5 {
                let alpha = ((min_dist + 0.5) * 255.0) as u8;
                if alpha > 10 {
                    px_aa(buf, ppx, py, alpha);
                }
            }
        }
    }
}

/// Draw anti-aliased filled rectangle
fn rect_aa(buf: &mut [u32; PIXELS], x1: f32, y1: f32, x2: f32, y2: f32) {
    for py in 0..SZ {
        for ppx in 0..SZ {
            let fx = ppx as f32 + 0.5;
            let fy = py as f32 + 0.5;

            let dl = fx - x1;
            let dr = x2 - fx;
            let dt = fy - y1;
            let db = y2 - fy;
            let min_d = dl.min(dr).min(dt).min(db);

            if min_d >= 0.5 {
                px(buf, ppx, py);
            } else if min_d > -0.5 {
                let alpha = ((min_d + 0.5) * 255.0) as u8;
                if alpha > 10 {
                    px_aa(buf, ppx, py, alpha);
                }
            }
        }
    }
}

/// ◀ Previous — single bold left triangle
fn icon_prev() -> [u32; PIXELS] {
    let mut b = [0u32; PIXELS];
    let cy = SZ as f32 / 2.0;
    tri_left_aa(&mut b, 10.0, cy, 8.0);
    b
}

/// ▶ Play — big bold right triangle centered
fn icon_play() -> [u32; PIXELS] {
    let mut b = [0u32; PIXELS];
    let cy = SZ as f32 / 2.0;
    tri_right_aa(&mut b, 10.0, cy, 8.0);
    b
}

/// ⏸ Pause — two thick vertical bars
fn icon_pause() -> [u32; PIXELS] {
    let mut b = [0u32; PIXELS];
    let cy = SZ as f32 / 2.0;
    let h = 7.0;
    rect_aa(&mut b, 4.0, cy - h, 8.0, cy + h);
    rect_aa(&mut b, 12.0, cy - h, 16.0, cy + h);
    b
}

/// ▶ Next — single bold right triangle
fn icon_next() -> [u32; PIXELS] {
    let mut b = [0u32; PIXELS];
    let cy = SZ as f32 / 2.0;
    tri_right_aa(&mut b, 10.0, cy, 8.0);
    b
}

// ── Locale helper ────────────────────────────────────────────────────

fn get_tooltip(key: &str, fallback: &str) -> String {
    let mut lang_code = String::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        let path = std::path::PathBuf::from(appdata)
            .join("dev.xacnio.radikodesktop")
            .join("settings.json");
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(l) = json.get("language").and_then(|v| v.as_str()) {
                    lang_code = l.to_string();
                }
            }
        }
    }
    if lang_code.is_empty() {
        extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
            fn GetLocaleInfoW(locale: u32, ty: u32, data: *mut u16, len: i32) -> i32;
        }
        let lang_id = unsafe { GetUserDefaultUILanguage() } as u32;
        let mut buf = [0u16; 9];
        let res = unsafe { GetLocaleInfoW(lang_id, 0x0059, buf.as_mut_ptr(), 9) };
        if res > 0 {
            if let Ok(s) = String::from_utf16(&buf[..(res - 1) as usize]) {
                lang_code = s.to_lowercase();
            }
        }
        if lang_code.is_empty() {
            lang_code = "en".to_string();
        }
    }
    let file_name = format!("{}.json", lang_code);
    if let Some(file) = LOCALES_DIR
        .get_file(&file_name)
        .or_else(|| LOCALES_DIR.get_file("en.json"))
    {
        if let Some(json_str) = file.contents_utf8() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                let parts: Vec<&str> = key.split('.').collect();
                if parts.len() == 2 {
                    if let Some(s) = v
                        .get(parts[0])
                        .and_then(|o| o.get(parts[1]))
                        .and_then(|s| s.as_str())
                    {
                        return s.to_string();
                    }
                }
            }
        }
    }
    fallback.to_string()
}

fn make_tip(s: &str) -> [u16; 260] {
    let mut buf = [0u16; 260];
    for (i, c) in s.encode_utf16().enumerate() {
        if i >= 259 {
            break;
        }
        buf[i] = c;
    }
    buf
}

// ── Public API ───────────────────────────────────────────────────────

pub fn setup_thumb_buttons(hwnd_raw: isize, app_handle: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));

        let tip_prev_s = get_tooltip("thumbbar.previousStation", "Previous station");
        let tip_play_s = get_tooltip("thumbbar.playPause", "Play / Pause");
        let tip_pause_s = get_tooltip("thumbbar.pause", "Pause");
        let tip_next_s = get_tooltip("thumbbar.nextStation", "Next station");

        unsafe {
            CoInitializeEx(std::ptr::null(), 0x2); // COINIT_APARTMENTTHREADED

            let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_TASKBAR_LIST,
                std::ptr::null(),
                1,
                &IID_ITASKBARLIST3,
                &mut ptr,
            );
            if hr != 0 || ptr.is_null() {
                warn!("ITaskbarList3 creation failed: 0x{:08X}", hr);
                return;
            }

            let tbl = ptr as *mut ITaskbarList3;
            let vtbl = &*(*tbl).vtbl;
            let hr_init: unsafe extern "system" fn(*mut ITaskbarList3) -> HRESULT =
                std::mem::transmute(vtbl.hr_init);
            hr_init(tbl);

            let h_play = make_icon(&icon_play());
            let h_pause = make_icon(&icon_pause());
            let h_prev = make_icon(&icon_prev());
            let h_next = make_icon(&icon_next());

            let mask = THB_ICON | THB_TOOLTIP | THB_FLAGS;
            let buttons = [
                THUMBBUTTON {
                    dw_mask: mask,
                    i_id: BTN_PREV,
                    i_bitmap: 0,
                    h_icon: h_prev,
                    sz_tip: make_tip(&tip_prev_s),
                    dw_flags: THBF_ENABLED,
                },
                THUMBBUTTON {
                    dw_mask: mask,
                    i_id: BTN_PLAYPAUSE,
                    i_bitmap: 0,
                    h_icon: h_play,
                    sz_tip: make_tip(&tip_play_s),
                    dw_flags: THBF_ENABLED,
                },
                THUMBBUTTON {
                    dw_mask: mask,
                    i_id: BTN_NEXT,
                    i_bitmap: 0,
                    h_icon: h_next,
                    sz_tip: make_tip(&tip_next_s),
                    dw_flags: THBF_ENABLED,
                },
            ];

            let hr = (vtbl.thumb_bar_add_buttons)(tbl, hwnd_raw, 3, buttons.as_ptr());
            if hr != 0 {
                warn!("ThumbBarAddButtons failed: 0x{:08X}", hr);
                return;
            }

            // Subclass FIRST, then store state, so WM_THUMBBAR_UPDATE can be handled
            APP_HANDLE = Some(app_handle);
            ORIGINAL_WNDPROC = GetWindowLongPtrW(hwnd_raw, GWLP_WNDPROC);
            SetWindowLongPtrW(hwnd_raw, GWLP_WNDPROC, thumb_wndproc as *const () as isize);

            *THUMB_STATE.lock().unwrap() = Some(ThumbBarState {
                tbl,
                hwnd: hwnd_raw,
                is_playing: false,
                icon_play: h_play,
                icon_pause: h_pause,
                tip_play: make_tip(&tip_play_s),
                tip_pause: make_tip(&tip_pause_s),
                ready: true,
            });

            info!("Taskbar thumbnail toolbar ready");
        }
    });
}

/// Called from events.rs when playback state changes.
/// Posts a message to the window thread to update the button safely.
pub fn set_playing(playing: bool) {
    if let Ok(mut guard) = THUMB_STATE.lock() {
        if let Some(ref mut state) = *guard {
            if !state.ready || state.is_playing == playing {
                return;
            }
            state.is_playing = playing;
            // Post to window thread so COM update happens on the right thread
            unsafe {
                PostMessageW(state.hwnd, WM_THUMBBAR_UPDATE, 0, 0);
            }
        }
    }
}

use std::sync::atomic::{AtomicBool, Ordering, AtomicIsize};
use tauri::{AppHandle, Manager, Emitter};
use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::{HHOOK, HWND, RECT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_MBUTTONDOWN,
    GetWindowRect, MSLLHOOKSTRUCT, WindowFromPoint
};
use once_cell::sync::Lazy;

use std::sync::atomic::AtomicU64;

pub static LAST_HIDE_TIME: AtomicU64 = AtomicU64::new(0);

static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
static IS_HOOKED: AtomicBool = AtomicBool::new(false);
static APP_HANDLE: Lazy<std::sync::Mutex<Option<AppHandle>>> = Lazy::new(|| std::sync::Mutex::new(None));

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0 {
        if w_param as u32 == WM_LBUTTONDOWN || w_param as u32 == WM_RBUTTONDOWN || w_param as u32 == WM_MBUTTONDOWN {
            
            let hook_struct = &*(l_param as *const MSLLHOOKSTRUCT);
            let hwnd = WindowFromPoint(hook_struct.pt);
            
            if !hwnd.is_null() {
                if let Ok(mut handle) = APP_HANDLE.lock() {
                    if let Some(app) = handle.as_ref() {
                        if let Some(window) = app.get_webview_window("tray") {
                            if window.is_visible().unwrap_or(false) {
                                let mut inside = false;
                                if let Ok(pos) = window.outer_position() {
                                    if let Ok(size) = window.outer_size() {
                                        let wx = pos.x as i32;
                                        let wy = pos.y as i32;
                                        let ww = size.width as i32;
                                        let wh = size.height as i32;
                                        let mx = hook_struct.pt.x;
                                        let my = hook_struct.pt.y;
                                        
                                        if mx >= wx && mx <= (wx + ww) && my >= wy && my <= (wy + wh) {
                                            inside = true;
                                        }
                                    }
                                }
                                
                                if !inside {
                                    LAST_HIDE_TIME.store(
                                        std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or(std::time::Duration::from_millis(0))
                                            .as_millis() as u64,
                                        Ordering::SeqCst,
                                    );
                                    let _ = app.emit("hide-tray", ());
                                    let _ = window.hide();
                                    tauri::async_runtime::spawn(async {
                                        stop_mouse_hook();
                                    });
                                }
                            } else {
                                tauri::async_runtime::spawn(async {
                                    stop_mouse_hook();
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    
    CallNextHookEx(HOOK_HANDLE.load(Ordering::SeqCst) as HHOOK, code, w_param, l_param)
}

pub fn start_mouse_hook(app: AppHandle, hwnd: isize) {
    if IS_HOOKED.load(Ordering::SeqCst) {
        return;
    }
    
    *APP_HANDLE.lock().unwrap() = Some(app);
    TRAY_HWND.store(hwnd, Ordering::SeqCst);
    
    unsafe {
        let hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_proc),
            GetModuleHandleW(std::ptr::null()),
            0,
        );
        
        if !hook.is_null() {
            HOOK_HANDLE.store(hook as isize, Ordering::SeqCst);
            IS_HOOKED.store(true, Ordering::SeqCst);
            tracing::info!("Global mouse hook started successfully");
            
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if !IS_HOOKED.load(Ordering::SeqCst) {
                    break;
                }
            }
        } else {
            tracing::error!("Failed to install global mouse hook");
        }
    }
}

pub fn stop_mouse_hook() {
    if !IS_HOOKED.load(Ordering::SeqCst) {
        return;
    }
    
    unsafe {
        UnhookWindowsHookEx(HOOK_HANDLE.load(Ordering::SeqCst) as HHOOK);
        HOOK_HANDLE.store(0, Ordering::SeqCst);
        IS_HOOKED.store(false, Ordering::SeqCst);
        tracing::info!("Global mouse hook stopped");
    }
}

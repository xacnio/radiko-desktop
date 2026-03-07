use std::sync::atomic::{AtomicBool, Ordering, AtomicIsize};
use tauri::{AppHandle, Manager, Emitter};
use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::{HHOOK, HWND, RECT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_MBUTTONDOWN,
    MSLLHOOKSTRUCT, WindowFromPoint, GetWindowRect, PostThreadMessageW, WM_QUIT
};
use winapi::um::processthreadsapi::GetCurrentThreadId;
use once_cell::sync::Lazy;

use std::sync::atomic::AtomicU64;

pub static LAST_HIDE_TIME: AtomicU64 = AtomicU64::new(0);

static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static HOOK_THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
static IS_HOOKED: AtomicBool = AtomicBool::new(false);
static APP_HANDLE: Lazy<std::sync::Mutex<Option<AppHandle>>> = Lazy::new(|| std::sync::Mutex::new(None));

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0 {
        if w_param as u32 == WM_LBUTTONDOWN || w_param as u32 == WM_RBUTTONDOWN || w_param as u32 == WM_MBUTTONDOWN {
            
            let hook_struct = &*(l_param as *const MSLLHOOKSTRUCT);
            let hwnd = WindowFromPoint(hook_struct.pt);
            
            if !hwnd.is_null() {
                let tray_hwnd = TRAY_HWND.load(Ordering::SeqCst);
                if tray_hwnd != 0 {
                    let mut rect: RECT = std::mem::zeroed();
                    if unsafe { GetWindowRect(tray_hwnd as HWND, &mut rect) } != 0 {
                        let mx = hook_struct.pt.x;
                        let my = hook_struct.pt.y;
                        
                        let inside = mx >= rect.left && mx <= rect.right && my >= rect.top && my <= rect.bottom;
                        
                        if !inside {
                            LAST_HIDE_TIME.store(
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or(std::time::Duration::from_millis(0))
                                    .as_millis() as u64,
                                Ordering::SeqCst,
                            );
                            
                            if let Ok(handle) = APP_HANDLE.lock() {
                                if let Some(app) = handle.as_ref() {
                                    let app_clone = app.clone();
                                    tauri::async_runtime::spawn(async move {
                                        if let Some(window) = app_clone.get_webview_window("tray") {
                                            let _ = app_clone.emit("hide-tray", ());
                                            let _ = window.hide();
                                        }
                                        stop_mouse_hook();
                                    });
                                } else {
                                    tauri::async_runtime::spawn(async move {
                                        stop_mouse_hook();
                                    });
                                }
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
            HOOK_THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);
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
        
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            PostThreadMessageW(tid, WM_QUIT, 0, 0);
            HOOK_THREAD_ID.store(0, Ordering::SeqCst);
        }
        
        tracing::info!("Global mouse hook stopped");
    }
}

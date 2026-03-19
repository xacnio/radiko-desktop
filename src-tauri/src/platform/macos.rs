use objc::{class, msg_send, sel, sel_impl, runtime::Object};
use objc::declare::ClassDecl;
use objc::runtime::Class;
use std::sync::Once;
use tauri::Manager;

extern "C" fn window_will_miniaturize(this: &Object, _cmd: objc::runtime::Sel, _notification: *mut Object) {
    unsafe {
        let app_handle_ptr: usize = *this.get_ivar("appHandlePtr");
        if app_handle_ptr == 0 { return; }
        let app_handle = &*(app_handle_ptr as *const tauri::AppHandle);

        let minimize_to_tray = app_handle
            .try_state::<crate::state::AppState>()
            .map(|s| s.inner.lock().unwrap().minimize_to_tray)
            .unwrap_or(false);

        if minimize_to_tray {
            tracing::info!("windowWillMiniaturize: hiding to tray");
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    let _ = win.hide();
                }
            });
        }
    }
}

fn get_observer_class() -> &'static Class {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RadikoMiniaturizeObserver", superclass).unwrap();
        unsafe {
            decl.add_ivar::<usize>("appHandlePtr");
            decl.add_method(
                sel!(windowWillMiniaturize:),
                window_will_miniaturize as extern "C" fn(&Object, _, *mut Object),
            );
        }
        decl.register();
    });
    class!(RadikoMiniaturizeObserver)
}

unsafe fn ns_string(s: &str) -> *mut Object {
    let cls = class!(NSString);
    let obj: *mut Object = msg_send![cls, alloc];
    let bytes = s.as_ptr() as *const std::os::raw::c_void;
    msg_send![obj, initWithBytes:bytes length:s.len() encoding:4u64 /* NSUTF8StringEncoding */]
}

/// Registers a native macOS observer for NSWindowWillMiniaturizeNotification.
/// Zero-cost when idle — no polling, no timers.
pub fn register_miniaturize_observer(app_handle: &tauri::AppHandle) {
    let ns_window = match app_handle.get_webview_window("main") {
        Some(w) => match w.ns_window() {
            Ok(ptr) => ptr as *mut Object,
            Err(_) => return,
        },
        None => return,
    };

    let cls = get_observer_class();
    let handle_ptr = Box::into_raw(Box::new(app_handle.clone())) as usize;

    unsafe {
        let observer: *mut Object = msg_send![cls, new];
        (*observer).set_ivar("appHandlePtr", handle_ptr);

        let nc: *mut Object = msg_send![class!(NSNotificationCenter), defaultCenter];
        let name = ns_string("NSWindowWillMiniaturizeNotification");

        let _: () = msg_send![nc,
            addObserver: observer
            selector: sel!(windowWillMiniaturize:)
            name: name
            object: ns_window
        ];
    }
}

/// Registers a CoreAudio listener for default output device changes.
/// When the system default output device changes, restarts playback automatically.
pub fn register_default_device_listener(app_handle: tauri::AppHandle) {
    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        #[allow(dead_code)]
        fn AudioObjectAddPropertyListenerBlock(
            object: u32,
            address: *const [u32; 3],
            dispatch_queue: *mut std::ffi::c_void,
            listener: *mut std::ffi::c_void,
        ) -> i32;
    }

    const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
    const K_DEFAULT_OUTPUT: u32 = u32::from_be_bytes(*b"dOut");
    const ADDR: [u32; 3] = [K_DEFAULT_OUTPUT, u32::from_be_bytes(*b"glob"), 0];

    // Use a raw pointer to pass app_handle into the C callback via Box leak
    let handle_ptr = Box::into_raw(Box::new(app_handle)) as usize;

    // The block is a Rust closure converted to an Objective-C block via a trampoline.
    // We use a simple approach: spawn a thread that polls via a channel triggered by
    // AudioObjectAddPropertyListenerBlock using objc blocks.
    // Simpler: use a static AtomicBool + background thread approach.
    // Actually, use AudioObjectAddPropertyListener (non-block version) which is simpler.
    unsafe {
        // Use the non-block C function pointer version
        extern "C" fn device_changed(
            _object: u32,
            _num_addresses: u32,
            _addresses: *const [u32; 3],
            client_data: *mut std::ffi::c_void,
        ) -> i32 {
            let handle_ptr = client_data as usize;
            if handle_ptr == 0 { return 0; }
            let app_handle = unsafe { &*(handle_ptr as *const tauri::AppHandle) };
            tracing::info!("macOS: default output device changed, restarting playback");
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::restart_on_device_change(
                    handle.clone(),
                    handle.state::<crate::state::AppState>(),
                ).await;
            });
            0
        }

        #[link(name = "CoreAudio", kind = "framework")]
        extern "C" {
            fn AudioObjectAddPropertyListener(
                object: u32,
                address: *const [u32; 3],
                listener: extern "C" fn(u32, u32, *const [u32; 3], *mut std::ffi::c_void) -> i32,
                client_data: *mut std::ffi::c_void,
            ) -> i32;
        }

        AudioObjectAddPropertyListener(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &ADDR,
            device_changed,
            handle_ptr as *mut std::ffi::c_void,
        );
    }

    tracing::info!("macOS: default output device listener registered");
}

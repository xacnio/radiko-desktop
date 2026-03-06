//! Native splash screen — shown while the Tauri WebView is loading.
//! On Windows this creates a borderless popup via Win32; on other platforms it is a no-op.

#[cfg(target_os = "windows")]
pub use self::win32::SplashScreen;

#[cfg(not(target_os = "windows"))]
pub use self::stub::SplashScreen;

#[cfg(not(target_os = "windows"))]
mod stub {
    pub struct SplashScreen;
    impl SplashScreen {
        pub fn show() -> Option<Self> {
            None
        }
        pub fn close(&self) {}
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(clippy::upper_case_acronyms)]
    use std::sync::mpsc;

    type HWND = isize;
    type HINSTANCE = isize;
    type WPARAM = usize;
    type LPARAM = isize;
    type LRESULT = isize;
    type UINT = u32;
    type BOOL = i32;
    type HDC = isize;
    type HGDIOBJ = isize;
    type COLORREF = u32;

    #[repr(C)]
    struct WNDCLASSEXW {
        cb_size: u32,
        style: u32,
        lpfn_wnd_proc: Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: HINSTANCE,
        h_icon: isize,
        h_cursor: isize,
        hbr_background: isize,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: isize,
    }
    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: UINT,
        w_param: WPARAM,
        l_param: LPARAM,
        time: u32,
        pt_x: i32,
        pt_y: i32,
    }
    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct PAINTSTRUCT {
        hdc: HDC,
        f_erase: BOOL,
        rc_paint: RECT,
        _rest: [u8; 40],
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
        bi_x_pels: i32,
        bi_y_pels: i32,
        bi_clr_used: u32,
        bi_clr_important: u32,
    }

    const WS_POPUP: u32 = 0x8000_0000;
    const WM_PAINT: u32 = 0x000F;
    const WM_ERASEBKGND: u32 = 0x0014;
    const WM_DESTROY: u32 = 0x0002;
    const WM_CLOSE: u32 = 0x0010;
    const WM_TIMER: u32 = 0x0113;
    const WM_CREATE: u32 = 0x0001;
    const CS_HREDRAW: u32 = 2;
    const CS_VREDRAW: u32 = 1;
    const DT_CENTER: u32 = 1;
    const DT_VCENTER: u32 = 4;
    const DT_SINGLELINE: u32 = 0x20;
    const WS_EX_TOPMOST: u32 = 8;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const SRCCOPY: u32 = 0x00CC0020;
    #[allow(non_upper_case_globals)]
    const IDC_ARROW: *const u16 = 32512 as *const u16;

    extern "system" {
        fn RegisterClassExW(wc: *const WNDCLASSEXW) -> u16;
        fn CreateWindowExW(
            ex: u32,
            cls: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: HWND,
            menu: isize,
            inst: HINSTANCE,
            param: isize,
        ) -> HWND;
        fn ShowWindow(h: HWND, cmd: i32) -> BOOL;
        fn UpdateWindow(h: HWND) -> BOOL;
        fn GetMessageW(msg: *mut MSG, h: HWND, min: UINT, max: UINT) -> BOOL;
        fn TranslateMessage(msg: *const MSG) -> BOOL;
        fn DispatchMessageW(msg: *const MSG) -> LRESULT;
        fn DefWindowProcW(h: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT;
        fn PostQuitMessage(code: i32);
        fn GetSystemMetrics(idx: i32) -> i32;
        fn PostMessageW(h: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> BOOL;
        fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> HDC;
        fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> BOOL;
        fn FillRect(hdc: HDC, rc: *const RECT, br: isize) -> i32;
        fn CreateSolidBrush(color: COLORREF) -> isize;
        fn DeleteObject(obj: HGDIOBJ) -> BOOL;
        fn SetBkMode(hdc: HDC, mode: i32) -> i32;
        fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
        fn DrawTextW(hdc: HDC, txt: *const u16, len: i32, rc: *mut RECT, fmt: u32) -> i32;
        fn CreateFontW(
            h: i32,
            w: i32,
            esc: i32,
            ori: i32,
            weight: i32,
            italic: u32,
            underline: u32,
            strike: u32,
            charset: u32,
            out_prec: u32,
            clip: u32,
            quality: u32,
            pitch: u32,
            face: *const u16,
        ) -> isize;
        fn SelectObject(hdc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
        fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
        fn SetTimer(h: HWND, id: usize, ms: u32, func: isize) -> usize;
        fn KillTimer(h: HWND, id: usize) -> BOOL;
        fn InvalidateRect(h: HWND, rc: *const RECT, erase: BOOL) -> BOOL;
        fn GetClientRect(h: HWND, rc: *mut RECT) -> BOOL;
        fn CreateRoundRectRgn(x1: i32, y1: i32, x2: i32, y2: i32, cx: i32, cy: i32) -> isize;
        fn SetWindowRgn(h: HWND, rgn: isize, redraw: BOOL) -> i32;
        fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> isize;
        fn CreateCompatibleDC(hdc: HDC) -> HDC;
        fn CreateCompatibleBitmap(hdc: HDC, w: i32, h: i32) -> isize;
        fn BitBlt(
            dest: HDC,
            dx: i32,
            dy: i32,
            w: i32,
            h: i32,
            src: HDC,
            sx: i32,
            sy: i32,
            rop: u32,
        ) -> BOOL;
        fn DeleteDC(hdc: HDC) -> BOOL;
        fn GetDC(h: HWND) -> HDC;
        fn ReleaseDC(h: HWND, hdc: HDC) -> i32;
        fn CreateDIBSection(
            hdc: HDC,
            bmi: *const BITMAPINFOHEADER,
            usage: u32,
            bits: *mut *mut u8,
            section: isize,
            offset: u32,
        ) -> isize;
        fn StretchBlt(
            dest: HDC,
            dx: i32,
            dy: i32,
            dw: i32,
            dh: i32,
            src: HDC,
            sx: i32,
            sy: i32,
            sw: i32,
            sh: i32,
            rop: u32,
        ) -> BOOL;
        fn SetStretchBltMode(hdc: HDC, mode: i32) -> i32;
        fn GetUserDefaultUILanguage() -> u16;
        fn GetLocaleInfoW(locale: u32, lctype: u32, data: *mut u16, len: i32) -> i32;
    }

    fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
        r as u32 | (g as u32) << 8 | (b as u32) << 16
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    static mut TICK: u32 = 0;
    static mut ICON_DC: HDC = 0;
    static mut ICON_BMP: isize = 0;
    static mut ICON_W: i32 = 0;
    static mut ICON_H: i32 = 0;

    static LOCALES_DIR: include_dir::Dir =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/../src/locales");
    static LOADING_TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    /// Read "loading" text from the actual locale JSON files (embedded at compile time).
    fn get_loading_text() -> &'static str {
        LOADING_TEXT.get_or_init(|| {
            // Priority 1: Check language saved in settings.json by the app
            let mut lang_code = String::new();
            if let Ok(app_data) = std::env::var("APPDATA") {
                let settings_path = std::path::Path::new(&app_data)
                    .join("dev.xacnio.radikodesktop")
                    .join("settings.json");
                if let Ok(content) = std::fs::read_to_string(settings_path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(l) = json.get("language").and_then(|v| v.as_str()) {
                            lang_code = l.to_string();
                        }
                    }
                }
            }

            // Priority 2: Fallback to System language if no app language is set
            if lang_code.is_empty() {
                let lang_id = unsafe { GetUserDefaultUILanguage() } as u32;
                let mut buf = [0u16; 9];
                let res = unsafe { GetLocaleInfoW(lang_id, 0x0059, buf.as_mut_ptr(), 9) }; // LOCALE_SISO639LANGNAME
                if res > 0 {
                    if let Ok(s) = String::from_utf16(&buf[..(res - 1) as usize]) {
                        lang_code = s.to_lowercase();
                    }
                }
                if lang_code.is_empty() {
                    lang_code = "en".to_string();
                }
                println!("[SPLASH] System generic language detected: {}", lang_code);
            } else {
                println!("[SPLASH] Settings file language detected: {}", lang_code);
            }

            // Get translation from embedded JSONs dynamically
            let file_name = format!("{}.json", lang_code);
            if let Some(file) = LOCALES_DIR
                .get_file(&file_name)
                .or_else(|| LOCALES_DIR.get_file("en.json"))
            {
                if let Some(json_str) = file.contents_utf8() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(s) = v
                            .get("common")
                            .and_then(|c| c.get("loading"))
                            .and_then(|l| l.as_str())
                        {
                            return s.trim_end_matches('.').to_string();
                        }
                    }
                }
            }
            "Loading".to_string()
        })
    }

    const BG_R: u8 = 13;
    const BG_G: u8 = 13;
    const BG_B: u8 = 13;

    /// Decode the embedded PNG icon, pre-composite against the splash background,
    /// and create a GDI bitmap + DC ready for StretchBlt.
    unsafe fn load_icon_png() {
        let png_bytes = include_bytes!("../../icons/64x64.png");
        let img = match image::load_from_memory(png_bytes) {
            Ok(i) => i,
            Err(_) => return,
        };
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as i32, rgba.height() as i32);

        // Pre-composite against background colour and convert RGBA → BGRA
        let mut bgra: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
        for px in rgba.pixels() {
            let [r, g, b, a] = px.0;
            let af = a as f32 / 255.0;
            let inv = 1.0 - af;
            bgra.push((b as f32 * af + BG_B as f32 * inv) as u8); // B
            bgra.push((g as f32 * af + BG_G as f32 * inv) as u8); // G
            bgra.push((r as f32 * af + BG_R as f32 * inv) as u8); // R
            bgra.push(0xFF); // A (ignored)
        }

        let bmi = BITMAPINFOHEADER {
            bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            bi_width: w,
            bi_height: -h, // top-down
            bi_planes: 1,
            bi_bit_count: 32,
            bi_compression: 0,
            bi_size_image: 0,
            bi_x_pels: 0,
            bi_y_pels: 0,
            bi_clr_used: 0,
            bi_clr_important: 0,
        };

        let screen_dc = GetDC(0);
        let mut bits_ptr: *mut u8 = std::ptr::null_mut();
        let dib = CreateDIBSection(screen_dc, &bmi, 0, &mut bits_ptr as *mut *mut u8, 0, 0);
        if dib != 0 && !bits_ptr.is_null() {
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits_ptr, bgra.len());
            let mem = CreateCompatibleDC(screen_dc);
            SelectObject(mem, dib);
            ICON_DC = mem;
            ICON_BMP = dib;
            ICON_W = w;
            ICON_H = h;
        }
        ReleaseDC(0, screen_dc);
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => {
                SetTimer(hwnd, 1, 40, 0);
                0
            }
            WM_TIMER => {
                TICK = TICK.wrapping_add(1);
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                if ICON_DC != 0 {
                    DeleteDC(ICON_DC);
                    ICON_DC = 0;
                }
                if ICON_BMP != 0 {
                    DeleteObject(ICON_BMP);
                    ICON_BMP = 0;
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    unsafe fn paint(hwnd: HWND) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let real_dc = BeginPaint(hwnd, &mut ps);
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rc);

        // ── double-buffer ───────────────────────────────────────────────
        let mem_dc = CreateCompatibleDC(real_dc);
        let bmp = CreateCompatibleBitmap(real_dc, rc.right, rc.bottom);
        let old_bmp = SelectObject(mem_dc, bmp);
        let hdc = mem_dc;
        let cx = rc.right / 2;

        // splash border + background
        let br = CreateSolidBrush(rgb(42, 42, 42));
        FillRect(hdc, &rc, br);
        DeleteObject(br);
        let bg = RECT {
            left: 1,
            top: 1,
            right: rc.right - 1,
            bottom: rc.bottom - 1,
        };
        let br2 = CreateSolidBrush(rgb(BG_R, BG_G, BG_B));
        FillRect(hdc, &bg, br2);
        DeleteObject(br2);

        SetBkMode(hdc, 1);
        let face = wide("Segoe UI");

        // ── icon (40×40, centered) ──────────────────────────────────────
        let logo_size: i32 = 40;
        let logo_x = cx - logo_size / 2;
        let logo_y: i32 = 16;

        if ICON_DC != 0 {
            SetStretchBltMode(hdc, 4); // HALFTONE
            StretchBlt(
                hdc, logo_x, logo_y, logo_size, logo_size, ICON_DC, 0, 0, ICON_W, ICON_H, SRCCOPY,
            );
        }

        // ── title ───────────────────────────────────────────────────────
        let fnt = CreateFontW(-18, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
        let old_fnt = SelectObject(hdc, fnt);
        SetTextColor(hdc, rgb(240, 240, 240));
        let title = wide("Radiko Desktop");
        let mut tr = RECT {
            left: 0,
            top: 62,
            right: rc.right,
            bottom: 86,
        };
        DrawTextW(
            hdc,
            title.as_ptr(),
            -1,
            &mut tr,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // ── subtitle ────────────────────────────────────────────────────
        let dots = match (TICK / 10) % 4 {
            0 => "",
            1 => ".",
            2 => "..",
            _ => "...",
        };
        let sub = wide(&format!("{}{}", get_loading_text(), dots));
        let fnt2 = CreateFontW(-12, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
        SelectObject(hdc, fnt2);
        SetTextColor(hdc, rgb(110, 110, 110));
        let mut sr = RECT {
            left: 0,
            top: 90,
            right: rc.right,
            bottom: 108,
        };
        DrawTextW(
            hdc,
            sub.as_ptr(),
            -1,
            &mut sr,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // ── progress bar ────────────────────────────────────────────────
        let tw = 200;
        let th = 3;
        let tl = cx - tw / 2;
        let ty = 120;
        let track = RECT {
            left: tl,
            top: ty,
            right: tl + tw,
            bottom: ty + th,
        };
        let tbr = CreateSolidBrush(rgb(30, 30, 30));
        FillRect(hdc, &track, tbr);
        DeleteObject(tbr);

        let iw = 60;
        let max_pos = (tw - iw) as f32;
        let t = (TICK % 100) as f32 / 100.0;
        let lin = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
        let eased = lin * lin * (3.0 - 2.0 * lin);
        let ix = tl + (eased * max_pos) as i32;
        let ind = RECT {
            left: ix,
            top: ty,
            right: ix + iw,
            bottom: ty + th,
        };
        let ibr = CreateSolidBrush(rgb(16, 185, 129));
        FillRect(hdc, &ind, ibr);
        DeleteObject(ibr);

        // ── blit to screen ──────────────────────────────────────────────
        SelectObject(hdc, old_fnt);
        DeleteObject(fnt);
        DeleteObject(fnt2);
        BitBlt(real_dc, 0, 0, rc.right, rc.bottom, mem_dc, 0, 0, SRCCOPY);
        SelectObject(mem_dc, old_bmp);
        DeleteObject(bmp);
        DeleteDC(mem_dc);
        EndPaint(hwnd, &ps);
    }

    pub struct SplashScreen {
        hwnd: HWND,
    }
    unsafe impl Send for SplashScreen {}
    unsafe impl Sync for SplashScreen {}

    impl SplashScreen {
        pub fn show() -> Option<Self> {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || unsafe {
                let hi = GetModuleHandleW(std::ptr::null());

                // Load icon PNG from embedded bytes
                load_icon_png();

                // Warm up loading text (reads from locale JSONs)
                let _ = get_loading_text();

                let cls = wide("RadikoSplash");
                let arrow = LoadCursorW(0, IDC_ARROW);
                let wc = WNDCLASSEXW {
                    cb_size: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfn_wnd_proc: Some(wnd_proc),
                    cb_cls_extra: 0,
                    cb_wnd_extra: 0,
                    h_instance: hi,
                    h_icon: 0,
                    h_cursor: arrow,
                    hbr_background: 0,
                    lpsz_menu_name: std::ptr::null(),
                    lpsz_class_name: cls.as_ptr(),
                    h_icon_sm: 0,
                };
                RegisterClassExW(&wc);

                let (w, h) = (340, 148);
                let sx = GetSystemMetrics(0);
                let sy = GetSystemMetrics(1);
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                    cls.as_ptr(),
                    std::ptr::null(),
                    WS_POPUP,
                    (sx - w) / 2,
                    (sy - h) / 2,
                    w,
                    h,
                    0,
                    0,
                    hi,
                    0,
                );
                if hwnd == 0 {
                    let _ = tx.send(0);
                    return;
                }

                let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, 14, 14);
                SetWindowRgn(hwnd, rgn, 0);
                ShowWindow(hwnd, 5);
                UpdateWindow(hwnd);
                let _ = tx.send(hwnd);

                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            });
            rx.recv()
                .ok()
                .filter(|&h| h != 0)
                .map(|hwnd| SplashScreen { hwnd })
        }

        pub fn close(&self) {
            unsafe {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            }
        }
    }
}

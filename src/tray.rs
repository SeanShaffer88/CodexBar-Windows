//! System tray app (Windows): a notification icon whose tooltip and popup
//! menu show the latest provider numbers. A timer refreshes on the
//! configured interval; the actual collection runs on a worker thread so
//! the message loop never blocks. Non-Windows builds return an error.

#[cfg(not(windows))]
pub fn run() -> i32 {
    eprintln!("The tray app only runs on Windows. Use `status`, `refresh`, and `history` here.");
    1
}

#[cfg(windows)]
pub fn run() -> i32 {
    imp::run()
}

#[cfg(windows)]
pub(crate) mod imp {
    use crate::model::ProviderReport;
    use crate::{config, ops, timeutil};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::Console::FreeConsole;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const WM_TRAY: u32 = WM_APP + 2;
    const WM_READY: u32 = WM_APP + 1;
    pub(crate) const WM_SETTINGS_SAVED: u32 = WM_APP + 7;
    const WM_OPEN_DETAILS: u32 = WM_APP + 8;
    const TIMER_ID: usize = 1;
    const ID_SETTINGS: usize = 1005;
    const ID_DETAILS: usize = 1006;
    const ID_REFRESH: usize = 1001;
    const ID_FOLDER: usize = 1002;
    const ID_HELP: usize = 1003;
    const ID_QUIT: usize = 1004;

    static REPORTS: Mutex<Vec<ProviderReport>> = Mutex::new(Vec::new());
    static LAST_ICON: Mutex<usize> = Mutex::new(0);
    static PREV_WORST: Mutex<Option<f64>> = Mutex::new(None);
    static DATA_DIR: Mutex<PathBuf> = Mutex::new(PathBuf::new());
    static REFRESH_MIN: Mutex<u32> = Mutex::new(15);
    static WARN_PCT: Mutex<u32> = Mutex::new(90);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn run() -> i32 {
        unsafe {
            // Single instance: a second launch asks the running tray
            // to show the usage detail window (v0.8.0) instead of just
            // saying the program is already running.
            let mutex = CreateMutexW(std::ptr::null(), 1, wide("CodexBarWinMutex").as_ptr());
            if mutex.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
                let existing = FindWindowExW(
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    wide("CodexBarWinTray").as_ptr(),
                    std::ptr::null(),
                );
                if !existing.is_null() {
                    PostMessageW(existing, WM_OPEN_DETAILS, 0, 0);
                } else {
                    MessageBoxW(
                        std::ptr::null_mut(),
                        wide("CodexBar-Windows is already running in the tray.").as_ptr(),
                        wide("CodexBar-Windows").as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                return 0;
            }
            FreeConsole(); // tray mode: no console window lingers

            let dir = config::data_dir_env();
            let (cfg, _) = config::load(&dir);
            *DATA_DIR.lock().unwrap() = dir.clone();
            *REFRESH_MIN.lock().unwrap() = cfg.refresh_minutes;
            *WARN_PCT.lock().unwrap() = cfg.warn_threshold_percent;

            let hinst = GetModuleHandleW(std::ptr::null());
            let class_name = wide("CodexBarWinTray");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinst,
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&wc);
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                wide("CodexBar-Windows").as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                hinst,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return 1;
            }
            add_icon(hwnd, None);
            SetTimer(hwnd, TIMER_ID, 30_000, None); // first refresh in 30 s
            spawn_refresh(hwnd);
            // The usage detail window opens at launch (owner's call,
            // 2026-11-10); posted so the tray loop is running first.
            PostMessageW(hwnd, WM_OPEN_DETAILS, 0, 0);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            0
        }
    }

    fn spawn_refresh(hwnd: HWND) {
        let hwnd_addr = hwnd as usize;
        std::thread::spawn(move || {
            let hwnd = hwnd_addr as HWND;
            let dir = DATA_DIR.lock().unwrap().clone();
            let (cfg, _) = config::load(&dir);
            let reports = ops::refresh(&cfg, &dir, timeutil::now_unix());
            if let Ok(mut slot) = REPORTS.lock() {
                *slot = reports;
            }
            unsafe { PostMessageW(hwnd, WM_READY, 0, 0) };
        });
    }

    fn worst_percent() -> Option<f64> {
        REPORTS
            .lock()
            .ok()?
            .iter()
            .filter(|r| r.ok)
            .filter_map(|r| r.worst_percent())
            .fold(None::<f64>, |a, p| Some(a.map_or(p, |x| x.max(p))))
    }

    fn tooltip_text() -> String {
        let reports = REPORTS.lock().unwrap();
        if reports.is_empty() {
            return "CodexBar-Windows — collecting…".to_string();
        }
        let parts: Vec<String> = reports
            .iter()
            .map(|r| match r.worst_percent() {
                Some(p) if r.ok => format!("{} {p:.0}%", r.name),
                _ => format!("{} —", r.name),
            })
            .collect();
        let s = format!("CodexBar — {}", parts.join(" · "));
        s.chars().take(120).collect()
    }

    /// Draw the tray icon at runtime: the worst window's percent as a
    /// number (v0.9.0, like the original's menu-bar text) in the band
    /// color; with no percent data, a dark square with a usage bar.
    unsafe fn make_icon(percent: Option<f64>) -> HICON {
        if let Some((text, color)) = crate::model::tray_badge(percent) {
            let icon = make_number_icon(&text, color);
            if !icon.is_null() {
                return icon;
            }
        }
        make_bar_icon(percent)
    }

    /// The percent as bold text on the dark square, drawn with GDI into
    /// a 32-bit DIB (alpha forced opaque afterwards — GDI does not
    /// write the alpha channel).
    unsafe fn make_number_icon(text: &str, color: [u8; 3]) -> HICON {
        const S: i32 = 32;
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = S;
        bmi.bmiHeader.biHeight = -S; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return std::ptr::null_mut();
        }
        let dib = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(dc);
            return std::ptr::null_mut();
        }
        let old_bmp = SelectObject(dc, dib);
        let bg = CreateSolidBrush(24 | (24 << 8) | (28 << 16));
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: S,
            bottom: S,
        };
        FillRect(dc, &rc, bg);
        DeleteObject(bg);
        let height = if text.len() >= 3 { -15 } else { -21 };
        let font = CreateFontW(
            height,
            0,
            0,
            0,
            700,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, 1); // TRANSPARENT
        SetTextColor(
            dc,
            (color[0] as u32) | ((color[1] as u32) << 8) | ((color[2] as u32) << 16),
        );
        DrawTextW(
            dc,
            wide(text).as_ptr(),
            -1,
            &mut rc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(dc, old_font);
        DeleteObject(font);
        SelectObject(dc, old_bmp);
        // Force every pixel opaque; the backdrop is the whole square.
        let px = std::slice::from_raw_parts_mut(bits as *mut u8, (S * S * 4) as usize);
        for i in (0..px.len()).step_by(4) {
            px[i + 3] = 255;
        }
        DeleteDC(dc);
        let mask_bits = vec![0u8; (S * S / 8) as usize];
        let mask = CreateBitmap(S, S, 1, 1, mask_bits.as_ptr() as *const _);
        let info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: dib,
        };
        let icon = CreateIconIndirect(&info);
        DeleteObject(dib);
        DeleteObject(mask);
        icon
    }

    /// The original bar glyph: fill width and color follow the worst
    /// provider window. Kept for the no-percent case.
    unsafe fn make_bar_icon(percent: Option<f64>) -> HICON {
        const S: i32 = 32;
        let mut px = vec![0u8; (S * S * 4) as usize];
        let (fr, fg, fb) = match percent {
            None => (120u8, 120u8, 120u8),
            Some(p) if p >= 90.0 => (230u8, 60u8, 50u8),
            Some(p) if p >= 70.0 => (240u8, 170u8, 40u8),
            Some(_) => (60u8, 200u8, 110u8),
        };
        let fill_w = match percent {
            Some(p) => ((S - 8) as f64 * (p.clamp(0.0, 100.0) / 100.0)) as i32,
            None => 0,
        };
        for y in 0..S {
            for x in 0..S {
                let i = ((y * S + x) * 4) as usize;
                // Bitmap rows are BGRA; bottom-up is fine for a symmetric glyph.
                let (r, g, b) = if (4..S - 4).contains(&y) && (4..S - 4).contains(&x) {
                    if x - 4 < fill_w {
                        (fr, fg, fb)
                    } else {
                        (70, 70, 76)
                    }
                } else {
                    (24, 24, 28)
                };
                px[i] = b;
                px[i + 1] = g;
                px[i + 2] = r;
                px[i + 3] = 255;
            }
        }
        let color = CreateBitmap(S, S, 1, 32, px.as_ptr() as *const _);
        let mask_bits = vec![0u8; (S * S / 8) as usize];
        let mask = CreateBitmap(S, S, 1, 1, mask_bits.as_ptr() as *const _);
        let info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }

    unsafe fn add_icon(hwnd: HWND, percent: Option<f64>) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = make_icon(percent);
        set_tip(&mut nid);
        Shell_NotifyIconW(NIM_ADD, &nid);
        *LAST_ICON.lock().unwrap() = nid.hIcon as usize;
    }

    unsafe fn update_icon(hwnd: HWND) {
        let icon = make_icon(worst_percent());
        if icon.is_null() {
            return;
        }
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = icon;
        set_tip(&mut nid);
        Shell_NotifyIconW(NIM_MODIFY, &nid);
        // The shell copies the icon, so the previous handle is ours
        // to release (the old code kept every refresh's icon alive).
        let mut last = LAST_ICON.lock().unwrap();
        if *last != 0 && *last != icon as usize {
            DestroyIcon(*last as HICON);
        }
        *last = icon as usize;
    }

    unsafe fn set_tip(nid: &mut NOTIFYICONDATAW) {
        let tip = tooltip_text();
        for (i, ch) in tip.encode_utf16().take(127).enumerate() {
            nid.szTip[i] = ch;
        }
    }

    unsafe fn show_balloon(hwnd: HWND, title: &str, text: &str) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_WARNING;
        for (i, ch) in text.encode_utf16().take(255).enumerate() {
            nid.szInfo[i] = ch;
        }
        for (i, ch) in title.encode_utf16().take(63).enumerate() {
            nid.szInfoTitle[i] = ch;
        }
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }

    unsafe fn remove_icon(hwnd: HWND) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_TIMER => {
                KillTimer(hwnd, TIMER_ID);
                let mins = *REFRESH_MIN.lock().unwrap();
                SetTimer(hwnd, TIMER_ID, mins * 60_000, None);
                spawn_refresh(hwnd);
                0
            }
            WM_READY => {
                update_icon(hwnd);
                // Notify once when the busiest window crosses 90%.
                let worst = {
                    let reports = REPORTS.lock().unwrap();
                    crate::ops::worst_window(&reports)
                };
                if let Some((name, label, pct)) = worst {
                    let mut prev = PREV_WORST.lock().unwrap();
                    let threshold = *WARN_PCT.lock().unwrap() as f64;
                    if crate::ops::crossed_threshold(*prev, Some(pct), threshold) {
                        show_balloon(
                            hwnd,
                            "CodexBar-Windows — limit warning",
                            &format!("{name} {label} is {pct:.0}% used."),
                        );
                    }
                    *prev = Some(pct);
                }
                0
            }
            WM_SETTINGS_SAVED => {
                let dir = DATA_DIR.lock().unwrap().clone();
                let (cfg, _) = config::load(&dir);
                *REFRESH_MIN.lock().unwrap() = cfg.refresh_minutes;
                *WARN_PCT.lock().unwrap() = cfg.warn_threshold_percent;
                KillTimer(hwnd, TIMER_ID);
                SetTimer(hwnd, TIMER_ID, cfg.refresh_minutes * 60_000, None);
                spawn_refresh(hwnd);
                0
            }
            WM_OPEN_DETAILS => {
                crate::detail::open(Some(hwnd));
                0
            }
            WM_TRAY => {
                if lp as u32 == WM_LBUTTONDBLCLK {
                    crate::settings::open(Some(hwnd));
                } else if lp as u32 == WM_LBUTTONUP || lp as u32 == WM_RBUTTONUP {
                    show_menu(hwnd);
                }
                0
            }
            WM_DESTROY => {
                remove_icon(hwnd);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    unsafe fn show_menu(hwnd: HWND) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let now = timeutil::now_unix();
        let lines: Vec<String> = {
            let reports = REPORTS.lock().unwrap();
            if reports.is_empty() {
                vec!["Collecting…".to_string()]
            } else {
                let mut v = Vec::new();
                for r in reports.iter() {
                    v.push(format!("— {} —", r.name));
                    if r.ok || !r.windows.is_empty() {
                        for w in &r.windows {
                            v.push(format!("   {}", w.describe(now)));
                        }
                    }
                    if let Some(e) = &r.error {
                        v.push(format!("   {e}"));
                    }
                }
                v
            }
        };
        for line in &lines {
            AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, wide(line).as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_REFRESH, wide("Refresh now").as_ptr());
        AppendMenuW(menu, MF_STRING, ID_DETAILS, wide("Details…").as_ptr());
        AppendMenuW(menu, MF_STRING, ID_SETTINGS, wide("Settings…").as_ptr());
        AppendMenuW(
            menu,
            MF_STRING,
            ID_FOLDER,
            wide("Open data folder").as_ptr(),
        );
        AppendMenuW(menu, MF_STRING, ID_HELP, wide("Help").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_QUIT, wide("Quit").as_ptr());

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        match cmd as usize {
            ID_DETAILS => {
                crate::detail::open(Some(hwnd));
            }
            ID_SETTINGS => {
                crate::settings::open(Some(hwnd));
            }
            ID_REFRESH => spawn_refresh(hwnd),
            ID_FOLDER => {
                let dir = DATA_DIR.lock().unwrap().clone();
                let _ = std::fs::create_dir_all(&dir);
                ShellExecuteW(
                    hwnd,
                    wide("open").as_ptr(),
                    wide(&dir.to_string_lossy()).as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    SW_SHOWNORMAL,
                );
            }
            ID_HELP => {
                MessageBoxW(
                    hwnd,
                    wide(crate::cli::HELP).as_ptr(),
                    wide("CodexBar-Windows — Help").as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            ID_QUIT => {
                crate::detail::close();
                DestroyWindow(hwnd);
            }
            _ => {}
        }
    }
}

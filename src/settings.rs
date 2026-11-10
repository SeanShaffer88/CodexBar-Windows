//! Settings window (Windows): a dark, single-page configuration window —
//! provider switches, API-key management, refresh interval, warning
//! threshold, start-with-Windows, and data-folder actions. Everything it
//! changes goes through the same operations as the `settings` command, so
//! the window and the command line never disagree. Off Windows, opening
//! the window is an honest error.

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn open(_tray: Option<*mut std::ffi::c_void>) -> i32 {
    eprintln!("The settings window only runs on Windows. Use `settings --refresh/--warn/--autostart` here.");
    1
}

#[cfg(windows)]
pub fn open(tray: Option<windows_sys::Win32::Foundation::HWND>) -> i32 {
    imp::open(tray)
}

#[cfg(windows)]
mod imp {
    use crate::{config, ops, secrets, store};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::SystemServices::SS_ENDELLIPSIS;
    use windows_sys::Win32::UI::Controls::*;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_F1};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    // Colors (mockup B: charcoal + crimson).
    const BG: u32 = rgb(32, 32, 36);
    const EDIT_BG: u32 = rgb(58, 58, 64);
    const INK: u32 = rgb(235, 235, 235);
    const DIM: u32 = rgb(160, 160, 168);
    const RED: u32 = rgb(200, 40, 55);
    const RED_TEXT: u32 = rgb(235, 92, 108);
    const BTN: u32 = rgb(58, 58, 64);
    const BTN_PRESS: u32 = rgb(44, 44, 50);
    const EDGE: u32 = rgb(90, 90, 100);

    const fn rgb(r: u32, g: u32, b: u32) -> u32 {
        r | (g << 8) | (b << 16)
    }

    const WM_KEY_CHANGED: u32 = WM_APP + 8;

    const ID_SAVE: usize = 1; // IDOK: Enter saves
    const ID_CANCEL: usize = 2; // IDCANCEL: Esc cancels
    const ID_CHECK0: usize = 100; // five provider checkboxes, 100..104
    const ID_KEYBTN0: usize = 300; // three key buttons, 300..302
    const ID_REFRESH_EDIT: usize = 400;
    const ID_WARN_EDIT: usize = 401;
    const ID_AUTOSTART: usize = 402;
    const ID_OPEN_DATA: usize = 404;
    const ID_CLEAR_HISTORY: usize = 405;
    const ID_FEEDBACK: usize = 500;
    const ID_HDR_PROVIDERS: usize = 900;
    const ID_HDR_GENERAL: usize = 901;

    const PROVIDERS: [(&str, &str); 7] = [
        ("codex", "Codex"),
        ("claude-code", "Claude Code"),
        ("openrouter", "OpenRouter"),
        ("deepseek", "DeepSeek"),
        ("moonshot", "Moonshot"),
        ("openai", "OpenAI API"),
        ("anthropic", "Anthropic API"),
    ];
    const KEY_IDS: [&str; 5] = ["openrouter", "deepseek", "moonshot", "openai", "anthropic"];

    static OPEN: AtomicBool = AtomicBool::new(false);
    static DONE: AtomicBool = AtomicBool::new(false);
    static WIN: Mutex<usize> = Mutex::new(0);

    struct Theme {
        font: HFONT,
        bold: HFONT,
        bg: HBRUSH,
        edit: HBRUSH,
    }
    // Raw handles shared across the two window procedures on the UI thread.
    unsafe impl Send for Theme {}
    static THEME: Mutex<Option<Theme>> = Mutex::new(None);

    fn theme() -> (HFONT, HFONT, HBRUSH, HBRUSH) {
        let mut slot = THEME.lock().unwrap();
        if slot.is_none() {
            unsafe {
                let font = CreateFontW(
                    -13,
                    0,
                    0,
                    0,
                    400,
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
                let bold = CreateFontW(
                    -13,
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
                *slot = Some(Theme {
                    font,
                    bold,
                    bg: CreateSolidBrush(BG),
                    edit: CreateSolidBrush(EDIT_BG),
                });
            }
        }
        let t = slot.as_ref().unwrap();
        (t.font, t.bold, t.bg, t.edit)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn get_text(hwnd: HWND) -> String {
        unsafe {
            let n = GetWindowTextLengthW(hwnd);
            if n <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; (n + 1) as usize];
            let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            String::from_utf16_lossy(&buf[..got.max(0) as usize])
        }
    }

    fn set_text(hwnd: HWND, s: &str) {
        unsafe { SetWindowTextW(hwnd, wide(s).as_ptr()) };
    }

    fn checked(hwnd: HWND) -> bool {
        unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as LRESULT }
    }

    fn set_check(hwnd: HWND, on: bool) {
        let v: WPARAM = if on {
            BST_CHECKED as WPARAM
        } else {
            BST_UNCHECKED as WPARAM
        };
        unsafe { SendMessageW(hwnd, BM_SETCHECK, v, 0) };
    }

    fn msg_box(parent: HWND, text: &str, flags: u32) {
        unsafe {
            MessageBoxW(
                parent,
                wide(text).as_ptr(),
                wide("CodexBar-Windows — Settings").as_ptr(),
                flags,
            )
        };
    }

    struct State {
        dir: PathBuf,
        tray: Option<HWND>,
        checks: [HWND; 7],
        key_status: [HWND; 5],
        refresh_edit: HWND,
        warn_edit: HWND,
        autostart: HWND,
        feedback: HWND,
    }

    struct Init {
        dir: PathBuf,
        tray: Option<HWND>,
    }

    pub fn open(tray: Option<HWND>) -> i32 {
        unsafe {
            if OPEN.swap(true, Ordering::SeqCst) {
                let w = *WIN.lock().unwrap();
                if w != 0 {
                    SetForegroundWindow(w as HWND);
                }
                return 0;
            }
            theme();
            let hinst = GetModuleHandleW(std::ptr::null());
            let classes: [(&str, WNDPROC); 2] = [
                ("CodexBarWinSettings", Some(wnd_proc)),
                ("CodexBarWinKeyDlg", Some(key_proc)),
            ];
            for (name, proc) in classes {
                let class_name = wide(name);
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: proc,
                    hInstance: hinst,
                    hbrBackground: theme().2,
                    lpszClassName: class_name.as_ptr(),
                    ..std::mem::zeroed()
                };
                RegisterClassExW(&wc); // already-registered is fine
            }
            let init = Box::new(Init {
                dir: config::data_dir_env(),
                tray,
            });
            let class = wide("CodexBarWinSettings");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                wide("CodexBar-Windows — Settings").as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                676,
                640,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinst,
                Box::into_raw(init) as *const _,
            );
            if hwnd.is_null() {
                OPEN.store(false, Ordering::SeqCst);
                return 1;
            }
            // Center on screen.
            let mut rc: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rc);
            let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
            let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
            let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;
            SetWindowPos(hwnd, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE);
            *WIN.lock().unwrap() = hwnd as usize;
            DONE.store(false, Ordering::SeqCst);

            // Modal-style loop: pump until the window closes.
            let mut msg: MSG = std::mem::zeroed();
            loop {
                let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if r <= 0 {
                    if tray.is_some() {
                        PostQuitMessage(0); // preserve a pending quit for the tray loop
                    }
                    break;
                }
                if msg.message == WM_KEYDOWN && msg.wParam == VK_F1 as WPARAM {
                    msg_box(hwnd, crate::cli::HELP, MB_OK | MB_ICONINFORMATION);
                } else if IsDialogMessageW(hwnd, &msg) == 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if DONE.load(Ordering::SeqCst) {
                    break;
                }
            }
            *WIN.lock().unwrap() = 0;
            OPEN.store(false, Ordering::SeqCst);
            0
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn ctl(
        parent: HWND,
        class: &str,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        id: usize,
        bold: bool,
        s: f32,
    ) -> HWND {
        let hinst = GetModuleHandleW(std::ptr::null());
        let hwnd = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            (x as f32 * s) as i32,
            (y as f32 * s) as i32,
            (w as f32 * s) as i32,
            (h as f32 * s) as i32,
            parent,
            id as HMENU,
            hinst,
            std::ptr::null(),
        );
        let (font, bold_font, _, _) = theme();
        SendMessageW(
            hwnd,
            WM_SETFONT,
            (if bold { bold_font } else { font }) as WPARAM,
            1,
        );
        hwnd
    }

    unsafe fn state<'a>(hwnd: HWND) -> &'a mut State {
        &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State)
    }

    fn provider_on(cfg: &config::Config, idx: usize) -> bool {
        match idx {
            0 => cfg.providers.codex,
            1 => cfg.providers.claude_code,
            2 => cfg.providers.openrouter,
            3 => cfg.providers.deepseek,
            4 => cfg.providers.moonshot,
            5 => cfg.providers.openai,
            _ => cfg.providers.anthropic,
        }
    }

    fn refresh_key_rows(hwnd: HWND, st: &State) {
        let (cfg, _) = config::load(&st.dir);
        for (i, pid) in KEY_IDS.iter().enumerate() {
            let has = secrets::get_key(&st.dir, pid).is_some();
            set_text(
                st.key_status[i],
                if has {
                    "Key stored (encrypted)"
                } else {
                    "No key stored"
                },
            );
            let btn = unsafe { GetDlgItem(hwnd, (ID_KEYBTN0 + i) as i32) };
            set_text(btn, if has { "Change…" } else { "Add key…" });
            set_check(st.checks[i + 2], provider_on(&cfg, i + 2));
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => {
                let cs = &*(lp as *const CREATESTRUCTW);
                let init = Box::from_raw(cs.lpCreateParams as *mut Init);
                let dc = GetDC(hwnd);
                let dpi = GetDeviceCaps(dc, 88) as f32; // LOGPIXELSX
                ReleaseDC(hwnd, dc);
                let s = if dpi > 0.0 { dpi / 96.0 } else { 1.0 };
                let (cfg, _) = config::load(&init.dir);
                let mut st = Box::new(State {
                    dir: init.dir.clone(),
                    tray: init.tray,
                    checks: [std::ptr::null_mut(); 7],
                    key_status: [std::ptr::null_mut(); 5],
                    refresh_edit: std::ptr::null_mut(),
                    warn_edit: std::ptr::null_mut(),
                    autostart: std::ptr::null_mut(),
                    feedback: std::ptr::null_mut(),
                });
                // Providers panel
                ctl(
                    hwnd,
                    "BUTTON",
                    "",
                    BS_GROUPBOX as u32,
                    14,
                    14,
                    632,
                    352,
                    0,
                    false,
                    s,
                );
                ctl(
                    hwnd,
                    "STATIC",
                    "PROVIDERS",
                    0,
                    28,
                    5,
                    130,
                    18,
                    ID_HDR_PROVIDERS,
                    true,
                    s,
                );
                for (i, (_, pname)) in PROVIDERS.iter().enumerate() {
                    let y = 42 + (i as i32) * 44;
                    st.checks[i] = ctl(
                        hwnd,
                        "BUTTON",
                        pname,
                        (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                        28,
                        y,
                        200,
                        22,
                        ID_CHECK0 + i,
                        true,
                        s,
                    );
                    set_check(st.checks[i], provider_on(&cfg, i));
                    if i < 2 {
                        ctl(
                            hwnd,
                            "STATIC",
                            "Local logs — no key needed",
                            0,
                            250,
                            y + 2,
                            220,
                            20,
                            200 + i,
                            false,
                            s,
                        );
                    }
                }
                for i in 0..5 {
                    let y = 42 + ((i + 2) as i32) * 44;
                    st.key_status[i] = ctl(
                        hwnd,
                        "STATIC",
                        "",
                        0,
                        250,
                        y + 2,
                        190,
                        20,
                        202 + i,
                        false,
                        s,
                    );
                    ctl(
                        hwnd,
                        "BUTTON",
                        "",
                        (BS_OWNERDRAW as u32) | WS_TABSTOP,
                        516,
                        y - 2,
                        100,
                        26,
                        ID_KEYBTN0 + i,
                        false,
                        s,
                    );
                }
                ctl(
                    hwnd,
                    "STATIC",
                    "Keys are encrypted with Windows DPAPI and never shown after saving.",
                    0,
                    28,
                    336,
                    590,
                    16,
                    902,
                    false,
                    s,
                );
                // General panel
                ctl(
                    hwnd,
                    "BUTTON",
                    "",
                    BS_GROUPBOX as u32,
                    14,
                    380,
                    632,
                    150,
                    0,
                    false,
                    s,
                );
                ctl(
                    hwnd,
                    "STATIC",
                    "GENERAL",
                    0,
                    28,
                    371,
                    130,
                    18,
                    ID_HDR_GENERAL,
                    true,
                    s,
                );
                ctl(
                    hwnd,
                    "STATIC",
                    "Refresh every",
                    0,
                    28,
                    406,
                    110,
                    20,
                    0,
                    false,
                    s,
                );
                st.refresh_edit = ctl(
                    hwnd,
                    "EDIT",
                    &cfg.refresh_minutes.to_string(),
                    WS_BORDER | (ES_NUMBER as u32) | WS_TABSTOP,
                    142,
                    403,
                    56,
                    24,
                    ID_REFRESH_EDIT,
                    false,
                    s,
                );
                ctl(hwnd, "STATIC", "minutes", 0, 204, 406, 60, 20, 0, false, s);
                ctl(hwnd, "STATIC", "Warn at", 0, 300, 406, 60, 20, 0, false, s);
                st.warn_edit = ctl(
                    hwnd,
                    "EDIT",
                    &cfg.warn_threshold_percent.to_string(),
                    WS_BORDER | (ES_NUMBER as u32) | WS_TABSTOP,
                    362,
                    403,
                    56,
                    24,
                    ID_WARN_EDIT,
                    false,
                    s,
                );
                ctl(hwnd, "STATIC", "% used", 0, 424, 406, 60, 20, 0, false, s);
                st.autostart = ctl(
                    hwnd,
                    "BUTTON",
                    "Start with Windows",
                    (BS_AUTOCHECKBOX as u32) | WS_TABSTOP,
                    28,
                    444,
                    220,
                    22,
                    ID_AUTOSTART,
                    false,
                    s,
                );
                set_check(st.autostart, ops::autostart_enabled());
                ctl(
                    hwnd,
                    "STATIC",
                    "Data folder:",
                    0,
                    300,
                    446,
                    80,
                    20,
                    0,
                    false,
                    s,
                );
                ctl(
                    hwnd,
                    "STATIC",
                    &st.dir.to_string_lossy(),
                    SS_ENDELLIPSIS,
                    384,
                    446,
                    232,
                    20,
                    0,
                    false,
                    s,
                );
                ctl(
                    hwnd,
                    "BUTTON",
                    "Open data",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    28,
                    482,
                    100,
                    28,
                    ID_OPEN_DATA,
                    false,
                    s,
                );
                ctl(
                    hwnd,
                    "BUTTON",
                    "Clear history",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    138,
                    482,
                    110,
                    28,
                    ID_CLEAR_HISTORY,
                    false,
                    s,
                );
                st.feedback = ctl(
                    hwnd,
                    "STATIC",
                    "",
                    0,
                    260,
                    486,
                    356,
                    20,
                    ID_FEEDBACK,
                    false,
                    s,
                );
                // Save / Cancel
                ctl(
                    hwnd,
                    "BUTTON",
                    "Save",
                    (BS_OWNERDRAW as u32) | (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP,
                    432,
                    546,
                    100,
                    32,
                    ID_SAVE,
                    true,
                    s,
                );
                ctl(
                    hwnd,
                    "BUTTON",
                    "Cancel",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    542,
                    546,
                    100,
                    32,
                    ID_CANCEL,
                    false,
                    s,
                );
                refresh_key_rows(hwnd, &st);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(st) as isize);
                0
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let id = GetDlgCtrlID(lp as HWND);
                let color = if id == ID_HDR_PROVIDERS as i32 || id == ID_HDR_GENERAL as i32 {
                    RED_TEXT
                } else if (200..=206).contains(&id) || id == 902 {
                    DIM
                } else {
                    INK
                };
                SetTextColor(wp as HDC, color);
                SetBkColor(wp as HDC, BG);
                theme().2 as LRESULT
            }
            WM_CTLCOLOREDIT => {
                SetTextColor(wp as HDC, INK);
                SetBkColor(wp as HDC, EDIT_BG);
                theme().3 as LRESULT
            }
            WM_DRAWITEM => {
                draw_button(&*(lp as *const DRAWITEMSTRUCT));
                1
            }
            WM_KEY_CHANGED => {
                refresh_key_rows(hwnd, state(hwnd));
                0
            }
            WM_COMMAND => {
                let id = wp & 0xFFFF;
                match id {
                    ID_SAVE => {
                        if save(hwnd) {
                            DestroyWindow(hwnd);
                        }
                    }
                    ID_CANCEL => {
                        DestroyWindow(hwnd);
                    }
                    ID_OPEN_DATA => {
                        let st = state(hwnd);
                        let _ = std::fs::create_dir_all(&st.dir);
                        ShellExecuteW(
                            hwnd,
                            wide("open").as_ptr(),
                            wide(&st.dir.to_string_lossy()).as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                            SW_SHOWNORMAL,
                        );
                    }
                    ID_CLEAR_HISTORY => {
                        let st = state(hwnd);
                        let fb = st.feedback;
                        match store::clear(&st.dir) {
                            Ok(()) => set_text(
                                fb,
                                "History cleared — old history kept as history.json.bak.",
                            ),
                            Err(e) => set_text(fb, &e),
                        }
                    }
                    _ if (ID_KEYBTN0..ID_KEYBTN0 + 5).contains(&id) => {
                        let idx = id - ID_KEYBTN0;
                        let dir = state(hwnd).dir.clone();
                        key_dialog(hwnd, &dir, KEY_IDS[idx]);
                    }
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    drop(Box::from_raw(ptr as *mut State));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                DONE.store(true, Ordering::SeqCst);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    /// Apply the window's values. Returns false (and explains) when a value
    /// is invalid, so the window stays open for correction.
    unsafe fn save(hwnd: HWND) -> bool {
        let st = state(hwnd);
        let parse = |edit: HWND, lo: u32, hi: u32, what: &str| -> Option<u32> {
            match get_text(edit).trim().parse::<u32>() {
                Ok(n) if (lo..=hi).contains(&n) => Some(n),
                _ => {
                    msg_box(
                        hwnd,
                        &format!("{what} must be a whole number from {lo} to {hi}."),
                        MB_OK | MB_ICONWARNING,
                    );
                    None
                }
            }
        };
        let Some(refresh) = parse(st.refresh_edit, 5, 120, "Refresh interval") else {
            return false;
        };
        let Some(warn) = parse(st.warn_edit, 50, 100, "Warning threshold") else {
            return false;
        };
        let (mut cfg, _) = config::load(&st.dir);
        cfg.providers.codex = checked(st.checks[0]);
        cfg.providers.claude_code = checked(st.checks[1]);
        cfg.providers.openrouter = checked(st.checks[2]);
        cfg.providers.deepseek = checked(st.checks[3]);
        cfg.providers.moonshot = checked(st.checks[4]);
        cfg.providers.openai = checked(st.checks[5]);
        cfg.providers.anthropic = checked(st.checks[6]);
        cfg.refresh_minutes = refresh;
        cfg.warn_threshold_percent = warn;
        let want_auto = checked(st.autostart);
        match ops::set_autostart(want_auto) {
            Ok(()) => cfg.start_with_windows = want_auto,
            Err(e) => {
                msg_box(
                    hwnd,
                    &format!("Start with Windows was not changed: {e}"),
                    MB_OK | MB_ICONWARNING,
                );
                cfg.start_with_windows = ops::autostart_enabled();
            }
        }
        if let Err(e) = config::save(&st.dir, &cfg) {
            msg_box(
                hwnd,
                &format!("Settings were not saved: {e}"),
                MB_OK | MB_ICONERROR,
            );
            return false;
        }
        if let Some(t) = st.tray {
            PostMessageW(t, crate::tray::imp::WM_SETTINGS_SAVED, 0, 0);
        }
        true
    }

    unsafe fn draw_button(dis: &DRAWITEMSTRUCT) {
        let dc = dis.hDC;
        let rc = dis.rcItem;
        let pressed = dis.itemState & ODS_SELECTED != 0;
        let disabled = dis.itemState & ODS_DISABLED != 0;
        let primary = dis.CtlID as usize == ID_SAVE;
        let fill = if disabled {
            rgb(50, 50, 56)
        } else if primary {
            if pressed {
                rgb(165, 32, 45)
            } else {
                RED
            }
        } else if pressed {
            BTN_PRESS
        } else {
            BTN
        };
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, if primary { RED } else { EDGE });
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(dc, rc.left, rc.top, rc.right, rc.bottom, 8, 8);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
        let text = get_text(dis.hwndItem);
        if !text.is_empty() {
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, if disabled { DIM } else { INK });
            let mut r2 = rc;
            let w = wide(&text);
            DrawTextW(
                dc,
                w.as_ptr(),
                (w.len() - 1) as i32,
                &mut r2,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }

    // ---------------- API key dialog ----------------

    struct KeyInit {
        dir: PathBuf,
        provider: &'static str,
        name: &'static str,
    }

    struct KeyState {
        dir: PathBuf,
        provider: &'static str,
        edit: HWND,
    }

    static KEY_DONE: AtomicBool = AtomicBool::new(false);
    const ID_REMOVE_KEY: usize = 701;

    unsafe fn key_dialog(parent: HWND, dir: &std::path::Path, provider: &'static str) {
        let name = PROVIDERS
            .iter()
            .find(|(id, _)| *id == provider)
            .map(|(_, n)| *n)
            .unwrap_or(provider);
        let init = Box::new(KeyInit {
            dir: dir.to_path_buf(),
            provider,
            name,
        });
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("CodexBarWinKeyDlg");
        let dlg = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            wide(&format!("{name} API key")).as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            0,
            0,
            476,
            219,
            parent,
            std::ptr::null_mut(),
            hinst,
            Box::into_raw(init) as *const _,
        );
        if dlg.is_null() {
            return;
        }
        // Center over the settings window.
        let mut prc: RECT = std::mem::zeroed();
        GetWindowRect(parent, &mut prc);
        let mut drc: RECT = std::mem::zeroed();
        GetWindowRect(dlg, &mut drc);
        let (dw, dh) = (drc.right - drc.left, drc.bottom - drc.top);
        SetWindowPos(
            dlg,
            std::ptr::null_mut(),
            prc.left + ((prc.right - prc.left) - dw) / 2,
            prc.top + ((prc.bottom - prc.top) - dh) / 2,
            0,
            0,
            SWP_NOSIZE,
        );
        EnableWindow(parent, 0);
        KEY_DONE.store(false, Ordering::SeqCst);
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r <= 0 {
                break;
            }
            if IsDialogMessageW(dlg, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if KEY_DONE.load(Ordering::SeqCst) {
                break;
            }
        }
        EnableWindow(parent, 1);
        SetForegroundWindow(parent);
        PostMessageW(parent, WM_KEY_CHANGED, 0, 0);
    }

    unsafe extern "system" fn key_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => {
                let cs = &*(lp as *const CREATESTRUCTW);
                let init = Box::from_raw(cs.lpCreateParams as *mut KeyInit);
                let has = secrets::get_key(&init.dir, init.provider).is_some();
                let s = 1.0f32;
                ctl(
                    hwnd,
                    "STATIC",
                    &format!(
                        "Paste the {} API key. It is stored encrypted with Windows DPAPI and never shown again.",
                        init.name
                    ),
                    0,
                    16,
                    14,
                    428,
                    36,
                    0,
                    false,
                    s,
                );
                let edit = ctl(
                    hwnd,
                    "EDIT",
                    "",
                    WS_BORDER | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32) | WS_TABSTOP,
                    16,
                    56,
                    428,
                    26,
                    400,
                    false,
                    s,
                );
                ctl(
                    hwnd,
                    "BUTTON",
                    "Save key",
                    (BS_OWNERDRAW as u32) | (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP,
                    16,
                    100,
                    110,
                    30,
                    ID_SAVE,
                    true,
                    s,
                );
                if has {
                    ctl(
                        hwnd,
                        "BUTTON",
                        "Remove key",
                        (BS_OWNERDRAW as u32) | WS_TABSTOP,
                        136,
                        100,
                        110,
                        30,
                        ID_REMOVE_KEY,
                        false,
                        s,
                    );
                }
                ctl(
                    hwnd,
                    "BUTTON",
                    "Cancel",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    334,
                    100,
                    110,
                    30,
                    ID_CANCEL,
                    false,
                    s,
                );
                SetFocus(edit);
                let st = Box::new(KeyState {
                    dir: init.dir.clone(),
                    provider: init.provider,
                    edit,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(st) as isize);
                0
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                SetTextColor(wp as HDC, INK);
                SetBkColor(wp as HDC, BG);
                theme().2 as LRESULT
            }
            WM_CTLCOLOREDIT => {
                SetTextColor(wp as HDC, INK);
                SetBkColor(wp as HDC, EDIT_BG);
                theme().3 as LRESULT
            }
            WM_DRAWITEM => {
                draw_button(&*(lp as *const DRAWITEMSTRUCT));
                1
            }
            WM_COMMAND => {
                let id = wp & 0xFFFF;
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut KeyState;
                if ptr.is_null() {
                    return 0;
                }
                let st = &*ptr;
                match id {
                    ID_SAVE => {
                        let key = get_text(st.edit);
                        if key.trim().is_empty() {
                            msg_box(
                                hwnd,
                                "Paste a key first — or Cancel.",
                                MB_OK | MB_ICONWARNING,
                            );
                            return 0;
                        }
                        match secrets::set_key(&st.dir, st.provider, &key) {
                            Ok(()) => {
                                set_provider(&st.dir, st.provider, true);
                                DestroyWindow(hwnd);
                            }
                            Err(e) => msg_box(hwnd, &e, MB_OK | MB_ICONERROR),
                        }
                    }
                    ID_REMOVE_KEY => match secrets::clear_key(&st.dir, st.provider) {
                        Ok(()) => {
                            set_provider(&st.dir, st.provider, false);
                            DestroyWindow(hwnd);
                        }
                        Err(e) => msg_box(hwnd, &e, MB_OK | MB_ICONERROR),
                    },
                    ID_CANCEL => {
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    drop(Box::from_raw(ptr as *mut KeyState));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                KEY_DONE.store(true, Ordering::SeqCst);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    /// Mirror the CLI: storing a key turns its provider on, removing the
    /// key turns it off.
    fn set_provider(dir: &std::path::Path, provider: &str, on: bool) {
        let (mut cfg, _) = config::load(dir);
        match provider {
            "openrouter" => cfg.providers.openrouter = on,
            "deepseek" => cfg.providers.deepseek = on,
            "moonshot" => cfg.providers.moonshot = on,
            "openai" => cfg.providers.openai = on,
            "anthropic" => cfg.providers.anthropic = on,
            _ => return,
        }
        let _ = config::save(dir, &cfg);
    }
}

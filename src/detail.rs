//! Usage detail window (Windows): a dark window of provider cards
//! (design B, picked 2026-11-09) — one card per provider with its
//! headline number big, a supporting line, and a bar or footnote.
//! Opened from the tray menu's "Details…" item or the `details`
//! command. The card content is a pure function of the latest reports,
//! so it is tested off Windows; only the surface is Windows-only.

use crate::model::{fmt_num, ProviderReport};

/// One provider card, ready to paint. (Consumed by the Windows
/// surface; exercised directly by tests elsewhere.)
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub label: String,
    pub big: String,
    pub sub: String,
    pub foot: String,
    /// Bar fill percent (0–100) when the headline is a usage window.
    pub bar: Option<f64>,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{cut}\u{2026}")
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn others(windows: &[crate::model::UsageWindow], skip: usize, now: i64) -> String {
    let parts: Vec<String> = windows
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != skip)
        .map(|(_, w)| w.describe(now))
        .collect();
    short(&parts.join(" · "), 118)
}

/// Build the card for one provider report. Headline choice, in order:
/// a percent window, then a "Cost…" dollar window, then the first
/// window. A failed provider shows a dash and its honest error.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn card_for(r: &ProviderReport, now: i64) -> Card {
    let label = r.name.to_uppercase();
    if r.windows.is_empty() {
        return Card {
            label,
            big: "\u{2014}".to_string(),
            sub: short(r.error.as_deref().unwrap_or("no data"), 56),
            foot: String::new(),
            bar: None,
        };
    }
    if let Some((i, w)) = r
        .windows
        .iter()
        .enumerate()
        .find(|(_, w)| w.used_percent.is_some())
    {
        let p = w.used_percent.unwrap_or(0.0);
        return Card {
            label,
            big: format!("{p:.0}%"),
            sub: format!("{} used", w.label.to_lowercase()),
            foot: others(&r.windows, i, now),
            bar: Some(p),
        };
    }
    if let Some((i, w)) = r
        .windows
        .iter()
        .enumerate()
        .find(|(_, w)| w.unit == "USD" && w.label.starts_with("Cost"))
    {
        return Card {
            label,
            big: format!("${:.2}", w.used.unwrap_or(0.0)),
            sub: "cost this month".to_string(),
            foot: others(&r.windows, i, now),
            bar: None,
        };
    }
    let w = &r.windows[0];
    let big = match w.used {
        Some(u) if w.unit == "USD" => format!("${u:.2}"),
        Some(u) => fmt_num(u),
        None => "\u{2014}".to_string(),
    };
    let sub = if w.unit == "tokens" {
        format!("{} {}", w.label.to_lowercase(), "tokens")
    } else {
        w.label.to_lowercase()
    };
    Card {
        label,
        big,
        sub: short(&sub, 56),
        foot: others(&r.windows, 0, now),
        bar: None,
    }
}

/// Cards for a whole snapshot, in report order.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn cards_for(reports: &[ProviderReport], now: i64) -> Vec<Card> {
    reports.iter().map(|r| card_for(r, now)).collect()
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn open(_tray: Option<*mut std::ffi::c_void>) -> i32 {
    eprintln!("The detail window only runs on Windows. Use `status` or `report` here.");
    1
}

#[cfg(windows)]
pub fn open(tray: Option<windows_sys::Win32::Foundation::HWND>) -> i32 {
    imp::open(tray)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::UsageWindow;

    fn report(windows: Vec<UsageWindow>) -> ProviderReport {
        ProviderReport {
            id: "x".into(),
            name: "Codex".into(),
            ok: true,
            offline: false,
            error: None,
            windows,
            fetched_at: 0,
        }
    }

    #[test]
    fn percent_window_is_the_headline() {
        let r = report(vec![
            UsageWindow {
                label: "Session (5h)".into(),
                used_percent: Some(42.0),
                resets_at: Some(3600),
                ..Default::default()
            },
            UsageWindow {
                label: "Weekly".into(),
                used_percent: Some(12.0),
                ..Default::default()
            },
        ]);
        let c = card_for(&r, 0);
        assert_eq!(c.big, "42%");
        assert_eq!(c.bar, Some(42.0));
        assert!(c.foot.contains("Weekly"));
    }

    #[test]
    fn cost_window_beats_token_windows() {
        let r = report(vec![
            UsageWindow {
                label: "Today".into(),
                used: Some(351_000.0),
                unit: "tokens".into(),
                ..Default::default()
            },
            UsageWindow {
                label: "Cost (month to date)".into(),
                used: Some(18.42),
                unit: "USD".into(),
                ..Default::default()
            },
        ]);
        let c = card_for(&r, 0);
        assert_eq!(c.big, "$18.42");
        assert_eq!(c.sub, "cost this month");
        assert!(c.foot.contains("Today"));
    }

    #[test]
    fn failed_provider_is_a_dash_with_its_error() {
        let mut r = report(vec![]);
        r.ok = false;
        r.error = Some("no Admin API key stored".into());
        let c = card_for(&r, 0);
        assert_eq!(c.big, "\u{2014}");
        assert!(c.sub.contains("no Admin API key"));
    }
}

#[cfg(windows)]
mod imp {
    use super::{cards_for, Card};
    use crate::{config, ops, store, timeutil};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const fn rgb(r: u32, g: u32, b: u32) -> u32 {
        r | (g << 8) | (b << 16)
    }
    const BG: u32 = rgb(32, 32, 36);
    const CARD: u32 = rgb(43, 46, 52);
    const EDGE: u32 = rgb(90, 90, 100);
    const INK: u32 = rgb(235, 235, 235);
    const DIM: u32 = rgb(160, 160, 168);
    const RED_TEXT: u32 = rgb(235, 92, 108);
    const BTN: u32 = rgb(58, 58, 64);
    const BTN_PRESS: u32 = rgb(44, 44, 50);
    const TRACK: u32 = rgb(62, 67, 75);
    const GREEN: u32 = rgb(46, 160, 90);
    const AMBER: u32 = rgb(217, 164, 32);
    const REDBAR: u32 = rgb(200, 40, 55);

    const ID_REFRESH: usize = 500;
    const ID_CLOSE: usize = 2; // IDCANCEL: Esc closes

    static OPEN: AtomicBool = AtomicBool::new(false);
    static WIN: Mutex<usize> = Mutex::new(0);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn font(height: i32, weight: i32) -> HFONT {
        unsafe {
            CreateFontW(
                height,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                wide("Segoe UI").as_ptr(),
            )
        }
    }

    struct State {
        dir: PathBuf,
        tray: Option<HWND>,
        cards: Vec<Card>,
        fetched: i64,
        f_label: HFONT,
        f_big: HFONT,
        f_norm: HFONT,
        f_small: HFONT,
    }

    struct Init {
        dir: PathBuf,
        tray: Option<HWND>,
    }

    fn load_cards(dir: &std::path::Path) -> (Vec<Card>, i64) {
        if let Some(snap) = store::latest(dir) {
            let now = timeutil::now_unix();
            return (cards_for(&snap.reports, now), snap.at);
        }
        let (cfg, _) = config::load(dir);
        let now = timeutil::now_unix();
        let reports = ops::refresh(&cfg, dir, now);
        (cards_for(&reports, now), now)
    }

    fn rows(n: usize) -> i32 {
        n.div_ceil(2) as i32
    }

    fn win_height(n: usize) -> i32 {
        60 + rows(n) * 136 + 52
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
            let dir = config::data_dir_env();
            let (cards, _) = load_cards(&dir);
            let hinst = GetModuleHandleW(std::ptr::null());
            let class_name = wide("CodexBarWinDetail");
            let bg = CreateSolidBrush(BG);
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinst,
                hbrBackground: bg,
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&wc);
            let init = Box::new(Init { dir, tray });
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                wide("CodexBar-Windows \u{2014} Usage detail").as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                680,
                win_height(cards.len()),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinst,
                Box::into_raw(init) as *const _,
            );
            if hwnd.is_null() {
                OPEN.store(false, Ordering::SeqCst);
                return 1;
            }
            *WIN.lock().unwrap() = hwnd as usize;
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if IsDialogMessageW(hwnd, &msg) == 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            OPEN.store(false, Ordering::SeqCst);
            *WIN.lock().unwrap() = 0;
            0
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn ctl(
        hwnd: HWND,
        class: &str,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        id: usize,
    ) -> HWND {
        CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            style | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            id as HWND,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }

    unsafe fn text_out(dc: HDC, x: i32, y: i32, f: HFONT, color: u32, s: &str) {
        let old = SelectObject(dc, f);
        SetTextColor(dc, color);
        let w = wide(s);
        TextOutW(dc, x, y, w.as_ptr(), (w.len() - 1) as i32);
        SelectObject(dc, old);
    }

    unsafe fn paint(hwnd: HWND, st: &State) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let bg = CreateSolidBrush(BG);
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rc);
        FillRect(dc, &rc, bg);
        DeleteObject(bg);
        SetBkMode(dc, TRANSPARENT as i32);
        // Header.
        text_out(dc, 18, 14, st.f_big, INK, "");
        text_out(dc, 18, 16, st.f_norm, INK, "Usage detail");
        let stamp = timeutil::format_utc(st.fetched);
        let hm = if stamp.len() >= 16 {
            &stamp[11..16]
        } else {
            ""
        };
        let right = format!("Refreshed {hm}");
        let mut sz: SIZE = std::mem::zeroed();
        let old = SelectObject(dc, st.f_small);
        GetTextExtentPoint32W(dc, wide(&right).as_ptr(), right.len() as i32, &mut sz);
        SelectObject(dc, old);
        text_out(dc, rc.right - sz.cx - 18, 18, st.f_small, DIM, &right);
        // Cards.
        for (i, c) in st.cards.iter().enumerate() {
            let col = (i % 2) as i32;
            let row = (i / 2) as i32;
            let x = 14 + col * 332;
            let y = 60 + row * 136;
            let brush = CreateSolidBrush(CARD);
            let pen = CreatePen(PS_SOLID, 1, EDGE);
            let ob = SelectObject(dc, brush);
            let op = SelectObject(dc, pen);
            RoundRect(dc, x, y, x + 320, y + 124, 14, 14);
            SelectObject(dc, ob);
            SelectObject(dc, op);
            DeleteObject(brush);
            DeleteObject(pen);
            text_out(dc, x + 16, y + 12, st.f_label, RED_TEXT, &c.label);
            text_out(dc, x + 16, y + 32, st.f_big, INK, &c.big);
            text_out(dc, x + 16, y + 74, st.f_norm, DIM, &c.sub);
            if let Some(p) = c.bar {
                let track = CreateSolidBrush(TRACK);
                let ob2 = SelectObject(dc, track);
                let pen2 = CreatePen(PS_SOLID, 1, TRACK);
                let op2 = SelectObject(dc, pen2);
                RoundRect(dc, x + 16, y + 100, x + 304, y + 108, 8, 8);
                SelectObject(dc, ob2);
                SelectObject(dc, op2);
                DeleteObject(track);
                DeleteObject(pen2);
                let color = if p >= 90.0 {
                    REDBAR
                } else if p >= 70.0 {
                    AMBER
                } else {
                    GREEN
                };
                let wfill = ((288.0 * p / 100.0) as i32).clamp(10, 288);
                let fillb = CreateSolidBrush(color);
                let ob3 = SelectObject(dc, fillb);
                let pen3 = CreatePen(PS_SOLID, 1, color);
                let op3 = SelectObject(dc, pen3);
                RoundRect(dc, x + 16, y + 100, x + 16 + wfill, y + 108, 8, 8);
                SelectObject(dc, ob3);
                SelectObject(dc, op3);
                DeleteObject(fillb);
                DeleteObject(pen3);
            } else if !c.foot.is_empty() {
                text_out(dc, x + 16, y + 98, st.f_small, DIM, &c.foot);
            }
        }
        // Footer note.
        let fy = 60 + rows(st.cards.len()) * 136 + 4;
        text_out(
            dc,
            18,
            fy,
            st.f_small,
            DIM,
            "Estimates use public list prices. Offline numbers keep their last known values.",
        );
        EndPaint(hwnd, &ps);
    }

    unsafe fn draw_button(dis: &DRAWITEMSTRUCT) {
        let dc = dis.hDC;
        let rc = dis.rcItem;
        let pressed = dis.itemState & ODS_SELECTED != 0;
        let brush = CreateSolidBrush(if pressed { BTN_PRESS } else { BTN });
        let pen = CreatePen(PS_SOLID, 1, EDGE);
        let ob = SelectObject(dc, brush);
        let op = SelectObject(dc, pen);
        RoundRect(dc, rc.left, rc.top, rc.right, rc.bottom, 8, 8);
        SelectObject(dc, ob);
        SelectObject(dc, op);
        DeleteObject(brush);
        DeleteObject(pen);
        let label = if dis.CtlID as usize == ID_REFRESH {
            "Refresh"
        } else {
            "Close"
        };
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, INK);
        let f = font(-13, 400);
        let old = SelectObject(dc, f);
        let mut r = rc;
        DrawTextW(
            dc,
            wide(label).as_ptr(),
            -1,
            &mut r,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(dc, old);
        DeleteObject(f);
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => {
                let cs = &*(lp as *const CREATESTRUCTW);
                let init = Box::from_raw(cs.lpCreateParams as *mut Init);
                let (cards, fetched) = load_cards(&init.dir);
                let st = Box::new(State {
                    dir: init.dir.clone(),
                    tray: init.tray,
                    cards,
                    fetched,
                    f_label: font(-13, 700),
                    f_big: font(-34, 700),
                    f_norm: font(-15, 400),
                    f_small: font(-12, 400),
                });
                let n = st.cards.len();
                ctl(
                    hwnd,
                    "BUTTON",
                    "Refresh",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    432,
                    win_height(n) - 44,
                    100,
                    30,
                    ID_REFRESH,
                );
                ctl(
                    hwnd,
                    "BUTTON",
                    "Close",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    542,
                    win_height(n) - 44,
                    100,
                    30,
                    ID_CLOSE,
                );
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(st) as isize);
                0
            }
            WM_PAINT => {
                let st = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State);
                paint(hwnd, st);
                0
            }
            WM_DRAWITEM => {
                draw_button(&*(lp as *const DRAWITEMSTRUCT));
                1
            }
            WM_COMMAND => {
                let id = wp & 0xFFFF;
                if id == ID_REFRESH {
                    let st = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State);
                    let (cfg, _) = config::load(&st.dir);
                    let now = timeutil::now_unix();
                    let reports = ops::refresh(&cfg, &st.dir, now);
                    st.cards = cards_for(&reports, now);
                    st.fetched = now;
                    if let Some(t) = st.tray {
                        PostMessageW(t, crate::tray::imp::WM_SETTINGS_SAVED, 0, 0);
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                    0
                } else if id == ID_CLOSE {
                    DestroyWindow(hwnd);
                    0
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    let st = Box::from_raw(ptr as *mut State);
                    DeleteObject(st.f_label);
                    DeleteObject(st.f_big);
                    DeleteObject(st.f_norm);
                    DeleteObject(st.f_small);
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

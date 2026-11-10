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
    short(&parts.join(" · "), 100)
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
            // An error card's whole content is its sentence — the
            // cap fits the full guidance (the sign-in and set-key
            // sentences run ~75 chars), wrapped over two lines.
            sub: short(r.error.as_deref().unwrap_or("no data"), 84),
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
    let sub = if w.unit == "tokens" && !w.label.to_lowercase().ends_with("tokens") {
        format!("{} tokens", w.label.to_lowercase())
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

/// One day in the tokens-per-day chart.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct DayTokens {
    /// Short day label, e.g. "Nov 3".
    pub label: String,
    pub tokens: f64,
    pub today: bool,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Tokens per local day for the last `days` days, oldest first. For
/// each day, the day's last snapshot contributes the sum of its
/// "Today" token windows — the providers that keep a daily count
/// (Claude Code and the two organization APIs). Days with no snapshot
/// count as zero, so gaps in the history stay visible as gaps.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn daily_tokens(
    snaps: &[crate::store::Snapshot],
    now: i64,
    days: i64,
    offset: i64,
) -> Vec<DayTokens> {
    let today = crate::timeutil::local_day_start(now, offset);
    let first = today - (days - 1) * 86_400;
    let mut by_day: std::collections::BTreeMap<i64, &crate::store::Snapshot> =
        std::collections::BTreeMap::new();
    for s in snaps {
        let d = crate::timeutil::local_day_start(s.at, offset);
        if d >= first && d <= today {
            by_day.insert(d, s);
        }
    }
    (0..days)
        .map(|i| {
            let d = first + i * 86_400;
            let tokens = by_day
                .get(&d)
                .map(|s| {
                    s.reports
                        .iter()
                        .flat_map(|r| &r.windows)
                        .filter(|w| w.label == "Today" && w.unit == "tokens")
                        .map(|w| w.used.unwrap_or(0.0))
                        .sum()
                })
                .unwrap_or(0.0);
            let date = crate::timeutil::format_utc(d + offset);
            let month = date
                .get(5..7)
                .and_then(|m| m.parse::<usize>().ok())
                .filter(|m| (1..=12).contains(m))
                .map(|m| MONTHS[m - 1])
                .unwrap_or("");
            let day = date.get(8..10).unwrap_or("").trim_start_matches('0');
            DayTokens {
                label: format!("{month} {day}"),
                tokens,
                today: d == today,
            }
        })
        .collect()
}

/// The chart is worth showing only when some day has a real count.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn chart_visible(days: &[DayTokens]) -> bool {
    days.iter().any(|d| d.tokens > 0.0)
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

    fn snap(at: i64, today_tokens: f64) -> crate::store::Snapshot {
        crate::store::Snapshot {
            at,
            breakdown: Vec::new(),
            reports: vec![ProviderReport {
                id: "claude".into(),
                name: "Claude Code".into(),
                ok: true,
                offline: false,
                error: None,
                windows: vec![UsageWindow {
                    label: "Today".into(),
                    used: Some(today_tokens),
                    unit: "tokens".into(),
                    ..Default::default()
                }],
                fetched_at: at,
            }],
        }
    }

    #[test]
    fn daily_tokens_uses_the_last_snapshot_of_each_day() {
        // Offset 0: local days are UTC days. d0 is a UTC midnight.
        let d0 = 1_700_006_400;
        let snaps = vec![
            snap(d0 + 3_600, 100.0),
            snap(d0 + 7_200, 250.0),
            snap(d0 + 86_400 + 60, 40.0),
        ];
        let days = daily_tokens(&snaps, d0 + 86_400 + 3_600, 3, 0);
        assert_eq!(days.len(), 3);
        assert_eq!(days[0].tokens, 0.0);
        assert_eq!(days[1].tokens, 250.0);
        assert_eq!(days[2].tokens, 40.0);
        assert!(days[2].today);
        assert!(!days[1].today);
        assert!(chart_visible(&days));
        assert!(!chart_visible(&daily_tokens(&[], d0, 3, 0)));
    }

    #[test]
    fn token_label_is_not_doubled() {
        let r = report(vec![UsageWindow {
            label: "Total tokens".into(),
            used: Some(89_100_000.0),
            unit: "tokens".into(),
            ..Default::default()
        }]);
        let c = card_for(&r, 0);
        assert_eq!(c.big, "89.1M");
        assert_eq!(c.sub, "total tokens");
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

    #[test]
    fn long_error_text_is_not_cut_mid_sentence() {
        let mut r = report(vec![]);
        r.ok = false;
        r.error = Some(
            "Gemini CLI is not signed in on this PC \u{2014} run `gemini` once to sign in".into(),
        );
        let c = card_for(&r, 0);
        assert_eq!(
            c.sub,
            "Gemini CLI is not signed in on this PC \u{2014} run `gemini` once to sign in"
        );
    }
}

#[cfg(windows)]
mod imp {
    use super::{cards_for, chart_visible, daily_tokens, Card, DayTokens};
    use crate::model::fmt_num;
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
    /// Posted by the load worker when its result is in `LOADED`.
    const WM_DETAIL_LOADED: u32 = WM_APP + 30;

    static OPEN: AtomicBool = AtomicBool::new(false);
    static WIN: Mutex<usize> = Mutex::new(0);
    static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

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
        chart: Vec<DayTokens>,
        breakdown: Vec<String>,
        /// The window height it was last sized to; a finished load
        /// resizes when the content's height differs.
        height: i32,
        fetched: i64,
        /// A background load is in flight (the window opened from
        /// the snapshot alone, or a refresh is running).
        loading: bool,
        /// The pending load is a full refresh, not just the
        /// breakdown fill-in.
        load_full: bool,
        f_label: HFONT,
        f_big: HFONT,
        f_norm: HFONT,
        f_small: HFONT,
    }

    struct Init {
        dir: PathBuf,
        tray: Option<HWND>,
        cards: Vec<Card>,
        chart: Vec<DayTokens>,
        breakdown: Vec<String>,
        fetched: i64,
        loading: bool,
        load_full: bool,
    }

    /// What a finished background load hands back through `LOADED`.
    struct Loaded {
        full: bool,
        cards: Option<Vec<Card>>,
        chart: Option<Vec<DayTokens>>,
        breakdown: Vec<String>,
        fetched: Option<i64>,
    }

    #[derive(Clone, Copy)]
    enum LoadKind {
        Full,
        BreakdownOnly,
    }

    fn load_chart(dir: &std::path::Path, now: i64) -> Vec<DayTokens> {
        let offset = timeutil::local_utc_offset_seconds();
        daily_tokens(&store::all(dir), now, 14, offset)
    }

    /// Everything the last snapshot already knows, ready to paint.
    struct SnapshotData {
        cards: Vec<Card>,
        fetched: i64,
        chart: Vec<DayTokens>,
        breakdown: Vec<String>,
    }

    /// The instant open path: everything the last snapshot already
    /// knows — cards, chart, and the breakdown lines stored with it.
    /// Store reads only; no provider collection, no log scans. One
    /// history read serves both the cards and the chart — the file
    /// used to be parsed twice per open.
    fn load_snapshot(dir: &std::path::Path) -> Option<SnapshotData> {
        let snaps = store::all(dir);
        let snap = snaps.last()?;
        let now = timeutil::now_unix();
        let offset = timeutil::local_utc_offset_seconds();
        Some(SnapshotData {
            cards: cards_for(&snap.reports, now),
            fetched: snap.at,
            chart: daily_tokens(&snaps, now, 14, offset),
            breakdown: snap.breakdown.clone(),
        })
    }

    /// Run a load off the UI thread (the tray's spawn_refresh
    /// pattern): the window paints what it has, the worker collects,
    /// and WM_DETAIL_LOADED applies the result.
    fn spawn_load(hwnd: HWND, dir: PathBuf, kind: LoadKind) {
        let hwnd_addr = hwnd as usize;
        std::thread::spawn(move || {
            let (cfg, _) = config::load(&dir);
            let now = timeutil::now_unix();
            let loaded = match kind {
                LoadKind::Full => {
                    let (reports, lines) = ops::refresh_detailed(&cfg, &dir, now);
                    Loaded {
                        full: true,
                        cards: Some(cards_for(&reports, now)),
                        chart: Some(load_chart(&dir, now)),
                        breakdown: lines,
                        fetched: Some(now),
                    }
                }
                LoadKind::BreakdownOnly => Loaded {
                    full: false,
                    cards: None,
                    chart: None,
                    breakdown: ops::breakdown_lines(&cfg, now),
                    fetched: None,
                },
            };
            if let Ok(mut slot) = LOADED.lock() {
                *slot = Some(loaded);
            }
            unsafe {
                PostMessageW(hwnd_addr as HWND, WM_DETAIL_LOADED, 0, 0);
            }
        });
    }

    /// The frame's size beyond the client area (borders + caption),
    /// measured from the window itself — exact at any DPI. The
    /// layout is all client-area math, so the window is sized to
    /// client + frame; before, the frame came OUT of the layout
    /// height and the content's bottom was cut off.
    unsafe fn frame_extra(hwnd: HWND) -> (i32, i32) {
        let mut wr: RECT = std::mem::zeroed();
        let mut cr: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut wr);
        GetClientRect(hwnd, &mut cr);
        (
            (wr.right - wr.left) - (cr.right - cr.left),
            (wr.bottom - wr.top) - (cr.bottom - cr.top),
        )
    }

    /// Resize the window (and move its buttons) when the content's
    /// height differs from the height it was last sized to.
    unsafe fn apply_height(hwnd: HWND, st: &mut State) {
        let h = win_height(st.cards.len(), &st.chart, &st.breakdown);
        if h == st.height {
            return;
        }
        st.height = h;
        let (fw, fh) = frame_extra(hwnd);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            680 + fw,
            h + fh,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        for (id, x) in [(ID_REFRESH, 432), (ID_CLOSE, 542)] {
            let b = GetDlgItem(hwnd, id as i32);
            if !b.is_null() {
                SetWindowPos(b, std::ptr::null_mut(), x, h - 44, 100, 30, SWP_NOZORDER);
            }
        }
    }

    fn rows(n: usize) -> i32 {
        n.div_ceil(2) as i32
    }

    /// Extra window height for the chart section, when it is shown.
    const CHART_H: i32 = 116;

    fn chart_extra(chart: &[DayTokens]) -> i32 {
        if chart_visible(chart) {
            CHART_H
        } else {
            0
        }
    }

    /// Extra window height for the breakdown section: title, one line
    /// per model, and padding.
    fn breakdown_extra(lines: &[String]) -> i32 {
        if lines.is_empty() {
            0
        } else {
            30 + (lines.len() as i32) * 18 + 6
        }
    }

    fn sections_extra(chart: &[DayTokens], breakdown: &[String]) -> i32 {
        chart_extra(chart) + breakdown_extra(breakdown)
    }

    fn win_height(n: usize, chart: &[DayTokens], breakdown: &[String]) -> i32 {
        60 + rows(n) * 160 + sections_extra(chart, breakdown) + 52
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
            // Open from the snapshot alone — the window appears at
            // once; a background load fills in whatever the snapshot
            // cannot know yet (see spawn_load).
            let (cards, fetched, chart, breakdown, loading, load_full) = match load_snapshot(&dir) {
                Some(d) => {
                    let fill_breakdown = d.breakdown.is_empty();
                    (
                        d.cards,
                        d.fetched,
                        d.chart,
                        d.breakdown,
                        fill_breakdown,
                        false,
                    )
                }
                None => (Vec::new(), 0, Vec::new(), Vec::new(), true, true),
            };
            let height = win_height(cards.len(), &chart, &breakdown);
            let hinst = GetModuleHandleW(std::ptr::null());
            let class_name = wide("CodexBarWinDetail");
            let bg = CreateSolidBrush(BG);
            let icon = crate::icon::app_icon();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinst,
                hIcon: icon,
                hIconSm: icon,
                hbrBackground: bg,
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&wc);
            let init = Box::new(Init {
                dir,
                tray,
                cards,
                chart,
                breakdown,
                fetched,
                loading,
                load_full,
            });
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                wide("CodexBar-Windows \u{2014} Usage detail").as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                680,
                height,
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

    /// Draw a single line of text centered in a slot (chart day labels).
    unsafe fn text_center(dc: HDC, x: i32, y: i32, w: i32, f: HFONT, color: u32, s: &str) {
        let old = SelectObject(dc, f);
        SetTextColor(dc, color);
        let mut rc = RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + 16,
        };
        DrawTextW(
            dc,
            wide(s).as_ptr(),
            -1,
            &mut rc,
            DT_CENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }

    /// Draw text wrapped to up to the rect's height in lines, clipped
    /// at the rect — card lines wrap instead of running off the card.
    #[allow(clippy::too_many_arguments)]
    unsafe fn text_wrap(dc: HDC, x: i32, y: i32, w: i32, h: i32, f: HFONT, color: u32, s: &str) {
        let old = SelectObject(dc, f);
        SetTextColor(dc, color);
        let mut rc = RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        };
        DrawTextW(
            dc,
            wide(s).as_ptr(),
            -1,
            &mut rc,
            DT_WORDBREAK | DT_NOPREFIX,
        );
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
        let right = if st.loading {
            if st.cards.is_empty() {
                "Loading\u{2026}".to_string()
            } else {
                "Refreshing\u{2026}".to_string()
            }
        } else {
            // Local wall time: shift the UTC stamp by the machine's
            // offset before formatting (it printed UTC before).
            let local = st.fetched + timeutil::local_utc_offset_seconds();
            let stamp = timeutil::format_utc(local);
            let hm = if stamp.len() >= 16 {
                &stamp[11..16]
            } else {
                ""
            };
            format!("Refreshed {hm}")
        };
        let mut sz: SIZE = std::mem::zeroed();
        let old = SelectObject(dc, st.f_small);
        GetTextExtentPoint32W(dc, wide(&right).as_ptr(), right.len() as i32, &mut sz);
        SelectObject(dc, old);
        text_out(dc, rc.right - sz.cx - 18, 18, st.f_small, DIM, &right);
        // Body states before any cards exist.
        if st.cards.is_empty() {
            let msg = if st.loading {
                "Collecting your latest numbers\u{2026}"
            } else {
                "No providers are enabled \u{2014} open Settings to turn some on."
            };
            text_out(dc, 18, 64, st.f_norm, DIM, msg);
        }
        // Cards.
        for (i, c) in st.cards.iter().enumerate() {
            let col = (i % 2) as i32;
            let row = (i / 2) as i32;
            let x = 14 + col * 332;
            let y = 60 + row * 160;
            let brush = CreateSolidBrush(CARD);
            let pen = CreatePen(PS_SOLID, 1, EDGE);
            let ob = SelectObject(dc, brush);
            let op = SelectObject(dc, pen);
            RoundRect(dc, x, y, x + 320, y + 148, 14, 14);
            SelectObject(dc, ob);
            SelectObject(dc, op);
            DeleteObject(brush);
            DeleteObject(pen);
            text_out(dc, x + 16, y + 12, st.f_label, RED_TEXT, &c.label);
            text_out(dc, x + 16, y + 30, st.f_big, INK, &c.big);
            text_wrap(dc, x + 16, y + 70, 288, 40, st.f_norm, DIM, &c.sub);
            if let Some(p) = c.bar {
                let track = CreateSolidBrush(TRACK);
                let ob2 = SelectObject(dc, track);
                let pen2 = CreatePen(PS_SOLID, 1, TRACK);
                let op2 = SelectObject(dc, pen2);
                RoundRect(dc, x + 16, y + 122, x + 304, y + 130, 8, 8);
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
                RoundRect(dc, x + 16, y + 122, x + 16 + wfill, y + 130, 8, 8);
                SelectObject(dc, ob3);
                SelectObject(dc, op3);
                DeleteObject(fillb);
                DeleteObject(pen3);
            } else if !c.foot.is_empty() {
                text_wrap(dc, x + 16, y + 114, 288, 32, st.f_small, DIM, &c.foot);
            }
        }
        // Tokens-per-day chart, when any day has a real count.
        if chart_visible(&st.chart) {
            let top = 60 + rows(st.cards.len()) * 160 + 8;
            text_out(dc, 18, top, st.f_norm, INK, "Tokens per day");
            let peak = st.chart.iter().map(|d| d.tokens).fold(0.0f64, f64::max);
            let right = format!("peak {} · last 14 days", fmt_num(peak));
            let mut sz: SIZE = std::mem::zeroed();
            let old = SelectObject(dc, st.f_small);
            GetTextExtentPoint32W(dc, wide(&right).as_ptr(), right.len() as i32, &mut sz);
            SelectObject(dc, old);
            text_out(dc, rc.right - sz.cx - 18, top + 2, st.f_small, DIM, &right);
            let base = top + 92;
            let lb = CreateSolidBrush(EDGE);
            let brc = RECT {
                left: 18,
                top: base,
                right: 664,
                bottom: base + 1,
            };
            FillRect(dc, &brc, lb);
            DeleteObject(lb);
            for (i, d) in st.chart.iter().enumerate() {
                let slot = 18 + (i as i32) * 46;
                if d.tokens > 0.0 && peak > 0.0 {
                    let h = ((d.tokens / peak) * 58.0) as i32;
                    let color = if d.today {
                        RED_TEXT
                    } else {
                        rgb(122, 133, 148)
                    };
                    let b = CreateSolidBrush(color);
                    let r = RECT {
                        left: slot + 9,
                        top: base - h.max(2),
                        right: slot + 37,
                        bottom: base,
                    };
                    FillRect(dc, &r, b);
                    DeleteObject(b);
                }
                if i % 2 == 0 || d.today {
                    text_center(dc, slot, base + 6, 46, st.f_small, DIM, &d.label);
                }
            }
        }
        // Per-model breakdown, when the local logs produced rows.
        if !st.breakdown.is_empty() {
            let top = 60 + rows(st.cards.len()) * 160 + chart_extra(&st.chart) + 8;
            text_out(dc, 18, top, st.f_norm, INK, "Per-model breakdown");
            let right = "last 30 days · list-price estimates";
            let mut sz: SIZE = std::mem::zeroed();
            let old = SelectObject(dc, st.f_small);
            GetTextExtentPoint32W(dc, wide(right).as_ptr(), right.len() as i32, &mut sz);
            SelectObject(dc, old);
            text_out(dc, rc.right - sz.cx - 18, top + 2, st.f_small, DIM, right);
            for (i, line) in st.breakdown.iter().enumerate() {
                text_out(dc, 18, top + 26 + (i as i32) * 18, st.f_small, DIM, line);
            }
        }
        // Footer note.
        let fy = 60 + rows(st.cards.len()) * 160 + sections_extra(&st.chart, &st.breakdown) + 4;
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
                let height = win_height(init.cards.len(), &init.chart, &init.breakdown);
                let st = Box::new(State {
                    dir: init.dir.clone(),
                    tray: init.tray,
                    cards: init.cards,
                    chart: init.chart,
                    breakdown: init.breakdown,
                    height,
                    fetched: init.fetched,
                    loading: init.loading,
                    load_full: init.load_full,
                    f_label: font(-13, 700),
                    f_big: font(-34, 700),
                    f_norm: font(-15, 400),
                    f_small: font(-12, 400),
                });
                ctl(
                    hwnd,
                    "BUTTON",
                    "Refresh",
                    (BS_OWNERDRAW as u32) | WS_TABSTOP,
                    432,
                    height - 44,
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
                    height - 44,
                    100,
                    30,
                    ID_CLOSE,
                );
                // The creation size was the layout (client) size;
                // grow the window by the real frame so the client
                // area is exactly what the layout expects.
                let (fw, fh) = frame_extra(hwnd);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    680 + fw,
                    height + fh,
                    SWP_NOMOVE | SWP_NOZORDER,
                );
                let pending = if st.loading {
                    Some(if st.load_full {
                        LoadKind::Full
                    } else {
                        LoadKind::BreakdownOnly
                    })
                } else {
                    None
                };
                let dir = st.dir.clone();
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(st) as isize);
                if let Some(kind) = pending {
                    spawn_load(hwnd, dir, kind);
                }
                0
            }
            WM_DETAIL_LOADED => {
                let st = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State);
                if let Some(loaded) = LOADED.lock().unwrap().take() {
                    st.breakdown = loaded.breakdown;
                    if loaded.full {
                        if let Some(cards) = loaded.cards {
                            st.cards = cards;
                        }
                        if let Some(chart) = loaded.chart {
                            st.chart = chart;
                        }
                        if let Some(f) = loaded.fetched {
                            st.fetched = f;
                        }
                    }
                    st.loading = false;
                    apply_height(hwnd, st);
                    if loaded.full {
                        if let Some(t) = st.tray {
                            PostMessageW(t, crate::tray::imp::WM_SETTINGS_SAVED, 0, 0);
                        }
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
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
                    // The refresh runs on a worker; the result lands
                    // through WM_DETAIL_LOADED. A click while a load
                    // is already in flight does nothing.
                    if !st.loading {
                        st.loading = true;
                        st.load_full = true;
                        let dir = st.dir.clone();
                        InvalidateRect(hwnd, std::ptr::null(), 1);
                        spawn_load(hwnd, dir, LoadKind::Full);
                    }
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

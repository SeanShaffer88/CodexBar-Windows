//! Operations: the single implementation of every capability. The tray and
//! the command line both call these — nothing lives only in a click handler.

use crate::config::Config;
use crate::model::ProviderReport;
use crate::scrapers::{
    anthropic, claude_code, codex, deepseek, gemini, moonshot, openai, openrouter,
};
use crate::store;
use std::path::{Path, PathBuf};

/// Collect fresh reports from every enabled provider, plus the
/// per-model breakdown lines computed from the SAME pass: the local
/// scrapers hand back their per-model rows alongside the report, so
/// a caller that needs both (the detail window) never scans the
/// logs twice. When a provider fails because the network looks down
/// and a cached snapshot exists, the cached numbers are returned
/// with an Offline note instead of an empty error.
fn collect_impl(cfg: &Config, data_dir: &Path, now: i64) -> (Vec<ProviderReport>, Vec<String>) {
    let home = crate::config::home_dir();
    let mut out: Vec<ProviderReport> = Vec::new();
    let mut claude_rows = None;
    let mut codex_rows = None;
    if cfg.providers.codex {
        let dir = cfg
            .codex_dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        let (report, rows) = codex::collect_with_breakdown(&dir, now);
        out.push(report);
        codex_rows = rows;
    }
    if cfg.providers.claude_code {
        let dir = cfg
            .claude_dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".claude")))
            .unwrap_or_else(|| PathBuf::from(".claude"));
        let (report, rows) = claude_code::collect_with_breakdown(&dir, now);
        out.push(report);
        claude_rows = rows;
    }
    if cfg.providers.gemini {
        let dir = cfg
            .gemini_dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".gemini")))
            .unwrap_or_else(|| PathBuf::from(".gemini"));
        out.push(gemini::collect(&dir, now));
    }
    if cfg.providers.openrouter {
        out.push(openrouter::collect(data_dir, now));
    }
    if cfg.providers.deepseek {
        out.push(deepseek::collect(data_dir, now));
    }
    if cfg.providers.moonshot {
        out.push(moonshot::collect(data_dir, now));
    }
    if cfg.providers.openai {
        out.push(openai::collect(data_dir, now));
    }
    if cfg.providers.anthropic {
        out.push(anthropic::collect(data_dir, now));
    }
    // Offline fallback: swap in the last cached windows for network failures.
    let cached = store::latest(data_dir);
    for r in out.iter_mut() {
        if !r.ok && r.offline {
            if let Some(snap) = &cached {
                if let Some(old) = snap.reports.iter().find(|o| o.id == r.id && o.ok) {
                    r.windows = old.windows.clone();
                    r.error = Some(format!(
                        "offline — showing last known numbers from {}",
                        crate::timeutil::format_utc(old.fetched_at)
                    ));
                }
            }
        }
    }
    let lines = format_breakdown_lines(claude_rows.as_deref(), codex_rows.as_deref());
    (out, lines)
}

/// True when the worst window crossed `threshold` upward since the last
/// check — the tray notifies once per crossing, never repeatedly.
/// (Consumed by the Windows tray; exercised directly by tests elsewhere.)
#[cfg_attr(not(windows), allow(dead_code))]
pub fn crossed_threshold(prev: Option<f64>, now: Option<f64>, threshold: f64) -> bool {
    match (prev, now) {
        (Some(p), Some(n)) => p < threshold && n >= threshold,
        (None, Some(n)) => n >= threshold,
        _ => false,
    }
}

/// The busiest window across all healthy reports: (provider, label, percent).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn worst_window(reports: &[ProviderReport]) -> Option<(String, String, f64)> {
    let mut best: Option<(String, String, f64)> = None;
    for r in reports.iter().filter(|r| r.ok) {
        for w in &r.windows {
            if let Some(p) = w.used_percent {
                if best.as_ref().map(|(_, _, bp)| p > *bp).unwrap_or(true) {
                    best = Some((r.name.clone(), w.label.clone(), p));
                }
            }
        }
    }
    best
}

/// Display lines for the detail window's per-model breakdown section:
/// the local providers' last-30-days totals, busiest model first
/// (at most 8). Estimates are public list-price estimates.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn breakdown_lines(cfg: &Config, now: i64) -> Vec<String> {
    let home = crate::config::home_dir();
    let resolve = |opt: &Option<String>, leaf: &str| {
        opt.as_ref()
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(leaf)))
            .unwrap_or_else(|| PathBuf::from(leaf))
    };
    let claude_rows =
        crate::scrapers::claude_code::model_breakdown(&resolve(&cfg.claude_dir, ".claude"), now);
    let codex_rows =
        crate::scrapers::codex::model_breakdown(&resolve(&cfg.codex_dir, ".codex"), now);
    format_breakdown_lines(claude_rows.as_deref(), codex_rows.as_deref())
}

/// Pure formatting behind `breakdown_lines`: merge both providers'
/// per-model month rows, drop zero rows, sort by tokens, cap at 8.
pub fn format_breakdown_lines(
    claude_rows: Option<&[(String, crate::scrapers::claude_code::ModelUse)]>,
    codex_rows: Option<&[(String, crate::scrapers::codex::MonthUse)]>,
) -> Vec<String> {
    let est = |model: &str,
               uncached: i64,
               cache_read: i64,
               cache_write: i64,
               output: i64|
     -> Option<f64> {
        crate::pricing::price_for(model)
            .map(|p| crate::pricing::estimate_usd(uncached, cache_read, cache_write, output, &p))
    };
    let mut rows: Vec<(String, String, i64, Option<f64>)> = Vec::new();
    if let Some(rs) = claude_rows {
        for (m, mu) in rs {
            let t = mu.month;
            let tokens = t.input + t.output + t.cache_create + t.cache_read;
            if tokens > 0 {
                rows.push((
                    "Claude Code".to_string(),
                    m.clone(),
                    tokens,
                    est(m, t.input, t.cache_read, t.cache_create, t.output),
                ));
            }
        }
    }
    if let Some(rs) = codex_rows {
        for (m, mu) in rs {
            let tokens = mu.input + mu.output;
            if tokens > 0 {
                rows.push((
                    "Codex".to_string(),
                    m.clone(),
                    tokens,
                    est(m, (mu.input - mu.cached).max(0), mu.cached, 0, mu.output),
                ));
            }
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.2));
    rows.truncate(8);
    rows.iter()
        .map(|(prov, m, tokens, e)| match e {
            Some(v) => format!(
                "{prov} · {m} — {} tokens · est. ${v:.2}",
                crate::model::fmt_num(*tokens as f64)
            ),
            None => format!(
                "{prov} · {m} — {} tokens",
                crate::model::fmt_num(*tokens as f64)
            ),
        })
        .collect()
}

/// Collect and record a history snapshot.
pub fn refresh(cfg: &Config, data_dir: &Path, now: i64) -> Vec<ProviderReport> {
    refresh_detailed(cfg, data_dir, now).0
}

/// Collect, snapshot (reports + breakdown lines together), and
/// return both — the detail window's load path.
pub fn refresh_detailed(
    cfg: &Config,
    data_dir: &Path,
    now: i64,
) -> (Vec<ProviderReport>, Vec<String>) {
    let (reports, lines) = collect_impl(cfg, data_dir, now);
    let _ = store::append(data_dir, &reports, &lines, now);
    (reports, lines)
}

/// Render reports as plain text (used by the CLI and as the tray's data).
pub fn render_text(reports: &[ProviderReport], now: i64) -> String {
    let mut s = String::new();
    for r in reports {
        s.push_str(&format!(
            "{} — {}\n",
            r.name,
            crate::timeutil::format_utc(r.fetched_at)
        ));
        if r.ok {
            for w in &r.windows {
                s.push_str(&format!("  {}\n", w.describe(now)));
            }
        } else if !r.windows.is_empty() {
            for w in &r.windows {
                s.push_str(&format!("  {}\n", w.describe(now)));
            }
            if let Some(e) = &r.error {
                s.push_str(&format!("  ({e})\n"));
            }
        } else if let Some(e) = &r.error {
            s.push_str(&format!("  {e}\n"));
        }
    }
    s
}

/// Whether the per-user "start with Windows" Run entry exists.
/// Always false off Windows.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        imp::autostart_enabled()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Create or remove the per-user Run entry. Windows only; anywhere else
/// this is an honest error so callers never claim a state that is not real.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn set_autostart(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        imp::set_autostart(on)
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Start with Windows can only be changed on Windows.".to_string())
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::*;

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "CodexBar-Windows";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open_key(access: u32) -> Result<HKEY, String> {
        let mut key: HKEY = std::ptr::null_mut();
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                std::ptr::null_mut(),
                REG_OPTION_NON_VOLATILE,
                access,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if rc == ERROR_SUCCESS {
            Ok(key)
        } else {
            Err(format!("cannot open the startup registry key (error {rc})"))
        }
    }

    pub fn autostart_enabled() -> bool {
        let Ok(key) = open_key(KEY_QUERY_VALUE) else {
            return false;
        };
        let mut ty: u32 = 0;
        let rc = unsafe {
            RegQueryValueExW(
                key,
                wide(VALUE_NAME).as_ptr(),
                std::ptr::null_mut(),
                &mut ty,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        unsafe { RegCloseKey(key) };
        rc == ERROR_SUCCESS
    }

    pub fn set_autostart(on: bool) -> Result<(), String> {
        let key = open_key(KEY_SET_VALUE)?;
        let rc = if on {
            let exe = std::env::current_exe()
                .map_err(|e| format!("cannot find the program path: {e}"))?;
            let val = wide(&format!("\"{}\"", exe.to_string_lossy()));
            unsafe {
                RegSetValueExW(
                    key,
                    wide(VALUE_NAME).as_ptr(),
                    0,
                    REG_SZ,
                    val.as_ptr() as *const u8,
                    (val.len() * 2) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(key, wide(VALUE_NAME).as_ptr()) }
        };
        unsafe { RegCloseKey(key) };
        // Deleting an entry that is not there is success for "off".
        if rc == ERROR_SUCCESS || (!on && rc == 2) {
            Ok(())
        } else {
            Err(format!(
                "cannot update the startup registry entry (error {rc})"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_crossing_fires_once() {
        assert!(crossed_threshold(Some(80.0), Some(95.0), 90.0));
        assert!(!crossed_threshold(Some(95.0), Some(97.0), 90.0));
        assert!(!crossed_threshold(Some(95.0), Some(80.0), 90.0));
        assert!(crossed_threshold(None, Some(91.0), 90.0));
        assert!(!crossed_threshold(None, None, 90.0));
    }

    #[test]
    fn breakdown_lines_sort_cap_and_label() {
        use crate::scrapers::claude_code::{ModelUse, Use};
        use crate::scrapers::codex::MonthUse;
        let claude = vec![(
            "claude-sonnet-4-5".to_string(),
            ModelUse {
                today: Use::default(),
                month: Use {
                    input: 100,
                    output: 50,
                    cache_create: 0,
                    cache_read: 850,
                },
            },
        )];
        let codex = vec![(
            "gpt-5".to_string(),
            MonthUse {
                input: 2_000,
                cached: 500,
                output: 100,
            },
        )];
        let lines = format_breakdown_lines(Some(&claude), Some(&codex));
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("Codex · gpt-5"));
        assert!(lines[0].contains("2.10K tokens"));
        assert!(lines[0].contains("est. $"));
        assert!(lines[1].starts_with("Claude Code · claude-sonnet-4-5"));
        assert!(format_breakdown_lines(None, None).is_empty());
    }

    #[test]
    fn render_shows_windows_and_errors() {
        let reports = vec![
            ProviderReport {
                id: "codex".into(),
                name: "Codex".into(),
                ok: true,
                offline: false,
                error: None,
                windows: vec![crate::model::UsageWindow {
                    label: "Session (5h)".into(),
                    used_percent: Some(42.0),
                    ..Default::default()
                }],
                fetched_at: 0,
            },
            ProviderReport::failed(
                "openrouter",
                "OpenRouter",
                0,
                "no API key stored".into(),
                false,
            ),
        ];
        let text = render_text(&reports, 0);
        assert!(text.contains("Session (5h): 42% used"));
        assert!(text.contains("no API key stored"));
    }
}

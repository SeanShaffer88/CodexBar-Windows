//! Operations: the single implementation of every capability. The tray and
//! the command line both call these — nothing lives only in a click handler.

use crate::config::Config;
use crate::model::ProviderReport;
use crate::scrapers::{claude_code, codex, openrouter};
use crate::store;
use std::path::{Path, PathBuf};

/// Collect fresh reports from every enabled provider. When a provider fails
/// because the network looks down and a cached snapshot exists, the cached
/// numbers are returned with an Offline note instead of an empty error.
pub fn collect_all(cfg: &Config, data_dir: &Path, now: i64) -> Vec<ProviderReport> {
    let home = crate::config::home_dir();
    let mut out: Vec<ProviderReport> = Vec::new();
    if cfg.providers.codex {
        let dir = cfg
            .codex_dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        out.push(codex::collect(&dir, now));
    }
    if cfg.providers.claude_code {
        let dir = cfg
            .claude_dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".claude")))
            .unwrap_or_else(|| PathBuf::from(".claude"));
        out.push(claude_code::collect(&dir, now));
    }
    if cfg.providers.openrouter {
        out.push(openrouter::collect(data_dir, now));
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
    out
}

/// Collect and record a history snapshot.
pub fn refresh(cfg: &Config, data_dir: &Path, now: i64) -> Vec<ProviderReport> {
    let reports = collect_all(cfg, data_dir, now);
    let _ = store::append(data_dir, &reports, now);
    reports
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

#[cfg(test)]
mod tests {
    use super::*;

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

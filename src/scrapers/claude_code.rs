//! Claude Code local logs: `~/.claude/projects/**/*.jsonl`.
//!
//! Each assistant message records its token usage (input, output, and cache
//! reads/writes) with a timestamp. Claude Code does not store the account's
//! rate-limit percentages locally, so this source reports real token totals
//! for today, the last 7 days, and the last 30 days — never invented
//! percentages. Duplicate copies of one message (resumed sessions) are
//! counted once, by message id.

use crate::model::{ProviderReport, UsageWindow};
use crate::timeutil;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TokenTotals {
    pub today: i64,
    pub week: i64,
    pub month: i64,
}

fn usage_total(v: &serde_json::Value) -> i64 {
    let g = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    g("input_tokens")
        + g("output_tokens")
        + g("cache_creation_input_tokens")
        + g("cache_read_input_tokens")
}

/// Fold one transcript file's lines into `totals`, relative to `now`.
/// "Today" starts at local midnight (`utc_offset_secs` east of UTC);
/// the 7/30-day windows are rolling.
pub fn parse_lines(
    lines: &[String],
    now: i64,
    utc_offset_secs: i64,
    totals: &mut TokenTotals,
    seen: &mut HashSet<String>,
) {
    let today_start = timeutil::local_day_start(now, utc_offset_secs);
    let week_start = now - 7 * 86_400;
    let month_start = now - 30 * 86_400;
    for line in lines {
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let Some(msg) = v.get("message") else {
            continue;
        };
        let Some(usage) = msg.get("usage") else {
            continue;
        };
        if let Some(id) = msg.get("id").and_then(|i| i.as_str()) {
            if !seen.insert(id.to_string()) {
                continue;
            }
        }
        let Some(ts) = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(timeutil::parse_iso8601)
        else {
            continue;
        };
        let n = usage_total(usage);
        if ts >= today_start {
            totals.today += n;
        }
        if ts >= week_start {
            totals.week += n;
        }
        if ts >= month_start {
            totals.month += n;
        }
    }
}

/// Scan a Claude Code data directory and build the report.
pub fn collect(dir: &Path, now: i64) -> ProviderReport {
    let projects = dir.join("projects");
    if !projects.is_dir() {
        return ProviderReport::failed(
            "claude-code",
            "Claude Code",
            now,
            format!("no Claude Code projects found at {}", projects.display()),
            false,
        );
    }
    let mut totals = TokenTotals::default();
    let mut seen = HashSet::new();
    let offset = timeutil::local_utc_offset_seconds();
    for f in super::jsonl_files(&projects) {
        parse_lines(&super::read_lines(&f), now, offset, &mut totals, &mut seen);
    }
    let win = |label: &str, n: i64| UsageWindow {
        label: label.to_string(),
        used: Some(n as f64),
        unit: "tokens".to_string(),
        ..Default::default()
    };
    ProviderReport {
        id: "claude-code".to_string(),
        name: "Claude Code".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows: vec![
            win("Today", totals.today),
            win("Last 7 days", totals.week),
            win("Last 30 days", totals.month),
        ],
        fetched_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_799_200_000; // fixed reference point

    fn msg(id: &str, ts: &str, input: i64, output: i64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"{id}","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
    }

    #[test]
    fn buckets_and_dedupes() {
        // NOW is 2027-01-05T...; use timestamps relative to its UTC day.
        let today =
            timeutil::format_utc(timeutil::day_start(NOW) + 3600).replace(' ', "T") + ":00Z";
        let mut totals = TokenTotals::default();
        let mut seen = HashSet::new();
        let lines = vec![
            msg("a", &today, 100, 50),
            msg("a", &today, 100, 50), // duplicate copy of the same message
            msg("b", "2020-01-01T00:00:00Z", 999, 999), // far outside every window
        ];
        parse_lines(&lines, NOW, 0, &mut totals, &mut seen);
        assert_eq!(totals.today, 150);
        assert_eq!(totals.week, 150);
        assert_eq!(totals.month, 150);
    }

    #[test]
    fn cache_tokens_count() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":4,"cache_read_input_tokens":8}"#,
        )
        .unwrap();
        assert_eq!(usage_total(&v), 15);
    }

    #[test]
    fn missing_dir_is_honest_error() {
        let r = collect(Path::new("/nonexistent-claude-dir"), NOW);
        assert!(!r.ok);
    }
}

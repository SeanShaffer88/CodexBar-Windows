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
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TokenTotals {
    pub today: i64,
    pub week: i64,
    pub month: i64,
}

/// Token mix for one model, used for list-price cost estimates.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Use {
    pub input: i64,
    pub output: i64,
    pub cache_create: i64,
    pub cache_read: i64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ModelUse {
    pub today: Use,
    pub month: Use,
}

fn usage_parts(v: &serde_json::Value) -> Use {
    let g = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    Use {
        input: g("input_tokens"),
        output: g("output_tokens"),
        cache_create: g("cache_creation_input_tokens"),
        cache_read: g("cache_read_input_tokens"),
    }
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
    text: &str,
    now: i64,
    utc_offset_secs: i64,
    totals: &mut TokenTotals,
    seen: &mut HashSet<u64>,
    per_model: &mut HashMap<String, ModelUse>,
) {
    let today_start = timeutil::local_day_start(now, utc_offset_secs);
    let week_start = now - 7 * 86_400;
    let month_start = now - 30 * 86_400;
    for line in text.lines() {
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
            if !seen.insert(super::str_hash(id)) {
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
        if ts >= month_start {
            let model = msg
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown")
                .to_string();
            let parts = usage_parts(usage);
            let entry = per_model.entry(model).or_default();
            entry.month.input += parts.input;
            entry.month.output += parts.output;
            entry.month.cache_create += parts.cache_create;
            entry.month.cache_read += parts.cache_read;
            if ts >= today_start {
                entry.today.input += parts.input;
                entry.today.output += parts.output;
                entry.today.cache_create += parts.cache_create;
                entry.today.cache_read += parts.cache_read;
            }
        }
    }
}

/// Estimated cost (USD, public list prices) for the per-model totals.
/// Returns (today, month, any_priced_model_seen).
pub fn estimate(per_model: &HashMap<String, ModelUse>) -> (f64, f64, bool) {
    let (mut today, mut month, mut priced) = (0.0, 0.0, false);
    for (model, mu) in per_model {
        if let Some(pr) = crate::pricing::price_for(model) {
            priced = true;
            let e = |u: &Use| {
                crate::pricing::estimate_usd(u.input, u.cache_read, u.cache_create, u.output, &pr)
            };
            today += e(&mu.today);
            month += e(&mu.month);
        }
    }
    (today, month, priced)
}

/// Per-model totals for the `breakdown` view, busiest model first.
/// None when there is no projects folder at all.
/// The one transcript scan: day/week/month totals plus the per-model
/// month aggregates. `collect` and `model_breakdown` both read from
/// this single pass — the detail window used to pay for the scan
/// twice per open before the two were merged.
///
/// With a `cutoff`, files last modified before it are skipped. That
/// loses nothing here: every number this scan produces covers at
/// most the last 30 days, an entry's file is modified when the
/// entry is appended (so in-window entries live in files modified
/// inside the window), and a duplicated message counts once from
/// whichever copy is read.
fn scan_cutoff(
    dir: &Path,
    now: i64,
    cutoff: Option<i64>,
) -> Option<(TokenTotals, HashMap<String, ModelUse>)> {
    let projects = dir.join("projects");
    if !projects.is_dir() {
        return None;
    }
    let mut totals = TokenTotals::default();
    let mut seen = HashSet::new();
    let mut per_model: HashMap<String, ModelUse> = HashMap::new();
    let offset = timeutil::local_utc_offset_seconds();
    for f in super::jsonl_files(&projects) {
        if let Some(c) = cutoff {
            if f.modified_secs() < c {
                continue;
            }
        }
        let text = super::load_text(&f.path);
        parse_lines(&text, now, offset, &mut totals, &mut seen, &mut per_model);
    }
    Some((totals, per_model))
}

fn scan(dir: &Path, now: i64) -> Option<(TokenTotals, HashMap<String, ModelUse>)> {
    scan_cutoff(dir, now, None)
}

fn sorted_rows(per_model: HashMap<String, ModelUse>) -> Vec<(String, ModelUse)> {
    let total = |u: &Use| u.input + u.output + u.cache_create + u.cache_read;
    let mut rows: Vec<(String, ModelUse)> = per_model.into_iter().collect();
    rows.sort_by_key(|(_, mu)| -total(&mu.month));
    rows
}

pub fn model_breakdown(dir: &Path, now: i64) -> Option<Vec<(String, ModelUse)>> {
    scan(dir, now).map(|(_, per_model)| sorted_rows(per_model))
}

/// Scan a Claude Code data directory and build the report.
fn report_from(
    totals: &TokenTotals,
    per_model: &HashMap<String, ModelUse>,
    now: i64,
) -> ProviderReport {
    let win = |label: &str, n: i64| UsageWindow {
        label: label.to_string(),
        used: Some(n as f64),
        unit: "tokens".to_string(),
        ..Default::default()
    };
    let (today_cost, month_cost, priced) = estimate(per_model);
    let mut today_win = win("Today", totals.today);
    if priced && today_cost > 0.0 {
        today_win.detail = format!("est. cost \u{2248} ${today_cost:.2} at list prices");
    }
    let mut windows = vec![
        today_win,
        win("Last 7 days", totals.week),
        win("Last 30 days", totals.month),
    ];
    if priced {
        windows.push(UsageWindow {
            label: "Est. cost (30 days)".to_string(),
            used: Some(month_cost),
            unit: "USD".to_string(),
            detail: "estimate at public list prices - a subscription bill differs".to_string(),
            ..Default::default()
        });
    }
    ProviderReport {
        id: "claude-code".to_string(),
        name: "Claude Code".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows,
        fetched_at: now,
    }
}

fn no_projects(dir: &Path, now: i64) -> ProviderReport {
    ProviderReport::failed(
        "claude-code",
        "Claude Code",
        now,
        format!(
            "no Claude Code projects found at {}",
            dir.join("projects").display()
        ),
        false,
    )
}

/// One scan producing both the report and the per-model breakdown
/// rows, for callers that need the two together (the detail window).
/// The refresh path, so it takes the modification-time gate:
/// files untouched for 31+ days cannot hold in-window entries.
pub fn collect_with_breakdown(
    dir: &Path,
    now: i64,
) -> (ProviderReport, Option<Vec<(String, ModelUse)>>) {
    match scan_cutoff(dir, now, Some(now - 31 * 86_400)) {
        None => (no_projects(dir, now), None),
        Some((totals, per_model)) => {
            let report = report_from(&totals, &per_model, now);
            (report, Some(sorted_rows(per_model)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_799_200_000; // fixed reference point

    fn msg(id: &str, ts: &str, input: i64, output: i64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"{id}","model":"claude-sonnet-4-5","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
    }

    #[test]
    fn buckets_and_dedupes() {
        // NOW is 2027-01-05T...; use timestamps relative to its UTC day.
        let today =
            timeutil::format_utc(timeutil::day_start(NOW) + 3600).replace(' ', "T") + ":00Z";
        let mut totals = TokenTotals::default();
        let mut seen = HashSet::new();
        let lines = [
            msg("a", &today, 100, 50),
            msg("a", &today, 100, 50), // duplicate copy of the same message
            msg("b", "2020-01-01T00:00:00Z", 999, 999), // far outside every window
        ];
        let mut pm = HashMap::new();
        parse_lines(&lines.join("\n"), NOW, 0, &mut totals, &mut seen, &mut pm);
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
        let (r, rows) = collect_with_breakdown(Path::new("/nonexistent-claude-dir"), NOW);
        assert!(!r.ok);
        assert!(rows.is_none());
    }

    #[test]
    fn gate_skips_files_older_than_the_cutoff() {
        let dir = std::env::temp_dir().join(format!("cbw-claude-gate-{}", std::process::id()));
        let projects = dir.join("projects");
        std::fs::create_dir_all(&projects).unwrap();
        let today =
            timeutil::format_utc(timeutil::day_start(NOW) + 3600).replace(' ', "T") + ":00Z";
        std::fs::write(projects.join("p.jsonl"), msg("a", &today, 100, 50)).unwrap();
        let (totals, _) = scan_cutoff(&dir, NOW, None).unwrap();
        assert_eq!(totals.today, 150);
        // A cutoff past every real file's modification time gates
        // the file out entirely.
        let (totals, _) = scan_cutoff(&dir, NOW, Some(i64::MAX)).unwrap();
        assert_eq!(totals.today, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

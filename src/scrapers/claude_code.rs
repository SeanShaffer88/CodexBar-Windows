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

/// What one assistant usage line contributes, pre-extracted so a
/// scan can cache it per file (see `crate::scancache`) and replay
/// it later instead of re-parsing the transcript. Field names are
/// single letters to keep the cache file compact.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct MsgRecord {
    /// Hash of the message id (None when the line carried none).
    #[serde(rename = "i")]
    pub id: Option<u64>,
    /// Event time; None when the timestamp did not parse. Such a
    /// line still claims its id in the dedupe set when parsed
    /// directly, so the record keeps it claimable.
    #[serde(rename = "t")]
    pub ts: Option<i64>,
    #[serde(rename = "m")]
    pub model: String,
    #[serde(rename = "n")]
    pub input: i64,
    #[serde(rename = "o")]
    pub output: i64,
    #[serde(rename = "c")]
    pub cache_create: i64,
    #[serde(rename = "r")]
    pub cache_read: i64,
}

/// Extract the usage records from one transcript file's text —
/// the parsing half of `parse_lines`, with no aggregation state.
pub fn extract(text: &str) -> Vec<MsgRecord> {
    let mut out = Vec::new();
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
        let parts = usage_parts(usage);
        out.push(MsgRecord {
            id: msg.get("id").and_then(|i| i.as_str()).map(super::str_hash),
            ts: v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(timeutil::parse_iso8601),
            model: msg
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown")
                .to_string(),
            input: parts.input,
            output: parts.output,
            cache_create: parts.cache_create,
            cache_read: parts.cache_read,
        });
    }
    out
}

/// Fold extracted records into `totals`, relative to `now` —
/// the aggregation half of `parse_lines`, and all a cached scan
/// has to do. "Today" starts at local midnight
/// (`utc_offset_secs` east of UTC); the 7/30-day windows are
/// rolling, evaluated against the replay-time `now`.
pub fn replay(
    records: &[MsgRecord],
    now: i64,
    utc_offset_secs: i64,
    totals: &mut TokenTotals,
    seen: &mut HashSet<u64>,
    per_model: &mut HashMap<String, ModelUse>,
) {
    let today_start = timeutil::local_day_start(now, utc_offset_secs);
    let week_start = now - 7 * 86_400;
    let month_start = now - 30 * 86_400;
    for r in records {
        if let Some(id) = r.id {
            if !seen.insert(id) {
                continue;
            }
        }
        let Some(ts) = r.ts else {
            continue;
        };
        let n = r.input + r.output + r.cache_create + r.cache_read;
        if ts >= today_start {
            totals.today += n;
        }
        if ts >= week_start {
            totals.week += n;
        }
        if ts >= month_start {
            totals.month += n;
            let entry = per_model.entry(r.model.clone()).or_default();
            entry.month.input += r.input;
            entry.month.output += r.output;
            entry.month.cache_create += r.cache_create;
            entry.month.cache_read += r.cache_read;
            if ts >= today_start {
                entry.today.input += r.input;
                entry.today.output += r.output;
                entry.today.cache_create += r.cache_create;
                entry.today.cache_read += r.cache_read;
            }
        }
    }
}

/// Fold one transcript file's text into `totals`: extract, then
/// replay. Kept as the direct path for the uncached scans.
pub fn parse_lines(
    text: &str,
    now: i64,
    utc_offset_secs: i64,
    totals: &mut TokenTotals,
    seen: &mut HashSet<u64>,
    per_model: &mut HashMap<String, ModelUse>,
) {
    replay(
        &extract(text),
        now,
        utc_offset_secs,
        totals,
        seen,
        per_model,
    );
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
/// The uncached transcript scan (the `breakdown` command's path):
/// day/week/month totals plus the per-model month aggregates in
/// one pass over every file.
fn scan(dir: &Path, now: i64) -> Option<(TokenTotals, HashMap<String, ModelUse>)> {
    let projects = dir.join("projects");
    if !projects.is_dir() {
        return None;
    }
    let mut totals = TokenTotals::default();
    let mut seen = HashSet::new();
    let mut per_model: HashMap<String, ModelUse> = HashMap::new();
    let offset = timeutil::local_utc_offset_seconds();
    for f in super::jsonl_files(&projects) {
        let text = super::load_text(&f.path);
        parse_lines(&text, now, offset, &mut totals, &mut seen, &mut per_model);
    }
    Some((totals, per_model))
}

/// The refresh-path scan, with the modification-time gate and the
/// per-file parse cache. The gate loses nothing: every number
/// produced covers at most the last 30 days, an entry's file is
/// modified when the entry is appended (so in-window entries live
/// in files modified inside the window), and a duplicated message
/// counts once from whichever copy is read. The cache loses
/// nothing either: a file's records are replayed only when its
/// modification time and size still match the stamp they were
/// extracted under, and replay is the same aggregation the direct
/// parse runs.
fn scan_cached(
    dir: &Path,
    cache_dir: Option<&Path>,
    now: i64,
) -> Option<(TokenTotals, HashMap<String, ModelUse>)> {
    let projects = dir.join("projects");
    if !projects.is_dir() {
        return None;
    }
    let cutoff = now - 31 * 86_400;
    let mut totals = TokenTotals::default();
    let mut seen = HashSet::new();
    let mut per_model: HashMap<String, ModelUse> = HashMap::new();
    let offset = timeutil::local_utc_offset_seconds();
    let cache_file = cache_dir.map(|d| crate::scancache::cache_path(d, "scan-cache-claude.json"));
    let mut cache = cache_file
        .as_ref()
        .map(|p| crate::scancache::load::<MsgRecord>(p))
        .unwrap_or_default();
    let mut dirty = false;
    let mut alive: HashSet<String> = HashSet::new();
    let mut total_records = 0usize;
    for f in super::jsonl_files(&projects) {
        if f.modified_secs() < cutoff {
            continue;
        }
        let key = f.path.to_string_lossy().into_owned();
        alive.insert(key.clone());
        let is_hit = cache
            .files
            .get(&key)
            .map(|e| e.mtime == f.modified_secs() && e.size == f.size)
            .unwrap_or(false);
        if is_hit {
            let entry = cache.files.get(&key).expect("checked above");
            total_records += entry.records.len();
            replay(
                &entry.records,
                now,
                offset,
                &mut totals,
                &mut seen,
                &mut per_model,
            );
        } else {
            let prev = cache.files.remove(&key);
            let (records, covered) = match prev {
                // The file only grew since its records were
                // extracted: parse just the appended bytes, and
                // only up to the last complete line — a line still
                // being written is counted once its newline lands.
                Some(e) if f.size > e.size => {
                    let chunk = super::load_range(&f.path, e.covered);
                    let upto = chunk.rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let mut recs = e.records;
                    recs.extend(extract(&chunk[..upto]));
                    let covered = e.covered + upto as u64;
                    (recs, covered)
                }
                _ => {
                    let text = super::load_text(&f.path);
                    let upto = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
                    (extract(&text[..upto]), upto as u64)
                }
            };
            total_records += records.len();
            replay(
                &records,
                now,
                offset,
                &mut totals,
                &mut seen,
                &mut per_model,
            );
            if cache_file.is_some() {
                cache.files.insert(
                    key,
                    crate::scancache::FileEntry {
                        mtime: f.modified_secs(),
                        size: f.size,
                        covered,
                        records,
                    },
                );
                dirty = true;
            }
        }
    }
    if let Some(p) = &cache_file {
        let pruned = cache.files.len() != alive.len();
        if dirty || pruned {
            cache.files.retain(|k, _| alive.contains(k));
            if total_records <= crate::scancache::MAX_RECORDS {
                crate::scancache::save(p, &cache);
            }
        }
    }
    Some((totals, per_model))
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
/// The refresh path: gated, and cached when `cache_dir` is given.
pub fn collect_with_breakdown(
    dir: &Path,
    cache_dir: Option<&Path>,
    now: i64,
) -> (ProviderReport, Option<Vec<(String, ModelUse)>>) {
    match scan_cached(dir, cache_dir, now) {
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
        // Cache creation/read tokens are part of a message's total.
        let line = r#"{"type":"assistant","timestamp":"2027-01-05T01:00:00Z","message":{"id":"x","model":"m","usage":{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":4,"cache_read_input_tokens":8}}}"#;
        let recs = extract(line);
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.input + r.output + r.cache_create + r.cache_read, 15);
    }

    #[test]
    fn missing_dir_is_honest_error() {
        let (r, rows) = collect_with_breakdown(Path::new("/nonexistent-claude-dir"), None, NOW);
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
        std::fs::write(
            projects.join("p.jsonl"),
            format!("{}\n", msg("a", &today, 100, 50)),
        )
        .unwrap();
        let month = |r: &ProviderReport| {
            r.windows
                .iter()
                .find(|w| w.label == "Last 30 days")
                .and_then(|w| w.used)
        };
        // Real now: the just-written file is inside the gate.
        let (r, _) = collect_with_breakdown(&dir, None, timeutil::now_unix());
        assert_eq!(month(&r), Some(150.0));
        // A "now" far in the future: the file looks untouched for
        // over 31 days, so the gate skips it entirely.
        let (r, _) = collect_with_breakdown(&dir, None, 4_000_000_000);
        assert_eq!(month(&r), Some(0.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_matches_direct_scan_and_repicks_changes() {
        let base = std::env::temp_dir().join(format!("cbw-claude-cache-{}", std::process::id()));
        let home = base.join("home");
        let projects = home.join("projects");
        std::fs::create_dir_all(&projects).unwrap();
        let cache_dir = base.join("data");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let today =
            timeutil::format_utc(timeutil::day_start(NOW) + 3600).replace(' ', "T") + ":00Z";
        let file = projects.join("p.jsonl");
        std::fs::write(
            &file,
            format!(
                "{}\n{}\n",
                msg("a", &today, 100, 50),
                msg("b", &today, 10, 5)
            ),
        )
        .unwrap();
        let month = |r: &ProviderReport| {
            r.windows
                .iter()
                .find(|w| w.label == "Last 30 days")
                .and_then(|w| w.used)
        };
        let now = timeutil::now_unix();
        let (direct, _) = collect_with_breakdown(&home, None, now);
        assert_eq!(month(&direct), Some(165.0));
        // First cached run parses and populates; the second replays.
        let (first, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&first), Some(165.0));
        assert!(cache_dir.join("scan-cache-claude.json").exists());
        let (second, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&second), Some(165.0));
        // An append moves the stamp: the file re-parses, new total.
        let mut text = std::fs::read_to_string(&file).unwrap();
        text.push_str(&format!("{}\n", msg("c", &today, 1000, 0)));
        std::fs::write(&file, text).unwrap();
        let (third, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&third), Some(1165.0));
        // A corrupt cache is ignored, never fatal or wrong.
        std::fs::write(cache_dir.join("scan-cache-claude.json"), "{broken").unwrap();
        let (fourth, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&fourth), Some(1165.0));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn cache_tail_append_inflight_line_and_shrink() {
        let base = std::env::temp_dir().join(format!("cbw-claude-tail-{}", std::process::id()));
        let home = base.join("home");
        let projects = home.join("projects");
        std::fs::create_dir_all(&projects).unwrap();
        let cache_dir = base.join("data");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let today =
            timeutil::format_utc(timeutil::day_start(NOW) + 3600).replace(' ', "T") + ":00Z";
        let file = projects.join("p.jsonl");
        std::fs::write(&file, format!("{}\n", msg("a", &today, 100, 50))).unwrap();
        let now = timeutil::now_unix();
        let month = |r: &ProviderReport| {
            r.windows
                .iter()
                .find(|w| w.label == "Last 30 days")
                .and_then(|w| w.used)
        };
        let append = |s: &str| {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&file)
                .unwrap()
                .write_all(s.as_bytes())
                .unwrap();
        };
        let (r, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&r), Some(150.0));
        // Half of the next message lands, newline still to come:
        // not counted yet — and not lost either.
        let line_b = msg("b", &today, 200, 100);
        append(&line_b[..line_b.len() / 2]);
        let (r, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&r), Some(150.0));
        // The rest of the line plus its newline: now it counts.
        append(&format!("{}\n", &line_b[line_b.len() / 2..]));
        let (r, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&r), Some(450.0));
        // The cache entry's records and consumed offset track it.
        let text = std::fs::read_to_string(cache_dir.join("scan-cache-claude.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entry = v["files"].as_object().unwrap().values().next().unwrap();
        assert_eq!(entry["records"].as_array().unwrap().len(), 2);
        assert_eq!(
            entry["covered"].as_u64().unwrap(),
            std::fs::metadata(&file).unwrap().len()
        );
        // A shorter rewrite is re-read in full, not tail-matched.
        std::fs::write(&file, format!("{}\n", msg("c", &today, 7, 3))).unwrap();
        let (r, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(month(&r), Some(10.0));
        let _ = std::fs::remove_dir_all(&base);
    }
}

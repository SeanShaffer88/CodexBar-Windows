//! Codex CLI local logs: `~/.codex/sessions/**/*.jsonl`.
//!
//! The Codex CLI records `token_count` events that carry the account's real
//! rate-limit windows (a primary 5-hour window and a secondary weekly
//! window, each with a used percent and a reset time) exactly as the server
//! reported them. The newest event is the current state — the same source
//! CodexBar reads on macOS.

use crate::model::{ProviderReport, UsageWindow};
use crate::timeutil;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Token mix for one model over the last 30 days (input includes the
/// cached part, exactly as the Codex log reports it).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MonthUse {
    pub input: i64,
    pub cached: i64,
    pub output: i64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CodexState {
    pub session_used_percent: Option<f64>,
    pub session_resets_at: Option<i64>,
    pub weekly_used_percent: Option<f64>,
    pub weekly_resets_at: Option<i64>,
    pub plan_type: Option<String>,
    pub total_tokens: Option<i64>,
    /// Model named by the newest `turn_context` event.
    pub current_model: Option<String>,
    /// Per-model token sums over the last 30 days, from each event's
    /// `last_token_usage` (the usage of that one request).
    pub month_models: HashMap<String, MonthUse>,
}

/// The rate-limit block of one `token_count` event, flattened for
/// the scan cache. Presence is preserved exactly: a present window
/// whose values are missing still overwrites the state's values
/// with None, like the direct parse does.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct RateRec {
    #[serde(rename = "p")]
    pub primary: Option<(Option<f64>, Option<i64>)>,
    #[serde(rename = "s")]
    pub secondary: Option<(Option<f64>, Option<i64>)>,
    #[serde(rename = "l")]
    pub plan: Option<String>,
}

/// One state-changing event from a session file, pre-extracted so
/// a scan can cache it per file (see `crate::scancache`) and
/// replay it later instead of re-parsing the log.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub enum Ev {
    /// A `turn_context` line set the current model.
    #[serde(rename = "m")]
    Model(String),
    /// The hash of a `token_count`-bearing line that changed no
    /// state (unparsable or a different payload type). Replay
    /// still has to claim the hash in the dedupe set.
    #[serde(rename = "h")]
    Hash(u64),
    /// A `token_count` event.
    #[serde(rename = "c")]
    Count {
        #[serde(rename = "h")]
        hash: u64,
        #[serde(rename = "t")]
        ts: i64,
        #[serde(rename = "T")]
        total: Option<i64>,
        /// (input, cached, output) of `last_token_usage`.
        #[serde(rename = "l")]
        last: Option<(i64, i64, i64)>,
        #[serde(rename = "r")]
        rate: Option<RateRec>,
    },
}

/// Extract the events from one session file's text — the parsing
/// half of `parse_lines`, with no aggregation state.
pub fn extract(text: &str) -> Vec<Ev> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.contains("\"turn_context\"") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let payload = v.get("payload").unwrap_or(&v);
                if let Some(m) = payload.get("model").and_then(|x| x.as_str()) {
                    out.push(Ev::Model(m.to_string()));
                }
            }
            continue;
        }
        if !line.contains("\"token_count\"") {
            continue;
        }
        let hash = super::str_hash(line);
        let parsed = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .filter(|v| {
                let payload = v.get("payload").unwrap_or(v);
                payload.get("type").and_then(|t| t.as_str()) == Some("token_count")
            });
        let Some(v) = parsed else {
            out.push(Ev::Hash(hash));
            continue;
        };
        let payload = v.get("payload").unwrap_or(&v);
        let mut ts = 0;
        let mut total = None;
        let mut last = None;
        if let Some(info) = payload.get("info") {
            total = info
                .get("total_token_usage")
                .and_then(|u| u.get("total_tokens"))
                .and_then(|t| t.as_i64());
            ts = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(timeutil::parse_iso8601)
                .unwrap_or(0);
            if let Some(l) = info.get("last_token_usage") {
                let g = |k: &str| l.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
                last = Some((
                    g("input_tokens"),
                    g("cached_input_tokens"),
                    g("output_tokens"),
                ));
            }
        }
        let rate = payload.get("rate_limits").map(|rl| RateRec {
            primary: rl.get("primary").map(|p| {
                (
                    p.get("used_percent").and_then(|x| x.as_f64()),
                    p.get("resets_at").and_then(|x| x.as_i64()),
                )
            }),
            secondary: rl.get("secondary").map(|s| {
                (
                    s.get("used_percent").and_then(|x| x.as_f64()),
                    s.get("resets_at").and_then(|x| x.as_i64()),
                )
            }),
            plan: rl
                .get("plan_type")
                .and_then(|x| x.as_str())
                .map(str::to_string),
        });
        out.push(Ev::Count {
            hash,
            ts,
            total,
            last,
            rate,
        });
    }
    out
}

/// Fold extracted events into `state`; later events overwrite
/// earlier ones, and the aggregation half of `parse_lines`.
/// `seen` dedupes by raw line across resumed/forked sessions.
pub fn replay(records: &[Ev], state: &mut CodexState, seen: &mut HashSet<u64>, now: i64) {
    let month_start = now - 30 * 86_400;
    for ev in records {
        match ev {
            Ev::Model(m) => state.current_model = Some(m.clone()),
            Ev::Hash(h) => {
                seen.insert(*h);
            }
            Ev::Count {
                hash,
                ts,
                total,
                last,
                rate,
            } => {
                if !seen.insert(*hash) {
                    continue;
                }
                if let Some(t) = total {
                    state.total_tokens = Some(*t);
                }
                if *ts >= month_start {
                    if let Some((input, cached, output)) = last {
                        let model = state
                            .current_model
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string());
                        let entry = state.month_models.entry(model).or_default();
                        entry.input += input;
                        entry.cached += cached;
                        entry.output += output;
                    }
                }
                if let Some(r) = rate {
                    if let Some((u, rs)) = r.primary {
                        state.session_used_percent = u;
                        state.session_resets_at = rs;
                    }
                    if let Some((u, rs)) = r.secondary {
                        state.weekly_used_percent = u;
                        state.weekly_resets_at = rs;
                    }
                    if let Some(p) = &r.plan {
                        state.plan_type = Some(p.clone());
                    }
                }
            }
        }
    }
}

/// Fold one session file's text into `state`: extract, then
/// replay. Kept as the direct path for the uncached scans.
pub fn parse_lines(text: &str, state: &mut CodexState, seen: &mut HashSet<u64>, now: i64) {
    replay(&extract(text), state, seen, now);
}

/// The uncached session-log scan (the `breakdown` command's path,
/// and the dormant-log fallback): one pass over every file.
fn scan(dir: &Path, now: i64) -> Option<CodexState> {
    let sessions = dir.join("sessions");
    if !sessions.is_dir() {
        return None;
    }
    let mut state = CodexState::default();
    let mut seen = HashSet::new();
    for f in super::jsonl_files(&sessions) {
        let text = super::load_text(&f.path);
        parse_lines(&text, &mut state, &mut seen, now);
    }
    Some(state)
}

/// The refresh-path scan, with the modification-time gate and the
/// per-file parse cache; the second return value says whether the
/// gate skipped any file. The per-model month rows lose nothing
/// from the gate (their entries are timestamp-gated to the last
/// 30 days, and an entry's file is modified when the entry is
/// appended). The all-time fields — total tokens, plan, the
/// latest rate-limit windows — come from the newest event
/// overall, which a gated pass can only miss when NO recent file
/// carries usage; the caller checks for that case and rescans
/// ungated. The cache loses nothing either: a file's events are
/// replayed only when its modification time and size still match
/// the stamp they were extracted under.
fn scan_cached(dir: &Path, cache_dir: Option<&Path>, now: i64) -> Option<(CodexState, bool)> {
    let sessions = dir.join("sessions");
    if !sessions.is_dir() {
        return None;
    }
    let cutoff = now - 31 * 86_400;
    let mut state = CodexState::default();
    let mut seen = HashSet::new();
    let mut skipped = false;
    let cache_file = cache_dir.map(|d| crate::scancache::cache_path(d, "scan-cache-codex.json"));
    let mut cache = cache_file
        .as_ref()
        .map(|p| crate::scancache::load::<Ev>(p))
        .unwrap_or_default();
    let mut dirty = false;
    let mut alive: HashSet<String> = HashSet::new();
    let mut total_records = 0usize;
    for f in super::jsonl_files(&sessions) {
        if f.modified_secs() < cutoff {
            skipped = true;
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
            replay(&entry.records, &mut state, &mut seen, now);
        } else {
            let prev = cache.files.remove(&key);
            let (records, covered) = match prev {
                // The file only grew since its events were
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
            replay(&records, &mut state, &mut seen, now);
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
    Some((state, skipped))
}

fn sorted_rows(state: CodexState) -> Vec<(String, MonthUse)> {
    let mut rows: Vec<(String, MonthUse)> = state.month_models.into_iter().collect();
    rows.sort_by_key(|(_, mu)| -(mu.input + mu.output));
    rows
}

/// Per-model last-30-days totals for the `breakdown` view, busiest
/// model first. None when there is no sessions folder at all.
pub fn model_breakdown(dir: &Path, now: i64) -> Option<Vec<(String, MonthUse)>> {
    scan(dir, now).map(sorted_rows)
}

fn no_sessions(dir: &Path, now: i64) -> ProviderReport {
    ProviderReport::failed(
        "codex",
        "Codex",
        now,
        format!(
            "no Codex sessions found at {}",
            dir.join("sessions").display()
        ),
        false,
    )
}

fn report_from(state: &CodexState, now: i64) -> ProviderReport {
    if state.session_used_percent.is_none() && state.total_tokens.is_none() {
        return ProviderReport::failed(
            "codex",
            "Codex",
            now,
            "Codex logs found, but none carry usage yet — run Codex once and refresh".to_string(),
            false,
        );
    }
    let mut windows = Vec::new();
    if state.session_used_percent.is_some() {
        windows.push(UsageWindow {
            label: "Session (5h)".to_string(),
            used_percent: state.session_used_percent,
            resets_at: state.session_resets_at,
            unit: "tokens".to_string(),
            ..Default::default()
        });
    }
    if state.weekly_used_percent.is_some() {
        windows.push(UsageWindow {
            label: "Weekly".to_string(),
            used_percent: state.weekly_used_percent,
            resets_at: state.weekly_resets_at,
            unit: "tokens".to_string(),
            ..Default::default()
        });
    }
    let (month_cost, any_priced) = {
        let mut cost = 0.0;
        let mut priced = false;
        for (model, mu) in &state.month_models {
            if let Some(pr) = crate::pricing::price_for(model) {
                priced = true;
                let uncached = (mu.input - mu.cached).max(0);
                cost += crate::pricing::estimate_usd(uncached, mu.cached, 0, mu.output, &pr);
            }
        }
        (cost, priced)
    };
    if any_priced {
        windows.push(UsageWindow {
            label: "Est. cost (30 days)".to_string(),
            used: Some(month_cost),
            unit: "USD".to_string(),
            detail: "API list-price estimate - Codex plans are subscription-based".to_string(),
            ..Default::default()
        });
    }
    if let Some(t) = state.total_tokens {
        windows.push(UsageWindow {
            label: "Total tokens".to_string(),
            used: Some(t as f64),
            unit: "tokens".to_string(),
            detail: match &state.plan_type {
                Some(p) => format!("plan: {p}"),
                None => String::new(),
            },
            ..Default::default()
        });
    }
    let _ = timeutil::day_start(now); // (kept for parity with other scrapers)
    ProviderReport {
        id: "codex".to_string(),
        name: "Codex".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows,
        fetched_at: now,
    }
}

/// One scan producing both the report and the per-model breakdown
/// rows, for callers that need the two together (the detail window).
/// The refresh path: gated, cached when `cache_dir` is given, and
/// with the dormant-log fallback — when the gated pass skipped
/// files and found no usage at all, the all-time totals may be
/// hiding in an old file, so the scan runs again ungated.
pub fn collect_with_breakdown(
    dir: &Path,
    cache_dir: Option<&Path>,
    now: i64,
) -> (ProviderReport, Option<Vec<(String, MonthUse)>>) {
    match scan_cached(dir, cache_dir, now) {
        None => (no_sessions(dir, now), None),
        Some((state, skipped)) => {
            let dormant =
                skipped && state.session_used_percent.is_none() && state.total_tokens.is_none();
            let state = if dormant {
                scan(dir, now).unwrap_or(state)
            } else {
                state
            };
            let report = report_from(&state, now);
            (report, Some(sorted_rows(state)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(used: f64, resets: i64, total: i64) -> String {
        format!(
            r#"{{"timestamp":"2026-11-09T10:00:00Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"total_tokens":{total}}}}},"rate_limits":{{"primary":{{"used_percent":{used},"window_minutes":300,"resets_at":{resets}}},"secondary":{{"used_percent":3.0,"window_minutes":10080,"resets_at":{resets}}},"plan_type":"plus"}}}}}}"#
        )
    }

    #[test]
    fn newest_event_wins() {
        let mut st = CodexState::default();
        let mut seen = HashSet::new();
        parse_lines(
            &[line(10.0, 100, 500), line(42.0, 200, 900)].join("\n"),
            &mut st,
            &mut seen,
            0,
        );
        assert_eq!(st.session_used_percent, Some(42.0));
        assert_eq!(st.session_resets_at, Some(200));
        assert_eq!(st.weekly_used_percent, Some(3.0));
        assert_eq!(st.total_tokens, Some(900));
        assert_eq!(st.plan_type.as_deref(), Some("plus"));
    }

    #[test]
    fn ignores_other_lines_and_bad_json() {
        let mut st = CodexState::default();
        let mut seen = HashSet::new();
        parse_lines(
            "{\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\"}}\n{broken token_count",
            &mut st,
            &mut seen,
            0,
        );
        assert_eq!(st, CodexState::default());
    }

    #[test]
    fn missing_dir_is_honest_error() {
        let (r, rows) = collect_with_breakdown(Path::new("/nonexistent-codex-dir"), None, 0);
        assert!(!r.ok);
        assert!(r.error.unwrap().contains("no Codex sessions"));
        assert!(rows.is_none());
    }

    #[test]
    fn cache_matches_direct_scan() {
        let base = std::env::temp_dir().join(format!("cbw-codex-cache-{}", std::process::id()));
        let home = base.join("home");
        let sessions = home.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let cache_dir = base.join("data");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            sessions.join("s.jsonl"),
            format!("{}\n", line(42.0, 200, 900)),
        )
        .unwrap();
        let now = crate::timeutil::now_unix();
        let total = |r: &ProviderReport| {
            r.windows
                .iter()
                .find(|w| w.label == "Total tokens")
                .and_then(|w| w.used)
        };
        let (direct, _) = collect_with_breakdown(&home, None, now);
        assert_eq!(total(&direct), Some(900.0));
        // First cached run parses and populates; the second replays
        // and must land on the same state, rate limits included.
        let (first, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(total(&first), Some(900.0));
        assert!(cache_dir.join("scan-cache-codex.json").exists());
        let (second, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(total(&second), Some(900.0));
        assert!(second
            .windows
            .iter()
            .any(|w| w.label == "Session (5h)" && w.used_percent == Some(42.0)));
        // An appended event arrives through the tail read and wins.
        {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(sessions.join("s.jsonl"))
                .unwrap()
                .write_all(format!("{}\n", line(43.0, 300, 950)).as_bytes())
                .unwrap();
        }
        let (third, _) = collect_with_breakdown(&home, Some(&cache_dir), now);
        assert_eq!(total(&third), Some(950.0));
        assert!(third
            .windows
            .iter()
            .any(|w| w.label == "Session (5h)" && w.used_percent == Some(43.0)));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn gate_falls_back_for_dormant_logs() {
        let dir = std::env::temp_dir().join(format!("cbw-codex-gate-{}", std::process::id()));
        let sessions = dir.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(sessions.join("s.jsonl"), line(42.0, 200, 900)).unwrap();
        // A "now" far in the future: the only file looks untouched
        // for over 31 days, so the gated pass skips it — and must
        // fall back to the full scan rather than report no usage.
        let (r, _rows) = collect_with_breakdown(&dir, None, 4_000_000_000);
        assert!(r.ok);
        assert!(r
            .windows
            .iter()
            .any(|w| w.label == "Total tokens" && w.used == Some(900.0)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

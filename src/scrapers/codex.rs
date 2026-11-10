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

/// Fold one session file's lines into `state`; later events overwrite
/// earlier ones. `seen` dedupes by raw line across resumed/forked sessions.
pub fn parse_lines(lines: &[String], state: &mut CodexState, seen: &mut HashSet<String>, now: i64) {
    let month_start = now - 30 * 86_400;
    for line in lines {
        if line.contains("\"turn_context\"") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let payload = v.get("payload").unwrap_or(&v);
                if let Some(m) = payload.get("model").and_then(|x| x.as_str()) {
                    state.current_model = Some(m.to_string());
                }
            }
            continue;
        }
        if !line.contains("\"token_count\"") {
            continue;
        }
        if !seen.insert(line.clone()) {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let payload = v.get("payload").unwrap_or(&v);
        if payload.get("type").and_then(|t| t.as_str()) != Some("token_count") {
            continue;
        }
        if let Some(info) = payload.get("info") {
            if let Some(t) = info
                .get("total_token_usage")
                .and_then(|u| u.get("total_tokens"))
                .and_then(|t| t.as_i64())
            {
                state.total_tokens = Some(t);
            }
            let ts = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(timeutil::parse_iso8601)
                .unwrap_or(0);
            if ts >= month_start {
                if let Some(last) = info.get("last_token_usage") {
                    let g = |k: &str| last.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
                    let model = state
                        .current_model
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let entry = state.month_models.entry(model).or_default();
                    entry.input += g("input_tokens");
                    entry.cached += g("cached_input_tokens");
                    entry.output += g("output_tokens");
                }
            }
        }
        if let Some(rl) = payload.get("rate_limits") {
            if let Some(p) = rl.get("primary") {
                state.session_used_percent = p.get("used_percent").and_then(|x| x.as_f64());
                state.session_resets_at = p.get("resets_at").and_then(|x| x.as_i64());
            }
            if let Some(s) = rl.get("secondary") {
                state.weekly_used_percent = s.get("used_percent").and_then(|x| x.as_f64());
                state.weekly_resets_at = s.get("resets_at").and_then(|x| x.as_i64());
            }
            if let Some(plan) = rl.get("plan_type").and_then(|x| x.as_str()) {
                state.plan_type = Some(plan.to_string());
            }
        }
    }
}

/// Per-model last-30-days totals for the `breakdown` view, busiest
/// model first. None when there is no sessions folder at all.
pub fn model_breakdown(dir: &Path, now: i64) -> Option<Vec<(String, MonthUse)>> {
    let sessions = dir.join("sessions");
    if !sessions.is_dir() {
        return None;
    }
    let mut state = CodexState::default();
    let mut seen = HashSet::new();
    for f in super::jsonl_files(&sessions) {
        parse_lines(&super::read_lines(&f), &mut state, &mut seen, now);
    }
    let mut rows: Vec<(String, MonthUse)> = state.month_models.into_iter().collect();
    rows.sort_by_key(|(_, mu)| -(mu.input + mu.output));
    Some(rows)
}

/// Scan a Codex data directory and build the report.
pub fn collect(dir: &Path, now: i64) -> ProviderReport {
    let sessions = dir.join("sessions");
    if !sessions.is_dir() {
        return ProviderReport::failed(
            "codex",
            "Codex",
            now,
            format!("no Codex sessions found at {}", sessions.display()),
            false,
        );
    }
    let mut state = CodexState::default();
    let mut seen = HashSet::new();
    for f in super::jsonl_files(&sessions) {
        parse_lines(&super::read_lines(&f), &mut state, &mut seen, now);
    }
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
            &[line(10.0, 100, 500), line(42.0, 200, 900)],
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
            &[
                "{\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\"}}".into(),
                "{broken token_count".into(),
            ],
            &mut st,
            &mut seen,
            0,
        );
        assert_eq!(st, CodexState::default());
    }

    #[test]
    fn missing_dir_is_honest_error() {
        let r = collect(Path::new("/nonexistent-codex-dir"), 0);
        assert!(!r.ok);
        assert!(r.error.unwrap().contains("no Codex sessions"));
    }
}

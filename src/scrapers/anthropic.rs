//! Anthropic API organization usage and cost reports on api.anthropic.com.
//! Endpoints: `GET /v1/organizations/usage_report/messages` and
//! `GET /v1/organizations/cost_report`. These are the documented Usage
//! & Cost (Admin) endpoints; they need an **Admin API key**
//! (`sk-ant-admin...`, created in the Console under Settings, Admin
//! keys) sent as `x-api-key` with an `anthropic-version` header. The
//! organization must be on a Team plan or higher - on individual plans
//! the API refuses, and that refusal is reported as-is.
//!
//! Cost amounts come back as decimal strings in **cents**; they are
//! divided by 100 before display. The key is stored DPAPI-encrypted (see
//! `secrets`) and only ever sent to api.anthropic.com.

use crate::model::{fmt_num, ProviderReport, UsageWindow};
use crate::timeutil;
use std::path::Path;

const ID: &str = "anthropic";
const NAME: &str = "Anthropic API";

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Bucket {
    pub start: i64,
    /// Uncached input + cache-creation + cache-read + output.
    pub total: i64,
    pub uncached_in: i64,
    pub cache_read: i64,
    pub output: i64,
}

fn result_total(r: &serde_json::Value) -> Bucket {
    let g = |k: &str| r.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let creation = r
        .get("cache_creation")
        .map(|c| {
            let h = |k: &str| c.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
            h("ephemeral_1h_input_tokens") + h("ephemeral_5m_input_tokens")
        })
        .unwrap_or(0);
    let b = Bucket {
        start: 0,
        uncached_in: g("uncached_input_tokens"),
        cache_read: g("cache_read_input_tokens"),
        output: g("output_tokens"),
        total: 0,
    };
    Bucket {
        total: b.uncached_in + creation + b.cache_read + b.output,
        ..b
    }
}

/// Parse one page of the messages usage report into per-day buckets.
pub fn parse_usage_page(body: &str) -> Result<Vec<Bucket>, String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "the server's answer could not be read".to_string())?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "the server's answer had no data block".to_string())?;
    let mut out = Vec::new();
    for b in data {
        let start = b
            .get("starting_at")
            .and_then(|x| x.as_str())
            .and_then(timeutil::parse_iso8601)
            .unwrap_or(0);
        let mut bucket = Bucket {
            start,
            ..Default::default()
        };
        if let Some(results) = b.get("results").and_then(|r| r.as_array()) {
            for r in results {
                let t = result_total(r);
                bucket.total += t.total;
                bucket.uncached_in += t.uncached_in;
                bucket.cache_read += t.cache_read;
                bucket.output += t.output;
            }
        }
        out.push(bucket);
    }
    Ok(out)
}

/// Parse one page of the cost report into a USD total (amounts are
/// decimal strings in cents).
pub fn parse_costs_page(body: &str) -> Result<f64, String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "the server's answer could not be read".to_string())?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "the server's answer had no data block".to_string())?;
    let mut cents = 0.0;
    for b in data {
        if let Some(results) = b.get("results").and_then(|r| r.as_array()) {
            for r in results {
                if let Some(a) = r.get("amount") {
                    cents += a
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .or_else(|| a.as_f64())
                        .unwrap_or(0.0);
                }
            }
        }
    }
    Ok(cents / 100.0)
}

fn sum_from(buckets: &[Bucket], from: i64) -> Bucket {
    let mut t = Bucket::default();
    for b in buckets.iter().filter(|b| b.start >= from) {
        t.total += b.total;
        t.uncached_in += b.uncached_in;
        t.cache_read += b.cache_read;
        t.output += b.output;
    }
    t
}

fn token_window(label: &str, b: Bucket) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        used: Some(b.total as f64),
        unit: "tokens".to_string(),
        detail: format!(
            "{} new in · {} cache read · {} out",
            fmt_num(b.uncached_in as f64),
            fmt_num(b.cache_read as f64),
            fmt_num(b.output as f64)
        ),
        ..Default::default()
    }
}

fn fetch_pages(base_url: &str, key: &str) -> Result<Vec<String>, String> {
    let headers = [("x-api-key", key), ("anthropic-version", "2023-06-01")];
    let mut pages = Vec::new();
    let mut url = base_url.to_string();
    for _ in 0..4 {
        let body = super::http::get_json_headers(&url, &headers)?;
        let next = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| {
                if v.get("has_more").and_then(|x| x.as_bool()) == Some(true) {
                    v.get("next_page")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            });
        pages.push(body);
        match next {
            Some(p) => {
                let sep = if base_url.contains('?') { "&" } else { "?" };
                url = format!("{base_url}{sep}page={p}");
            }
            None => break,
        }
    }
    Ok(pages)
}

pub fn collect(data_dir: &Path, now: i64) -> ProviderReport {
    let Some(key) = crate::secrets::get_key(data_dir, ID) else {
        return ProviderReport::failed(
            ID,
            NAME,
            now,
            "no Admin API key stored — add one with: codexbar-windows set-key anthropic (an sk-ant-admin key from the Console, Team plan or higher)".into(),
            false,
        );
    };
    let start = timeutil::month_start_utc(now);
    let usage_url = format!(
        "https://api.anthropic.com/v1/organizations/usage_report/messages?starting_at={}&ending_at={}&bucket_width=1d",
        timeutil::rfc3339(start),
        timeutil::rfc3339(now)
    );
    let costs_url = format!(
        "https://api.anthropic.com/v1/organizations/cost_report?starting_at={}&ending_at={}",
        timeutil::rfc3339(start),
        timeutil::rfc3339(now)
    );
    let fail = |e: String| {
        let offline =
            e.contains("offline") || e.contains("could not reach") || e.contains("no answer");
        ProviderReport::failed(ID, NAME, now, e, offline)
    };
    let mut buckets: Vec<Bucket> = Vec::new();
    match fetch_pages(&usage_url, &key) {
        Ok(pages) => {
            for p in &pages {
                match parse_usage_page(p) {
                    Ok(mut b) => buckets.append(&mut b),
                    Err(e) => return fail(e),
                }
            }
        }
        Err(e) => return fail(e),
    }
    let mut cost = 0.0;
    match fetch_pages(&costs_url, &key) {
        Ok(pages) => {
            for p in &pages {
                match parse_costs_page(p) {
                    Ok(c) => cost += c,
                    Err(e) => return fail(e),
                }
            }
        }
        Err(e) => return fail(e),
    }
    let today = timeutil::day_start(now);
    let week = timeutil::day_start(now - 6 * 86_400);
    ProviderReport {
        id: ID.to_string(),
        name: NAME.to_string(),
        ok: true,
        offline: false,
        error: None,
        windows: vec![
            token_window("Today", sum_from(&buckets, today)),
            token_window("Last 7 days", sum_from(&buckets, week)),
            UsageWindow {
                label: "Cost (month to date)".to_string(),
                used: Some(cost),
                unit: "USD".to_string(),
                detail: "whole organization, from the cost report".to_string(),
                ..Default::default()
            },
        ],
        fetched_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{"data":[
      {"starting_at":"2026-11-08T00:00:00Z","ending_at":"2026-11-09T00:00:00Z","results":[{"uncached_input_tokens":1000,"cache_creation":{"ephemeral_1h_input_tokens":100,"ephemeral_5m_input_tokens":50},"cache_read_input_tokens":5000,"output_tokens":300}]},
      {"starting_at":"2026-11-09T00:00:00Z","ending_at":"2026-11-10T00:00:00Z","results":[{"uncached_input_tokens":2000,"cache_creation":{"ephemeral_1h_input_tokens":0,"ephemeral_5m_input_tokens":0},"cache_read_input_tokens":9000,"output_tokens":700}]}
    ],"has_more":false}"#;

    #[test]
    fn parses_usage_buckets() {
        let buckets = parse_usage_page(USAGE).unwrap();
        assert_eq!(buckets.len(), 2);
        let day2 = timeutil::parse_iso8601("2026-11-09T00:00:00Z").unwrap();
        let t = sum_from(&buckets, day2);
        assert_eq!(t.total, 2000 + 9000 + 700);
        assert_eq!(t.uncached_in, 2000);
        let all = sum_from(&buckets, 0);
        assert_eq!(all.total, 6450 + 11_700);
    }

    #[test]
    fn parses_costs_cents_to_dollars() {
        let body = r#"{"data":[{"starting_at":"2026-11-08T00:00:00Z","results":[{"amount":"12345","currency":"USD","description":"tokens"},{"amount":"55","currency":"USD"}]}]}"#;
        assert_eq!(parse_costs_page(body).unwrap(), 124.0);
    }

    #[test]
    fn bad_bodies_are_errors() {
        assert!(parse_usage_page("nope").is_err());
        assert!(parse_usage_page("{}").is_err());
        assert!(parse_costs_page("{}").is_err());
    }
}

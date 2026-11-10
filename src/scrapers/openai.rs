//! OpenAI API organization usage: `GET /v1/organization/usage/completions`
//! and `GET /v1/organization/costs` on api.openai.com. These are the
//! documented Administration endpoints; they need an **Admin API key**
//! (`sk-admin-…`, created under Settings → Admin keys), not a regular
//! project key. This is API-platform usage for the whole organization —
//! it is not ChatGPT subscription quota, which has no public API.
//!
//! The key is stored DPAPI-encrypted (see `secrets`) and only ever sent
//! to api.openai.com.

use crate::model::{fmt_num, ProviderReport, UsageWindow};
use crate::timeutil;
use std::path::Path;

const ID: &str = "openai";
const NAME: &str = "OpenAI API";

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Bucket {
    pub start: i64,
    pub input: i64,
    pub output: i64,
    pub requests: i64,
}

/// Parse one page of the completions-usage response into per-day buckets.
pub fn parse_usage_page(body: &str) -> Result<(Vec<Bucket>, Option<String>), String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "the server's answer could not be read".to_string())?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "the server's answer had no data block".to_string())?;
    let mut out = Vec::new();
    for b in data {
        let start = b.get("start_time").and_then(|x| x.as_i64()).unwrap_or(0);
        let mut bucket = Bucket {
            start,
            ..Default::default()
        };
        if let Some(results) = b.get("results").and_then(|r| r.as_array()) {
            for r in results {
                let g = |k: &str| r.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
                bucket.input += g("input_tokens");
                bucket.output += g("output_tokens");
                bucket.requests += g("num_model_requests");
            }
        }
        out.push(bucket);
    }
    let next = if v.get("has_more").and_then(|x| x.as_bool()) == Some(true) {
        v.get("next_page")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };
    Ok((out, next))
}

/// Parse one page of the costs response into a USD total. The API is
/// documented to return `amount.value` as a number but sometimes ships a
/// string; both are accepted.
pub fn parse_costs_page(body: &str) -> Result<(f64, Option<String>), String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "the server's answer could not be read".to_string())?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "the server's answer had no data block".to_string())?;
    let mut total = 0.0;
    for b in data {
        if let Some(results) = b.get("results").and_then(|r| r.as_array()) {
            for r in results {
                if let Some(amount) = r.get("amount") {
                    let value = amount.get("value").map(|x| {
                        x.as_f64()
                            .or_else(|| x.as_str().and_then(|s| s.parse::<f64>().ok()))
                            .unwrap_or(0.0)
                    });
                    total += value.unwrap_or(0.0);
                }
            }
        }
    }
    let next = if v.get("has_more").and_then(|x| x.as_bool()) == Some(true) {
        v.get("next_page")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };
    Ok((total, next))
}

fn sum_from(buckets: &[Bucket], from: i64) -> Bucket {
    let mut t = Bucket::default();
    for b in buckets.iter().filter(|b| b.start >= from) {
        t.input += b.input;
        t.output += b.output;
        t.requests += b.requests;
    }
    t
}

fn token_window(label: &str, b: Bucket) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        used: Some((b.input + b.output) as f64),
        unit: "tokens".to_string(),
        detail: format!(
            "{} in · {} out · {} requests",
            fmt_num(b.input as f64),
            fmt_num(b.output as f64),
            fmt_num(b.requests as f64)
        ),
        ..Default::default()
    }
}

/// Fetch all pages (bounded) of a usage/costs endpoint.
fn fetch_pages(base_url: &str, key: &str) -> Result<Vec<String>, String> {
    let mut pages = Vec::new();
    let mut url = base_url.to_string();
    for _ in 0..4 {
        let body = super::http::get_json(&url, Some(key))?;
        // Peek for the continuation token without committing to a shape.
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
            "no Admin API key stored — add one with: codexbar-windows set-key openai (an sk-admin key from Settings \u{2192} Admin keys)".into(),
            false,
        );
    };
    let start = timeutil::month_start_utc(now);
    let usage_url = format!(
        "https://api.openai.com/v1/organization/usage/completions?start_time={start}&end_time={now}&bucket_width=1d"
    );
    let costs_url = format!(
        "https://api.openai.com/v1/organization/costs?start_time={start}&end_time={now}&bucket_width=1d"
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
                    Ok((mut b, _)) => buckets.append(&mut b),
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
                    Ok((c, _)) => cost += c,
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
                detail: "whole organization, from the costs API".to_string(),
                ..Default::default()
            },
        ],
        fetched_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{"object":"page","has_more":false,"data":[
      {"start_time":1000,"end_time":2000,"results":[{"input_tokens":100,"output_tokens":40,"input_cached_tokens":10,"num_model_requests":3,"model":"gpt-5"}]},
      {"start_time":90000,"end_time":100000,"results":[{"input_tokens":200,"output_tokens":60,"num_model_requests":5},{"input_tokens":50,"output_tokens":10,"num_model_requests":1}]}
    ]}"#;

    #[test]
    fn parses_usage_buckets() {
        let (buckets, next) = parse_usage_page(USAGE).unwrap();
        assert_eq!(buckets.len(), 2);
        assert_eq!(next, None);
        let t = sum_from(&buckets, 90_000);
        assert_eq!((t.input, t.output, t.requests), (250, 70, 6));
        let all = sum_from(&buckets, 0);
        assert_eq!((all.input, all.output, all.requests), (350, 110, 9));
    }

    #[test]
    fn parses_costs_number_and_string() {
        let body = r#"{"object":"page","has_more":false,"data":[{"start_time":1,"results":[{"amount":{"value":12.5,"currency":"usd"}},{"amount":{"value":"2.50","currency":"usd"}}]}]}"#;
        let (total, _) = parse_costs_page(body).unwrap();
        assert_eq!(total, 15.0);
    }

    #[test]
    fn bad_bodies_are_errors() {
        assert!(parse_usage_page("nope").is_err());
        assert!(parse_usage_page("{}").is_err());
        assert!(parse_costs_page("{}").is_err());
    }
}

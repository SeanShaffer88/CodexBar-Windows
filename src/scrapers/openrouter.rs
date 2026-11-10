//! OpenRouter: `GET https://openrouter.ai/api/v1/auth/key` with the user's
//! API key returns that key's real usage and limit. The key is stored
//! DPAPI-encrypted (see `secrets`) and only ever sent to openrouter.ai.

use crate::model::{ProviderReport, UsageWindow};
use std::path::Path;

pub const KEY_URL: &str = "https://openrouter.ai/api/v1/auth/key";

/// Turn an `/auth/key` response body into a report. Pure, so it is tested
/// against recorded response shapes.
pub fn parse_response(body: &str, now: i64) -> ProviderReport {
    let parsed = serde_json::from_str::<serde_json::Value>(body);
    let data = match parsed {
        Ok(v) => match v.get("data") {
            Some(d) => d.clone(),
            None => {
                return ProviderReport::failed(
                    "openrouter",
                    "OpenRouter",
                    now,
                    "the server's answer had no data block".into(),
                    false,
                )
            }
        },
        Err(_) => {
            return ProviderReport::failed(
                "openrouter",
                "OpenRouter",
                now,
                "the server's answer could not be read".into(),
                false,
            )
        }
    };
    let usage = data.get("usage").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let limit = data.get("limit").and_then(|x| x.as_f64());
    let remaining = data.get("limit_remaining").and_then(|x| x.as_f64());
    let free = data
        .get("is_free_tier")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let mut w = UsageWindow {
        label: "Credits".to_string(),
        used: Some(usage),
        limit,
        unit: "USD".to_string(),
        ..Default::default()
    };
    if let (Some(l), Some(r)) = (limit, remaining) {
        if l > 0.0 {
            w.used_percent = Some(((l - r) / l * 100.0).clamp(0.0, 100.0));
        }
        w.detail = format!("${r:.2} left");
    } else if free {
        w.detail = "free tier, no credit limit".to_string();
    } else {
        w.detail = "no credit limit set".to_string();
    }
    ProviderReport {
        id: "openrouter".to_string(),
        name: "OpenRouter".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows: vec![w],
        fetched_at: now,
    }
}

/// Collect with the stored key. No key is an honest "not set up" report,
/// not an error in the data.
pub fn collect(data_dir: &Path, now: i64) -> ProviderReport {
    let Some(key) = crate::secrets::get_openrouter_key(data_dir) else {
        return ProviderReport::failed(
            "openrouter",
            "OpenRouter",
            now,
            "no API key stored — add one with: codexbar-windows set-key openrouter".into(),
            false,
        );
    };
    match super::http::get_json(KEY_URL, Some(&key)) {
        Ok(body) => parse_response(&body, now),
        Err(e) => {
            let offline =
                e.contains("offline") || e.contains("could not reach") || e.contains("no answer");
            ProviderReport::failed("openrouter", "OpenRouter", now, e, offline)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_limited_key() {
        let body = r#"{"data":{"label":"sk-or-...","usage":12.5,"limit":50.0,"limit_remaining":37.5,"is_free_tier":false}}"#;
        let r = parse_response(body, 0);
        assert!(r.ok);
        let w = &r.windows[0];
        assert_eq!(w.used, Some(12.5));
        assert_eq!(w.limit, Some(50.0));
        assert_eq!(w.used_percent, Some(25.0));
    }

    #[test]
    fn parses_unlimited_key() {
        let body =
            r#"{"data":{"usage":3.25,"limit":null,"limit_remaining":null,"is_free_tier":true}}"#;
        let r = parse_response(body, 0);
        assert!(r.ok);
        assert_eq!(r.windows[0].used_percent, None);
    }

    #[test]
    fn bad_body_is_honest_error() {
        assert!(!parse_response("nope", 0).ok);
        assert!(!parse_response("{}", 0).ok);
    }
}

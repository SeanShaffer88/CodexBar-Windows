//! DeepSeek: `GET https://api.deepseek.com/user/balance` with the user's
//! API key returns the account's real balances (total, granted, topped-up)
//! per currency. The key is stored DPAPI-encrypted (see `secrets`) and only
//! ever sent to api.deepseek.com.

use crate::model::{ProviderReport, UsageWindow};
use std::path::Path;

pub const BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

/// Turn a `/user/balance` response body into a report. Pure, so it is
/// tested against the recorded response shape.
pub fn parse_response(body: &str, now: i64) -> ProviderReport {
    let parsed = serde_json::from_str::<serde_json::Value>(body);
    let v = match parsed {
        Ok(v) => v,
        Err(_) => {
            return ProviderReport::failed(
                "deepseek",
                "DeepSeek",
                now,
                "the server's answer could not be read".into(),
                false,
            )
        }
    };
    let Some(infos) = v.get("balance_infos").and_then(|x| x.as_array()) else {
        return ProviderReport::failed(
            "deepseek",
            "DeepSeek",
            now,
            "the server's answer had no balance block".into(),
            false,
        );
    };
    let available = v
        .get("is_available")
        .and_then(|x| x.as_bool())
        .unwrap_or(true);
    let money = |x: &serde_json::Value, k: &str| -> Option<f64> {
        x.get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| x.get(k).and_then(|v| v.as_f64()))
    };
    let mut windows = Vec::new();
    for info in infos {
        let currency = info
            .get("currency")
            .and_then(|c| c.as_str())
            .unwrap_or("USD");
        let total = money(info, "total_balance").unwrap_or(0.0);
        let granted = money(info, "granted_balance").unwrap_or(0.0);
        let topped = money(info, "topped_up_balance").unwrap_or(0.0);
        let symbol = if currency == "CNY" { "¥" } else { "$" };
        windows.push(UsageWindow {
            label: format!("Balance ({currency})"),
            detail: format!(
                "{symbol}{total:.2} total · {symbol}{granted:.2} granted · {symbol}{topped:.2} topped up{}",
                if available { "" } else { " · account marked unavailable" }
            ),
            unit: "USD".to_string(),
            ..Default::default()
        });
    }
    if windows.is_empty() {
        return ProviderReport::failed(
            "deepseek",
            "DeepSeek",
            now,
            "the account reports no balances".into(),
            false,
        );
    }
    ProviderReport {
        id: "deepseek".to_string(),
        name: "DeepSeek".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows,
        fetched_at: now,
    }
}

/// Collect with the stored key.
pub fn collect(data_dir: &Path, now: i64) -> ProviderReport {
    let Some(key) = crate::secrets::get_key(data_dir, "deepseek") else {
        return ProviderReport::failed(
            "deepseek",
            "DeepSeek",
            now,
            "no API key stored — add one with: codexbar-windows set-key deepseek".into(),
            false,
        );
    };
    match super::http::get_json(BALANCE_URL, Some(&key)) {
        Ok(body) => parse_response(&body, now),
        Err(e) => {
            let offline =
                e.contains("offline") || e.contains("could not reach") || e.contains("no answer");
            ProviderReport::failed("deepseek", "DeepSeek", now, e, offline)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_balance() {
        let body = r#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"12.34","granted_balance":"2.00","topped_up_balance":"10.34"}]}"#;
        let r = parse_response(body, 0);
        assert!(r.ok);
        assert!(r.windows[0].detail.contains("$12.34 total"));
        assert!(r.windows[0].detail.contains("$10.34 topped up"));
    }

    #[test]
    fn bad_body_is_honest_error() {
        assert!(!parse_response("nope", 0).ok);
        assert!(!parse_response("{}", 0).ok);
        assert!(!parse_response(r#"{"balance_infos":[]}"#, 0).ok);
    }
}

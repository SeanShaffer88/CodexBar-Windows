//! Moonshot / Kimi API: `GET https://api.moonshot.cn/v1/users/me/balance`
//! with the user's API key returns the account's real balances (available,
//! voucher, cash) in CNY. The key is stored DPAPI-encrypted (see
//! `secrets`) and only ever sent to api.moonshot.cn.

use crate::model::{ProviderReport, UsageWindow};
use std::path::Path;

pub const BALANCE_URL: &str = "https://api.moonshot.cn/v1/users/me/balance";

/// Turn a balance response body into a report. Pure, so it is tested
/// against the recorded response shape.
pub fn parse_response(body: &str, now: i64) -> ProviderReport {
    let parsed = serde_json::from_str::<serde_json::Value>(body);
    let data = match parsed {
        Ok(v) => match v.get("data") {
            Some(d) => d.clone(),
            None => {
                return ProviderReport::failed(
                    "moonshot",
                    "Moonshot",
                    now,
                    "the server's answer had no data block".into(),
                    false,
                )
            }
        },
        Err(_) => {
            return ProviderReport::failed(
                "moonshot",
                "Moonshot",
                now,
                "the server's answer could not be read".into(),
                false,
            )
        }
    };
    let num = |k: &str| data.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
    let (available, voucher, cash) = (
        num("available_balance"),
        num("voucher_balance"),
        num("cash_balance"),
    );
    let w = UsageWindow {
        label: "Balance (CNY)".to_string(),
        detail: format!("¥{available:.2} available · ¥{voucher:.2} voucher · ¥{cash:.2} cash"),
        unit: "USD".to_string(),
        ..Default::default()
    };
    ProviderReport {
        id: "moonshot".to_string(),
        name: "Moonshot".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows: vec![w],
        fetched_at: now,
    }
}

/// Collect with the stored key.
pub fn collect(data_dir: &Path, now: i64) -> ProviderReport {
    let Some(key) = crate::secrets::get_key(data_dir, "moonshot") else {
        return ProviderReport::failed(
            "moonshot",
            "Moonshot",
            now,
            "no API key stored — add one with: codexbar-windows set-key moonshot".into(),
            false,
        );
    };
    match super::http::get_json(BALANCE_URL, Some(&key)) {
        Ok(body) => parse_response(&body, now),
        Err(e) => {
            let offline =
                e.contains("offline") || e.contains("could not reach") || e.contains("no answer");
            ProviderReport::failed("moonshot", "Moonshot", now, e, offline)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_balance() {
        let body = r#"{"code":0,"data":{"available_balance":47.5,"voucher_balance":10.0,"cash_balance":37.5},"status":true}"#;
        let r = parse_response(body, 0);
        assert!(r.ok);
        assert!(r.windows[0].detail.contains("¥47.50 available"));
    }

    #[test]
    fn bad_body_is_honest_error() {
        assert!(!parse_response("nope", 0).ok);
        assert!(!parse_response("{}", 0).ok);
    }
}

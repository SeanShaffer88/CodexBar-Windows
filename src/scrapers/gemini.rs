//! Gemini: quota for the Gemini CLI, read the same way the upstream
//! project does — the CLI's own OAuth sign-in in `~/.gemini/oauth_creds.json`
//! plus Google's Code Assist quota service. Nothing is invented: no
//! sign-in file, no quota buckets, no report.
//!
//! The credentials file belongs to the Gemini CLI; this program only
//! reads it. When the access token has expired, the refresh token is
//! exchanged using the OAuth client values from the user's own Gemini
//! CLI install (the values are not baked into this program), and the
//! refreshed token is kept in memory only — the file is never rewritten.

use crate::model::{ProviderReport, UsageWindow};
use std::path::{Path, PathBuf};

pub const QUOTA_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
pub const LOAD_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Clone, PartialEq)]
pub struct Creds {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Token expiry, unix seconds (the file stores milliseconds).
    pub expiry: i64,
}

/// Parse `oauth_creds.json` text. Returns None when there is no usable
/// access token.
pub fn parse_creds(text: &str) -> Option<Creds> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let access_token = v.get("access_token")?.as_str()?.to_string();
    if access_token.is_empty() {
        return None;
    }
    let refresh_token = v
        .get("refresh_token")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let expiry = v
        .get("expiry_date")
        .and_then(|e| e.as_i64())
        .map(|ms| ms / 1000)
        .unwrap_or(0);
    Some(Creds {
        access_token,
        refresh_token,
        expiry,
    })
}

/// Percent-encode one form field value (application/x-www-form-urlencoded).
pub fn form_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The value assigned to a `NAME = '...'` constant in JavaScript source
/// (the Gemini CLI ships its OAuth client this way).
fn const_value(text: &str, name: &str) -> Option<String> {
    let at = text.find(name)?;
    let rest = &text[at + name.len()..];
    let eq = rest.find('=')?;
    let rest = &rest[eq + 1..];
    let q = rest.find(['\'', '"'])?;
    let quote = rest.as_bytes()[q];
    let rest = &rest[q + 1..];
    let end = rest.find(quote as char)?;
    let v = &rest[..end];
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// Extract (client_id, client_secret) from Gemini CLI source text.
pub fn extract_oauth_client(text: &str) -> Option<(String, String)> {
    let id = const_value(text, "OAUTH_CLIENT_ID")?;
    let secret = const_value(text, "OAUTH_CLIENT_SECRET")?;
    Some((id, secret))
}

fn oauth_client_from_file(path: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    extract_oauth_client(&text)
}

/// Find the OAuth client values: environment override first, then the
/// installed Gemini CLI's own source files (upstream's approach).
fn find_oauth_client() -> Option<(String, String)> {
    if let (Ok(id), Ok(secret)) = (
        std::env::var("GEMINI_OAUTH_CLIENT_ID"),
        std::env::var("GEMINI_OAUTH_CLIENT_SECRET"),
    ) {
        if !id.is_empty() && !secret.is_empty() {
            return Some((id, secret));
        }
    }
    if let Ok(p) = std::env::var("GEMINI_OAUTH2_JS_PATH") {
        if let Some(c) = oauth_client_from_file(Path::new(&p)) {
            return Some(c);
        }
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = crate::config::home_dir() {
        roots.push(home.join("AppData").join("Roaming").join("npm"));
    }
    if let Ok(path_var) = std::env::var("PATH") {
        roots.extend(std::env::split_paths(&path_var));
    }
    const RELS: [&str; 3] = [
        "node_modules/@google/gemini-cli/node_modules/@google/gemini-cli-core/dist/src/code_assist/oauth2.js",
        "node_modules/@google/gemini-cli-core/dist/src/code_assist/oauth2.js",
        "node_modules/@google/gemini-cli/bundle/gemini.js",
    ];
    for root in &roots {
        for rel in RELS {
            if let Some(c) = oauth_client_from_file(&root.join(rel)) {
                return Some(c);
            }
        }
    }
    None
}

/// Exchange the refresh token for a fresh access token.
fn refresh_access_token(creds: &Creds) -> Result<String, String> {
    let Some(refresh) = &creds.refresh_token else {
        return Err(
            "the Gemini sign-in has expired — run `gemini` once to sign in again".to_string(),
        );
    };
    let Some((client_id, client_secret)) = find_oauth_client() else {
        return Err("the Gemini sign-in has expired and the Gemini CLI install could not be found to refresh it — run `gemini` once to sign in again".to_string());
    };
    let body = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        form_encode(&client_id),
        form_encode(&client_secret),
        form_encode(refresh)
    );
    let resp = super::http::post_json(TOKEN_URL, None, "application/x-www-form-urlencoded", &body)
        .map_err(|e| format!("could not refresh the Gemini sign-in ({e})"))?;
    let v: serde_json::Value =
        serde_json::from_str(&resp).map_err(|_| "the sign-in answer could not be read")?;
    v.get("access_token")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| "the sign-in service returned no access token".to_string())
}

/// Plan label from a `loadCodeAssist` response: the paid tier's own
/// name when present, else the standard tier ids mapped to plain words.
pub fn tier_label(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(name) = v
        .get("paidTier")
        .and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
    {
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    match v
        .get("currentTier")
        .and_then(|t| t.get("id"))
        .and_then(|i| i.as_str())
    {
        Some("standard-tier") => Some("Paid".to_string()),
        Some("free-tier") => Some("Free".to_string()),
        Some("legacy-tier") => Some("Legacy".to_string()),
        _ => None,
    }
}

/// The quota project id from a `loadCodeAssist` response, when present.
pub fn project_id(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    match v.get("cloudaicompanionProject") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(obj) => obj
            .get("id")
            .and_then(|i| i.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// Turn a `retrieveUserQuota` response into a report: one window per
/// model family (Pro, Flash, Flash Lite), each carrying the family's
/// lowest remaining fraction, as upstream maps it.
pub fn parse_quota(body: &str, now: i64, tier: Option<&str>) -> ProviderReport {
    let parsed = serde_json::from_str::<serde_json::Value>(body);
    let v = match parsed {
        Ok(v) => v,
        Err(_) => {
            return ProviderReport::failed(
                "gemini",
                "Gemini",
                now,
                "the server's answer could not be read".into(),
                false,
            )
        }
    };
    let Some(buckets) = v.get("buckets").and_then(|b| b.as_array()) else {
        return ProviderReport::failed(
            "gemini",
            "Gemini",
            now,
            "the quota service returned no usage data".into(),
            false,
        );
    };
    // (family index, label) in display order.
    const FAMILIES: [(usize, &str); 3] = [
        (0, "Pro models"),
        (1, "Flash models"),
        (2, "Flash Lite models"),
    ];
    let mut best: [Option<(f64, Option<i64>)>; 3] = [None, None, None];
    for b in buckets {
        let Some(model) = b.get("modelId").and_then(|m| m.as_str()) else {
            continue;
        };
        let Some(frac) = b.get("remainingFraction").and_then(|f| f.as_f64()) else {
            continue;
        };
        let lower = model.to_lowercase();
        let fam = if lower.contains("flash-lite") || lower.contains("flash_lite") {
            2
        } else if lower.contains("flash") {
            1
        } else if lower.contains("pro") {
            0
        } else {
            continue;
        };
        let reset = b
            .get("resetTime")
            .and_then(|r| r.as_str())
            .and_then(crate::timeutil::parse_iso8601);
        if best[fam].map(|(f, _)| frac < f).unwrap_or(true) {
            best[fam] = Some((frac, reset));
        }
    }
    let mut windows = Vec::new();
    for (idx, label) in FAMILIES {
        if let Some((frac, reset)) = best[idx] {
            windows.push(UsageWindow {
                label: label.to_string(),
                used_percent: Some(((1.0 - frac) * 100.0).clamp(0.0, 100.0)),
                resets_at: reset,
                detail: if windows.is_empty() {
                    tier.map(|t| format!("plan: {t}")).unwrap_or_default()
                } else {
                    String::new()
                },
                ..Default::default()
            });
        }
    }
    if windows.is_empty() {
        return ProviderReport::failed(
            "gemini",
            "Gemini",
            now,
            "the quota service returned no model quotas".into(),
            false,
        );
    }
    ProviderReport {
        id: "gemini".to_string(),
        name: "Gemini".to_string(),
        ok: true,
        offline: false,
        error: None,
        windows,
        fetched_at: now,
    }
}

fn network_offline(e: &str) -> bool {
    e.contains("offline") || e.contains("could not reach") || e.contains("no answer")
}

/// Collect with the Gemini CLI's local sign-in. `dir` is `~/.gemini`.
pub fn collect(dir: &Path, now: i64) -> ProviderReport {
    let path = dir.join("oauth_creds.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return ProviderReport::failed(
            "gemini",
            "Gemini",
            now,
            "Gemini CLI is not signed in on this PC — run `gemini` once to sign in".into(),
            false,
        );
    };
    let Some(creds) = parse_creds(&text) else {
        return ProviderReport::failed(
            "gemini",
            "Gemini",
            now,
            "the Gemini CLI sign-in file could not be read — run `gemini` to sign in again".into(),
            false,
        );
    };
    let mut token = creds.access_token.clone();
    if creds.expiry > 0 && creds.expiry <= now + 60 {
        match refresh_access_token(&creds) {
            Ok(t) => token = t,
            Err(e) => {
                let offline = network_offline(&e);
                return ProviderReport::failed("gemini", "Gemini", now, e, offline);
            }
        }
    }
    // Tier + project (best effort: the quota call works without them).
    let mut tier = None;
    let mut project = None;
    if let Ok(body) = super::http::post_json(
        LOAD_URL,
        Some(&token),
        "application/json",
        r#"{"metadata":{"ideType":"GEMINI_CLI","pluginType":"GEMINI"}}"#,
    ) {
        tier = tier_label(&body);
        project = project_id(&body);
    }
    let qbody = match &project {
        Some(p) => format!(r#"{{"project":"{}"}}"#, p.replace('"', "")),
        None => "{}".to_string(),
    };
    match super::http::post_json(QUOTA_URL, Some(&token), "application/json", &qbody) {
        Ok(body) => parse_quota(&body, now, tier.as_deref()),
        Err(e) => {
            if e == "status 403" {
                return ProviderReport::failed(
                    "gemini",
                    "Gemini",
                    now,
                    "Google refused the quota request (403) — since June 2026 the Gemini CLI quota service no longer covers individual accounts; Workspace and Code Assist accounts still work".into(),
                    false,
                );
            }
            let offline = network_offline(&e);
            ProviderReport::failed(
                "gemini",
                "Gemini",
                now,
                format!("the quota service could not be reached ({e})"),
                offline,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_creds() {
        let c = parse_creds(
            r#"{"access_token":"ya29.x","refresh_token":"1//r","expiry_date":1893456000000}"#,
        )
        .unwrap();
        assert_eq!(c.access_token, "ya29.x");
        assert_eq!(c.refresh_token.as_deref(), Some("1//r"));
        assert_eq!(c.expiry, 1_893_456_000);
        assert!(parse_creds("{}").is_none());
        assert!(parse_creds("nope").is_none());
    }

    #[test]
    fn form_encoding_escapes_separators() {
        assert_eq!(form_encode("a b+c/d="), "a+b%2Bc%2Fd%3D");
        assert_eq!(form_encode("plain-1.0_x~"), "plain-1.0_x~");
    }

    #[test]
    fn extracts_oauth_client_from_source() {
        let src = "export const OAUTH_CLIENT_ID = '123-abc.apps.googleusercontent.com';\n\
                   export const OAUTH_CLIENT_SECRET = \"GOCSPX-fake\";\n";
        let (id, secret) = extract_oauth_client(src).unwrap();
        assert_eq!(id, "123-abc.apps.googleusercontent.com");
        assert_eq!(secret, "GOCSPX-fake");
        assert!(extract_oauth_client("nothing here").is_none());
    }

    #[test]
    fn tier_and_project_parse() {
        let body =
            r#"{"currentTier":{"id":"free-tier"},"cloudaicompanionProject":"gen-lang-client-1"}"#;
        assert_eq!(tier_label(body).as_deref(), Some("Free"));
        assert_eq!(project_id(body).as_deref(), Some("gen-lang-client-1"));
        let paid = r#"{"paidTier":{"name":"Google One AI Premium"},"cloudaicompanionProject":{"id":"p2"}}"#;
        assert_eq!(tier_label(paid).as_deref(), Some("Google One AI Premium"));
        assert_eq!(project_id(paid).as_deref(), Some("p2"));
    }

    #[test]
    fn quota_groups_by_family_lowest_wins() {
        let body = r#"{"buckets":[
            {"modelId":"gemini-2.5-pro","remainingFraction":0.8,"resetTime":"2030-01-01T00:00:00Z"},
            {"modelId":"gemini-2.5-pro-preview","remainingFraction":0.5,"resetTime":"2030-01-01T00:00:00Z"},
            {"modelId":"gemini-2.5-flash","remainingFraction":0.25,"resetTime":"2030-01-01T00:00:00Z"},
            {"modelId":"gemini-2.5-flash-lite","remainingFraction":1.0,"resetTime":"2030-01-01T00:00:00Z"}
        ]}"#;
        let r = parse_quota(body, 0, Some("Free"));
        assert!(r.ok);
        assert_eq!(r.windows.len(), 3);
        assert_eq!(r.windows[0].label, "Pro models");
        assert_eq!(r.windows[0].used_percent, Some(50.0));
        assert_eq!(r.windows[0].detail, "plan: Free");
        assert_eq!(r.windows[1].used_percent, Some(75.0));
        assert_eq!(r.windows[2].used_percent, Some(0.0));
        assert!(r.windows[0].resets_at.is_some());
    }

    #[test]
    fn quota_without_buckets_is_honest_error() {
        assert!(!parse_quota("{}", 0, None).ok);
        assert!(!parse_quota("nope", 0, None).ok);
        assert!(!parse_quota(r#"{"buckets":[]}"#, 0, None).ok);
    }
}

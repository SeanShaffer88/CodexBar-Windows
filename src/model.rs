//! Shared data model: one report per provider, each holding usage windows
//! (a 5-hour session window, a weekly window, a credit balance, ...).

use serde::{Deserialize, Serialize};

/// One usage window or balance for a provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageWindow {
    /// Short label, e.g. "Session (5h)", "Weekly", "Credits".
    pub label: String,
    /// Percent of the window used, 0-100, when the source reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    /// Amount used, in `unit` (tokens or USD), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<f64>,
    /// Window limit, in `unit`, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// "tokens" or "USD".
    #[serde(default)]
    pub unit: String,
    /// Unix seconds when the window resets, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
    /// Extra plain-language detail, e.g. "1,234,567 tokens today".
    #[serde(default)]
    pub detail: String,
}

impl UsageWindow {
    /// Percent remaining, when a used percent is known.
    pub fn remaining_percent(&self) -> Option<f64> {
        self.used_percent.map(|p| (100.0 - p).clamp(0.0, 100.0))
    }

    /// One-line human rendering, e.g. "Session (5h): 42% used, resets in 2h 10m".
    pub fn describe(&self, now: i64) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(p) = self.used_percent {
            let left = self.remaining_percent().unwrap_or(0.0);
            parts.push(format!("{p:.0}% used ({left:.0}% left)"));
        }
        if let (Some(u), Some(l)) = (self.used, self.limit) {
            if self.unit == "USD" {
                parts.push(format!("${u:.2} of ${l:.2}"));
            } else {
                parts.push(format!("{} of {}", fmt_num(u), fmt_num(l)));
            }
        } else if let Some(u) = self.used {
            if self.unit == "USD" {
                parts.push(format!("${u:.2}"));
            } else {
                parts.push(format!("{} {}", fmt_num(u), self.unit));
            }
        }
        if !self.detail.is_empty() {
            parts.push(self.detail.clone());
        }
        if let Some(r) = self.resets_at {
            parts.push(format!("resets in {}", fmt_duration(r - now)));
        }
        if parts.is_empty() {
            format!("{}: no data", self.label)
        } else {
            format!("{}: {}", self.label, parts.join(", "))
        }
    }
}

/// The full picture for one provider at one moment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderReport {
    /// Stable id: "codex", "claude-code", "openrouter".
    pub id: String,
    /// Display name.
    pub name: String,
    /// True when the source produced data.
    pub ok: bool,
    /// True when the failure looks like a network/offline failure, so the
    /// cached copy (if any) should be shown with an Offline badge.
    #[serde(default)]
    pub offline: bool,
    /// Plain-language problem description when `ok` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub windows: Vec<UsageWindow>,
    /// Unix seconds when this report was collected.
    pub fetched_at: i64,
}

impl ProviderReport {
    pub fn failed(id: &str, name: &str, now: i64, error: String, offline: bool) -> Self {
        ProviderReport {
            id: id.to_string(),
            name: name.to_string(),
            ok: false,
            offline,
            error: Some(error),
            windows: Vec::new(),
            fetched_at: now,
        }
    }

    /// Highest used-percent across windows — the number the tray icon shows.
    pub fn worst_percent(&self) -> Option<f64> {
        self.windows
            .iter()
            .filter_map(|w| w.used_percent)
            .fold(None::<f64>, |acc, p| Some(acc.map_or(p, |a| a.max(p))))
    }
}

/// Tray badge for a worst-window percent: the number to draw and its
/// RGB color (green under 70, amber 70–89, red 90+ — the same bands the
/// tray bar glyph used). None when there is no percent to show; the
/// tray then draws its neutral bar glyph.
/// (Consumed by the Windows tray; exercised directly by tests elsewhere.)
#[cfg_attr(not(windows), allow(dead_code))]
pub fn tray_badge(percent: Option<f64>) -> Option<(String, [u8; 3])> {
    let p = percent?;
    let color = if p >= 90.0 {
        [230, 60, 50]
    } else if p >= 70.0 {
        [240, 170, 40]
    } else {
        [60, 200, 110]
    };
    Some((format!("{:.0}", p.clamp(0.0, 100.0)), color))
}

/// Format a token count compactly: 950, 12.3K, 1.2M.
pub fn fmt_num(n: f64) -> String {
    let a = n.abs();
    if a >= 1_000_000.0 {
        format!("{:.1}M", n / 1_000_000.0)
    } else if a >= 10_000.0 {
        format!("{:.1}K", n / 1_000.0)
    } else if a >= 1_000.0 {
        format!("{:.2}K", n / 1_000.0)
    } else {
        format!("{n:.0}")
    }
}

/// Format a duration in seconds as "2h 10m", "3d 4h", "12m", or "now".
pub fn fmt_duration(secs: i64) -> String {
    if secs <= 0 {
        return "now".to_string();
    }
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_clamps() {
        let w = UsageWindow {
            used_percent: Some(130.0),
            ..Default::default()
        };
        assert_eq!(w.remaining_percent(), Some(0.0));
        let w = UsageWindow {
            used_percent: Some(25.0),
            ..Default::default()
        };
        assert_eq!(w.remaining_percent(), Some(75.0));
    }

    #[test]
    fn tray_badge_bands_and_rounding() {
        assert_eq!(tray_badge(None), None);
        assert_eq!(tray_badge(Some(42.4)), Some(("42".into(), [60, 200, 110])));
        assert_eq!(tray_badge(Some(70.0)), Some(("70".into(), [240, 170, 40])));
        assert_eq!(tray_badge(Some(95.0)), Some(("95".into(), [230, 60, 50])));
        assert_eq!(tray_badge(Some(130.0)), Some(("100".into(), [230, 60, 50])));
    }

    #[test]
    fn fmt_num_scales() {
        assert_eq!(fmt_num(950.0), "950");
        assert_eq!(fmt_num(12_300.0), "12.3K");
        assert_eq!(fmt_num(1_250_000.0), "1.2M");
    }

    #[test]
    fn worst_percent_picks_max() {
        let r = ProviderReport {
            id: "x".into(),
            name: "X".into(),
            ok: true,
            offline: false,
            error: None,
            windows: vec![
                UsageWindow {
                    used_percent: Some(10.0),
                    ..Default::default()
                },
                UsageWindow {
                    used_percent: Some(80.0),
                    ..Default::default()
                },
            ],
            fetched_at: 0,
        };
        assert_eq!(r.worst_percent(), Some(80.0));
    }
}

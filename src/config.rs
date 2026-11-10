//! Configuration and portable storage layout.
//!
//! Everything the program writes lives in a `data/` directory beside the
//! executable, so the whole app is portable. The one registry touch is
//! opt-in: turning on "start with Windows" writes the standard per-user
//! Run entry, and turning it off removes it.
//! `config.json` carries a `schema_version`; older files are migrated upward
//! field by field and the original is backed up before any rewrite.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Providers {
    #[serde(default = "yes")]
    pub codex: bool,
    #[serde(default = "yes")]
    pub claude_code: bool,
    #[serde(default = "yes")]
    pub openrouter: bool,
    /// Off until a key is stored: `set-key deepseek` turns it on.
    #[serde(default)]
    pub deepseek: bool,
    /// Off until a key is stored: `set-key moonshot` turns it on.
    #[serde(default)]
    pub moonshot: bool,
    /// Off until an Admin key is stored: `set-key openai` turns it on.
    #[serde(default)]
    pub openai: bool,
    /// Off until an Admin key is stored: `set-key anthropic` turns it on.
    #[serde(default)]
    pub anthropic: bool,
}

fn yes() -> bool {
    true
}

impl Default for Providers {
    fn default() -> Self {
        Providers {
            codex: true,
            claude_code: true,
            openrouter: true,
            deepseek: false,
            moonshot: false,
            openai: false,
            anthropic: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub schema_version: u32,
    /// Minutes between automatic refreshes in the tray (5-120).
    #[serde(default = "default_refresh")]
    pub refresh_minutes: u32,
    #[serde(default)]
    pub providers: Providers,
    /// Tray warning balloon threshold, percent used (50-100). Schema 2.
    #[serde(default = "default_warn")]
    pub warn_threshold_percent: u32,
    /// Start the tray app when Windows starts (per-user Run entry). Schema 2.
    #[serde(default)]
    pub start_with_windows: bool,
    /// Optional override for the Claude Code data directory
    /// (defaults to `~/.claude`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_dir: Option<String>,
    /// Optional override for the Codex data directory (defaults to `~/.codex`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_dir: Option<String>,
}

fn default_refresh() -> u32 {
    15
}

fn default_warn() -> u32 {
    90
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema_version: SCHEMA_VERSION,
            refresh_minutes: default_refresh(),
            warn_threshold_percent: default_warn(),
            start_with_windows: false,
            providers: Providers::default(),
            claude_dir: None,
            codex_dir: None,
        }
    }
}

impl Config {
    /// Migrate a parsed config of any older schema up to the current one.
    /// Unknown/missing fields fall back to defaults via serde; the version
    /// stamp is then raised. A file from a *newer* schema is refused so a
    /// downgrade never silently drops settings.
    pub fn migrate(mut self) -> Result<Self, String> {
        if self.schema_version > SCHEMA_VERSION {
            return Err(format!(
                "config was written by a newer version (schema {} > {}); update CodexBar-Windows",
                self.schema_version, SCHEMA_VERSION
            ));
        }
        self.schema_version = SCHEMA_VERSION;
        self.refresh_minutes = self.refresh_minutes.clamp(5, 120);
        self.warn_threshold_percent = self.warn_threshold_percent.clamp(50, 100);
        Ok(self)
    }
}

/// The `data/` directory beside the executable.
pub fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let base = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("data")
}

/// Test/dev override: `CODEXBAR_DATA_DIR` wins when set.
pub fn data_dir_env() -> PathBuf {
    match std::env::var_os("CODEXBAR_DATA_DIR") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => data_dir(),
    }
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

/// Load the config from `dir`, creating defaults on first run. A malformed
/// file is quarantined to `config.json.bad` and defaults are used, with the
/// problem reported in the returned note.
pub fn load(dir: &Path) -> (Config, Option<String>) {
    let path = config_path(dir);
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => {
            let cfg = Config::default();
            let _ = save(dir, &cfg);
            return (cfg, None);
        }
    };
    match serde_json::from_str::<Config>(&raw) {
        Ok(cfg) => match cfg.migrate() {
            Ok(m) => {
                if m.schema_version != 0 && !raw.contains("\"schema_version\"") {
                    let _ = save(dir, &m);
                }
                (m, None)
            }
            Err(e) => (Config::default(), Some(e)),
        },
        Err(e) => {
            let bad = dir.join("config.json.bad");
            let _ = std::fs::rename(&path, &bad);
            let cfg = Config::default();
            let _ = save(dir, &cfg);
            (cfg, Some(format!("config.json was unreadable ({e}); moved to config.json.bad and defaults restored")))
        }
    }
}

/// Save atomically: write a temp file, then rename over the target. The
/// previous file is copied to `config.json.bak` first.
pub fn save(dir: &Path, cfg: &Config) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create data folder: {e}"))?;
    let path = config_path(dir);
    if path.exists() {
        let _ = std::fs::copy(&path, dir.join("config.json.bak"));
    }
    let tmp = dir.join("config.json.tmp");
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write config: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("cannot replace config: {e}"))
}

/// User home directory from the environment (USERPROFILE, then HOME).
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_current_schema() {
        let c = Config::default();
        assert_eq!(c.schema_version, SCHEMA_VERSION);
        assert_eq!(c.refresh_minutes, 15);
        assert!(c.providers.codex && c.providers.claude_code && c.providers.openrouter);
    }

    #[test]
    fn migrate_clamps_and_stamps() {
        let c: Config =
            serde_json::from_str(r#"{"schema_version":0,"refresh_minutes":9999}"#).unwrap();
        let m = c.migrate().unwrap();
        assert_eq!(m.schema_version, SCHEMA_VERSION);
        assert_eq!(m.refresh_minutes, 120);
        assert!(m.providers.claude_code);
    }

    #[test]
    fn migrate_v1_gains_schema2_fields() {
        let c: Config =
            serde_json::from_str(r#"{"schema_version":1,"refresh_minutes":20}"#).unwrap();
        let m = c.migrate().unwrap();
        assert_eq!(m.schema_version, 2);
        assert_eq!(m.refresh_minutes, 20);
        assert_eq!(m.warn_threshold_percent, 90);
        assert!(!m.start_with_windows);
    }

    #[test]
    fn migrate_refuses_newer() {
        let c: Config = serde_json::from_str(r#"{"schema_version":99}"#).unwrap();
        assert!(c.migrate().is_err());
    }

    #[test]
    fn load_save_round_trip_and_quarantine() {
        let dir = std::env::temp_dir().join(format!("cbw-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (cfg, note) = load(&dir);
        assert!(note.is_none());
        assert_eq!(cfg, Config::default());
        std::fs::write(config_path(&dir), "{ not json").unwrap();
        let (cfg2, note2) = load(&dir);
        assert_eq!(cfg2, Config::default());
        assert!(note2.is_some());
        assert!(dir.join("config.json.bad").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

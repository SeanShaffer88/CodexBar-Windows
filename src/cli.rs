//! Command-line interface. Every tray capability has a command here:
//! refresh, status, history, key management, and settings.

use crate::{config, ops, secrets, store, timeutil};
use std::io::Read;
use std::path::Path;

pub const HELP: &str = "\
CodexBar-Windows — AI coding limits in your tray.

Usage:
  codexbar-windows                 Start the tray app (same as `tray`)
  codexbar-windows tray            Start the tray app
  codexbar-windows status          Show the last collected numbers
  codexbar-windows status --json   Same, as JSON
  codexbar-windows status --provider codex
                                   Only one provider (also on refresh)
  codexbar-windows refresh         Collect fresh numbers and save them
  codexbar-windows refresh --json  Same, as JSON
  codexbar-windows history         List saved snapshots (newest last)
  codexbar-windows report          Day-by-day summary from history
  codexbar-windows report --days 30 --json
                                   Same, more days, as JSON
  codexbar-windows clear-history   Move history to history.json.bak
  codexbar-windows providers       List providers, on/off, and key state
  codexbar-windows set-key <openrouter|deepseek|moonshot|openai|anthropic>
                                   Read an API key from stdin and store it
                                   encrypted (Windows DPAPI); turns the
                                   provider on. openai and anthropic need
                                   Admin keys (org usage endpoints)
  codexbar-windows clear-key <openrouter|deepseek|moonshot|openai|anthropic>
                                   Remove the stored API key; turns the
                                   provider off
  codexbar-windows settings        Open the settings window (Windows);
                                   with options below, change settings
                                   from the command line instead
  codexbar-windows settings --refresh <minutes 5-120>
  codexbar-windows settings --warn <percent 50-100>
  codexbar-windows settings --autostart <on|off>
  codexbar-windows settings --json Show settings as JSON
  codexbar-windows config          Show the settings file contents
  codexbar-windows config refresh <minutes>
                                   Set the tray refresh interval (5-120)
  codexbar-windows data-dir        Print the portable data folder path
  codexbar-windows clean           Delete settings, keys, and history
  codexbar-windows --version       Print the version
  codexbar-windows --help          This help

How it works: Codex and Claude Code numbers come from the log files those
tools already write on this PC. OpenRouter numbers come from OpenRouter's
own key-usage endpoint, using your stored key. Nothing is uploaded anywhere
else, and there is no telemetry.
";

pub fn run(args: &[String]) -> i32 {
    let dir = config::data_dir_env();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "" | "tray" => crate::tray::run(),
        "status" => status(
            &dir,
            args.iter().any(|a| a == "--json"),
            provider_filter(args),
        ),
        "refresh" => refresh_cmd(
            &dir,
            args.iter().any(|a| a == "--json"),
            provider_filter(args),
        ),
        "history" => history_cmd(&dir, args.iter().any(|a| a == "--json")),
        "report" => report_cmd(&dir, args),
        "clear-history" => match store::clear(&dir) {
            Ok(()) => {
                println!("History moved to history.json.bak (rename it back to undo).");
                0
            }
            Err(e) => fail(&e),
        },
        "providers" => providers_cmd(&dir),
        "set-key" => set_key(&dir, args),
        "clear-key" => clear_key(&dir, args),
        "config" => config_cmd(&dir, args),
        "settings" => settings_cmd(&dir, args),
        "data-dir" => {
            println!("{}", dir.display());
            0
        }
        "clean" | "--clean" | "--uninstall" => clean(&dir),
        "--version" | "-V" => {
            println!("codexbar-windows {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "--help" | "-h" | "help" => {
            print!("{HELP}");
            0
        }
        other => {
            eprintln!("Unknown command: {other}\n\n{HELP}");
            2
        }
    }
}

fn fail(msg: &str) -> i32 {
    eprintln!("{msg}");
    1
}

fn load_cfg(dir: &Path) -> config::Config {
    let (cfg, note) = config::load(dir);
    if let Some(n) = note {
        eprintln!("Note: {n}");
    }
    cfg
}

/// The `--provider <id>` value, when present.
fn provider_filter(args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == "--provider")
        .map(|w| w[1].clone())
}

fn only(
    reports: &[crate::model::ProviderReport],
    filter: &Option<String>,
) -> Vec<crate::model::ProviderReport> {
    match filter {
        Some(id) => reports.iter().filter(|r| &r.id == id).cloned().collect(),
        None => reports.to_vec(),
    }
}

fn status(dir: &Path, json: bool, filter: Option<String>) -> i32 {
    match store::latest(dir) {
        Some(snap) => {
            let reports = only(&snap.reports, &filter);
            if reports.is_empty() {
                println!("No saved reading for that provider yet. Run `refresh`.");
                return 0;
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&reports).unwrap_or_default()
                );
            } else {
                print!("{}", ops::render_text(&reports, timeutil::now_unix()));
                println!(
                    "(last collected {}; run `refresh` for fresh numbers)",
                    timeutil::format_utc(snap.at)
                );
            }
            0
        }
        None => {
            println!("Nothing collected yet — running a first refresh:");
            refresh_cmd(dir, json, filter)
        }
    }
}

fn refresh_cmd(dir: &Path, json: bool, filter: Option<String>) -> i32 {
    let cfg = load_cfg(dir);
    let now = timeutil::now_unix();
    let all = ops::refresh(&cfg, dir, now);
    let reports = only(&all, &filter);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_default()
        );
    } else {
        print!("{}", ops::render_text(&reports, now));
    }
    0
}

fn history_cmd(dir: &Path, json: bool) -> i32 {
    let snaps = store::all(dir);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snaps).unwrap_or_default()
        );
        return 0;
    }
    if snaps.is_empty() {
        println!("No history yet. Run `refresh` to start collecting.");
        return 0;
    }
    for s in &snaps {
        let names: Vec<String> = s
            .reports
            .iter()
            .map(|r| match r.worst_percent() {
                Some(p) => format!("{} {p:.0}%", r.name),
                None => r.name.clone(),
            })
            .collect();
        println!("{} — {}", timeutil::format_utc(s.at), names.join(" · "));
    }
    0
}

fn set_provider_enabled(dir: &Path, provider: &str, on: bool) {
    let mut cfg = load_cfg(dir);
    match provider {
        "openrouter" => cfg.providers.openrouter = on,
        "deepseek" => cfg.providers.deepseek = on,
        "moonshot" => cfg.providers.moonshot = on,
        "openai" => cfg.providers.openai = on,
        "anthropic" => cfg.providers.anthropic = on,
        _ => return,
    }
    let _ = config::save(dir, &cfg);
}

fn providers_cmd(dir: &Path) -> i32 {
    let cfg = load_cfg(dir);
    let key_state = |p: &str| {
        if secrets::KEY_PROVIDERS.contains(&p) {
            if secrets::get_key(dir, p).is_some() {
                "key stored"
            } else {
                "no key stored"
            }
        } else {
            "local logs"
        }
    };
    let rows = [
        ("codex", "Codex", cfg.providers.codex),
        ("claude-code", "Claude Code", cfg.providers.claude_code),
        ("openrouter", "OpenRouter", cfg.providers.openrouter),
        ("deepseek", "DeepSeek", cfg.providers.deepseek),
        ("moonshot", "Moonshot", cfg.providers.moonshot),
        ("openai", "OpenAI API", cfg.providers.openai),
        ("anthropic", "Anthropic API", cfg.providers.anthropic),
    ];
    for (id, name, on) in rows {
        println!(
            "{name:<12} {:<3}  {}",
            if on { "on" } else { "off" },
            key_state(id)
        );
    }
    0
}

fn set_key(dir: &Path, args: &[String]) -> i32 {
    let provider = args.get(1).map(|s| s.as_str()).unwrap_or("");
    if !secrets::KEY_PROVIDERS.contains(&provider) {
        return fail(&format!(
            "Usage: codexbar-windows set-key <{}>",
            secrets::KEY_PROVIDERS.join("|")
        ));
    }
    eprintln!("Paste the {provider} API key, then press Enter:");
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        let _ = std::io::stdin().read_to_string(&mut input);
    }
    match secrets::set_key(dir, provider, &input) {
        Ok(()) => {
            set_provider_enabled(dir, provider, true);
            println!("Key stored, encrypted with Windows DPAPI. The {provider} provider is on.");
            0
        }
        Err(e) => fail(&e),
    }
}

fn clear_key(dir: &Path, args: &[String]) -> i32 {
    let provider = args.get(1).map(|s| s.as_str()).unwrap_or("");
    if !secrets::KEY_PROVIDERS.contains(&provider) {
        return fail(&format!(
            "Usage: codexbar-windows clear-key <{}>",
            secrets::KEY_PROVIDERS.join("|")
        ));
    }
    match secrets::clear_key(dir, provider) {
        Ok(()) => {
            set_provider_enabled(dir, provider, false);
            println!("Stored {provider} key removed. The provider is off.");
            0
        }
        Err(e) => fail(&e),
    }
}

/// Day-by-day summary built from the saved history: for each of the last
/// `--days` days (default 7), the day's last snapshot, one line per
/// provider. Days are local days.
fn report_cmd(dir: &Path, args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let days: i64 = argval(args, "--days")
        .and_then(|v| v.parse().ok())
        .unwrap_or(7)
        .clamp(1, 90);
    let snaps = store::all(dir);
    if snaps.is_empty() {
        println!("No history yet. Run `refresh` to start collecting.");
        return 0;
    }
    let offset = timeutil::local_utc_offset_seconds();
    let cutoff = timeutil::local_day_start(timeutil::now_unix(), offset) - (days - 1) * 86_400;
    let mut by_day: std::collections::BTreeMap<i64, &store::Snapshot> =
        std::collections::BTreeMap::new();
    for s in &snaps {
        let d = timeutil::local_day_start(s.at, offset);
        if d >= cutoff {
            by_day.insert(d, s);
        }
    }
    if by_day.is_empty() {
        println!("No snapshots in the last {days} day(s).");
        return 0;
    }
    if json {
        let arr: Vec<serde_json::Value> = by_day
            .iter()
            .map(|(day, snap)| {
                serde_json::json!({
                    "date": &timeutil::format_utc(day + offset)[..10],
                    "at": snap.at,
                    "providers": snap.reports.iter().map(|r| {
                        serde_json::json!({
                            "id": r.id, "name": r.name, "ok": r.ok, "error": r.error,
                            "windows": r.windows.iter().map(|w| serde_json::json!({
                                "label": w.label, "used_percent": w.used_percent,
                                "used": w.used, "unit": w.unit, "detail": w.detail
                            })).collect::<Vec<_>>()
                        })
                    }).collect::<Vec<_>>()
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return 0;
    }
    for (day, snap) in &by_day {
        println!("{}", &timeutil::format_utc(day + offset)[..10]);
        for r in &snap.reports {
            if !r.ok {
                println!("  {} - {}", r.name, r.error.as_deref().unwrap_or("no data"));
                continue;
            }
            let parts: Vec<String> = r.windows.iter().map(|w| w.describe(snap.at)).collect();
            println!("  {} - {}", r.name, parts.join("; "));
        }
    }
    0
}

fn argval(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

fn settings_cmd(dir: &Path, args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let refresh = argval(args, "--refresh");
    let warn = argval(args, "--warn");
    let autostart = argval(args, "--autostart");
    let has_setters = refresh.is_some() || warn.is_some() || autostart.is_some();
    if !has_setters && !json {
        // No options: the settings window. Off Windows, print instead.
        #[cfg(windows)]
        {
            return crate::settings::open(None);
        }
        #[cfg(not(windows))]
        {
            return show_settings(dir, false);
        }
    }
    if has_setters {
        let mut cfg = load_cfg(dir);
        if let Some(v) = &refresh {
            match v.parse::<u32>() {
                Ok(n) if (5..=120).contains(&n) => cfg.refresh_minutes = n,
                _ => return fail("Usage: settings --refresh <minutes 5-120>"),
            }
        }
        if let Some(v) = &warn {
            match v.parse::<u32>() {
                Ok(n) if (50..=100).contains(&n) => cfg.warn_threshold_percent = n,
                _ => return fail("Usage: settings --warn <percent 50-100>"),
            }
        }
        if let Some(v) = &autostart {
            let on = match v.as_str() {
                "on" => true,
                "off" => false,
                _ => return fail("Usage: settings --autostart <on|off>"),
            };
            if let Err(e) = ops::set_autostart(on) {
                return fail(&e);
            }
            cfg.start_with_windows = on;
        }
        if let Err(e) = config::save(dir, &cfg) {
            return fail(&e);
        }
        println!("Settings saved. A running tray picks changes up on its next refresh, except the interval, which applies when the tray restarts or saves from its Settings window.");
    }
    show_settings(dir, json)
}

fn show_settings(dir: &Path, json: bool) -> i32 {
    let cfg = load_cfg(dir);
    if json {
        let key = |p: &str| secrets::get_key(dir, p).is_some();
        let v = serde_json::json!({
            "refresh_minutes": cfg.refresh_minutes,
            "warn_threshold_percent": cfg.warn_threshold_percent,
            "start_with_windows_configured": cfg.start_with_windows,
            "start_with_windows_active": ops::autostart_enabled(),
            "data_dir": dir.to_string_lossy(),
            "providers": {
                "codex": { "on": cfg.providers.codex, "key": "local logs" },
                "claude-code": { "on": cfg.providers.claude_code, "key": "local logs" },
                "openrouter": { "on": cfg.providers.openrouter, "key_stored": key("openrouter") },
                "deepseek": { "on": cfg.providers.deepseek, "key_stored": key("deepseek") },
                "moonshot": { "on": cfg.providers.moonshot, "key_stored": key("moonshot") },
                "openai": { "on": cfg.providers.openai, "key_stored": key("openai") },
                "anthropic": { "on": cfg.providers.anthropic, "key_stored": key("anthropic") },
            }
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return 0;
    }
    println!("Refresh every:        {} minutes", cfg.refresh_minutes);
    println!("Warn at:              {}% used", cfg.warn_threshold_percent);
    println!(
        "Start with Windows:   {} (entry currently {})",
        if cfg.start_with_windows { "on" } else { "off" },
        if ops::autostart_enabled() {
            "present"
        } else {
            "absent"
        }
    );
    println!("Data folder:          {}", dir.display());
    println!();
    providers_cmd(dir)
}

fn config_cmd(dir: &Path, args: &[String]) -> i32 {
    match args.get(1).map(|s| s.as_str()) {
        None => {
            let cfg = load_cfg(dir);
            println!("{}", serde_json::to_string_pretty(&cfg).unwrap_or_default());
            println!("(file: {})", config::config_path(dir).display());
            0
        }
        Some("refresh") => {
            let Some(v) = args.get(2).and_then(|s| s.parse::<u32>().ok()) else {
                return fail("Usage: codexbar-windows config refresh <minutes 5-120>");
            };
            let mut cfg = load_cfg(dir);
            cfg.refresh_minutes = v.clamp(5, 120);
            match config::save(dir, &cfg) {
                Ok(()) => {
                    println!(
                        "Refresh interval set to {} minutes. Restart the tray to apply.",
                        cfg.refresh_minutes
                    );
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => fail("Usage: codexbar-windows config [refresh <minutes>]"),
    }
}

fn clean(dir: &Path) -> i32 {
    let mut removed = 0;
    for name in ["config.json", "secrets.json", "history.json"] {
        let p = dir.join(name);
        if p.exists() && std::fs::remove_file(&p).is_ok() {
            removed += 1;
        }
    }
    println!(
        "Removed {removed} data file(s) from {}. Backups (.bak) were kept.",
        dir.display()
    );
    0
}

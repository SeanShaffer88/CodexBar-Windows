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
  codexbar-windows refresh         Collect fresh numbers and save them
  codexbar-windows refresh --json  Same, as JSON
  codexbar-windows history         List saved snapshots (newest last)
  codexbar-windows clear-history   Move history to history.json.bak
  codexbar-windows set-key openrouter
                                   Read an OpenRouter API key from stdin and
                                   store it encrypted (Windows DPAPI)
  codexbar-windows clear-key openrouter
                                   Remove the stored OpenRouter API key
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
        "status" => status(&dir, args.iter().any(|a| a == "--json")),
        "refresh" => refresh_cmd(&dir, args.iter().any(|a| a == "--json")),
        "history" => history_cmd(&dir, args.iter().any(|a| a == "--json")),
        "clear-history" => match store::clear(&dir) {
            Ok(()) => {
                println!("History moved to history.json.bak (rename it back to undo).");
                0
            }
            Err(e) => fail(&e),
        },
        "set-key" => set_key(&dir, args),
        "clear-key" => clear_key(&dir, args),
        "config" => config_cmd(&dir, args),
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

fn status(dir: &Path, json: bool) -> i32 {
    match store::latest(dir) {
        Some(snap) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snap.reports).unwrap_or_default()
                );
            } else {
                print!("{}", ops::render_text(&snap.reports, timeutil::now_unix()));
                println!(
                    "(last collected {}; run `refresh` for fresh numbers)",
                    timeutil::format_utc(snap.at)
                );
            }
            0
        }
        None => {
            println!("Nothing collected yet — running a first refresh:");
            refresh_cmd(dir, json)
        }
    }
}

fn refresh_cmd(dir: &Path, json: bool) -> i32 {
    let cfg = load_cfg(dir);
    let now = timeutil::now_unix();
    let reports = ops::refresh(&cfg, dir, now);
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

fn set_key(dir: &Path, args: &[String]) -> i32 {
    match args.get(1).map(|s| s.as_str()) {
        Some("openrouter") => {
            eprintln!("Paste the OpenRouter API key, then press Enter:");
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err() {
                let _ = std::io::stdin().read_to_string(&mut input);
            }
            match secrets::set_openrouter_key(dir, &input) {
                Ok(()) => {
                    println!("Key stored, encrypted with Windows DPAPI.");
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => fail("Usage: codexbar-windows set-key openrouter"),
    }
}

fn clear_key(dir: &Path, args: &[String]) -> i32 {
    match args.get(1).map(|s| s.as_str()) {
        Some("openrouter") => match secrets::clear_openrouter_key(dir) {
            Ok(()) => {
                println!("Stored OpenRouter key removed.");
                0
            }
            Err(e) => fail(&e),
        },
        _ => fail("Usage: codexbar-windows clear-key openrouter"),
    }
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

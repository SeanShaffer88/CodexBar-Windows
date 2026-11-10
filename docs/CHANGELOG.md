# Changelog

## v0.4.0 — unreleased

- **Settings window**: a dark, single-page configuration window (design B)
  opened from the tray menu's "Settings…" item, by double-clicking the
  tray icon, or with `codexbar-windows settings`. It covers provider
  on/off switches, API-key add/change/remove for OpenRouter, DeepSeek,
  and Moonshot (DPAPI-encrypted, never shown after saving), the refresh
  interval, the warning threshold, start-with-Windows, and data-folder
  actions (open folder, clear history with the usual backup kept).
- The tray warning balloon now uses the configured warning threshold
  (new `warn_threshold_percent` setting, default 90) instead of a fixed
  90%.
- The settings window's changes apply to a running tray immediately,
  including the refresh timer.
- `settings` command: shows every setting (text or `--json`) and changes
  them from the command line with `--refresh`, `--warn`, and
  `--autostart on|off` (start-with-Windows uses the standard per-user
  Run entry and is Windows-only).
- Config schema is now version 2; version 1 files are migrated upward
  automatically with the new settings at their defaults.

## v0.3.0 — 2026-11-09

Grouped release: also covers v0.2.0 and v0.1.0 below, which were merged without individual releases.

- Claude Code "Today" now starts at local midnight (using the machine's
  time-zone offset) instead of UTC midnight.
- New provider: **Moonshot / Kimi API** — available, voucher, and cash
  balances (CNY) from `api.moonshot.cn`, with a DPAPI-stored API key,
  enabled by `set-key moonshot` like the other key providers.
- `status` and `refresh` accept `--provider <id>` to show one provider.

## v0.2.0 — grouped into v0.3.0

- New provider: **DeepSeek** — account balances (total, granted,
  topped-up, per currency) from `api.deepseek.com/user/balance` with a
  DPAPI-stored API key. Storing a key turns the provider on; clearing it
  turns it off.
- API-key handling generalized across key providers (`set-key` /
  `clear-key` now take `openrouter` or `deepseek`), plus a new
  `providers` command listing every provider, its on/off state, and
  whether a key is stored.
- Tray warning: when the busiest usage window crosses 90%, the tray
  shows one warning balloon naming the provider and window. It fires
  once per crossing, not on every refresh.

## v0.1.0 — grouped into v0.3.0

First version. A native Windows version of CodexBar's core idea: AI
coding limits in the tray.

- System tray app (native Win32): usage-bar icon colored by the busiest
  window, summary tooltip, click for a popup menu with the full
  breakdown, Refresh, data-folder shortcut, built-in Help, and Quit.
  Single instance; refreshes on a timer (15 minutes by default) from a
  worker thread so the tray never stalls.
- Codex: session (5-hour) and weekly rate-limit windows with reset
  countdowns, plan name, and total tokens, read from the Codex CLI's
  local session logs (`~/.codex/sessions`).
- Claude Code: token totals for today, the last 7 days, and the last 30
  days, read from local transcripts (`~/.claude/projects`), with
  duplicate messages counted once.
- OpenRouter: credit usage and remaining balance from the `auth/key`
  endpoint. The API key is stored encrypted with Windows DPAPI and never
  written in plain text.
- Offline honesty: when the network is down, OpenRouter shows the last
  known numbers labeled with their age; missing sources show a plain
  explanation instead of empty or invented numbers.
- Full command line: `status`, `refresh`, `history`, `set-key`,
  `clear-key`, `config`, `data-dir`, `clear-history`, `clean`, with
  `--json` output where it matters.
- Portable storage: all state in a `data/` folder beside the executable
  (config with `schema_version` and migration, encrypted keys, bounded
  history). Destructive actions keep a `.bak` backup.

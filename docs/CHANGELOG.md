# Changelog

## v0.1.0 — unreleased

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

# CodexBar-Windows v0.3.0

First release — a grouped release covering v0.1.0, v0.2.0, and v0.3.0.
A native Windows version of CodexBar: AI coding limits in your system tray.

## Highlights

- System tray app (native Win32): usage-bar icon colored by the busiest
  window, summary tooltip, popup menu with the full breakdown, Refresh,
  and Quit. Single instance; refreshes on a timer from a worker thread.
- **Codex**: session (5-hour) and weekly rate-limit windows with reset
  countdowns, plan name, and total tokens, read from the Codex CLI's
  local session logs.
- **Claude Code**: token totals for today (from local midnight), the
  last 7 days, and the last 30 days, from local transcripts, with
  duplicate messages counted once.
- **OpenRouter, DeepSeek, Moonshot**: credit and balance tracking from
  each provider's own API. Keys are stored encrypted with Windows DPAPI
  and never written in plain text; storing a key turns its provider on.
- Tray warning balloon when a usage window crosses 90%.
- Offline honesty: when the network is down, the last known numbers stay
  on screen labeled with their age; missing sources explain themselves
  instead of showing invented numbers.
- Full command line (`status`, `refresh`, `history`, `providers`,
  `set-key`, `config`, `--json` output) and a portable `data/` folder —
  no registry writes, no telemetry, no external runtime.

## Install

- `CodexBar-Windows-v0.3.0-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.3.0-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

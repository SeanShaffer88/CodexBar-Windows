# CodexBar-Windows v0.6.0

A grouped release covering v0.4.0, v0.5.0, and v0.6.0.
A native Windows version of CodexBar: AI coding limits in your system tray.

## Highlights

- **Usage detail window**: a dark window of provider cards — each
  provider's headline number big (session percent, month cost, tokens
  today, credits left), a supporting line, and a bar or footnote.
  Opens from the tray menu's "Details…" item or the `details`
  command; its Refresh button re-collects on the spot.
- **`breakdown` command**: per-model token totals and list-price cost
  estimates for Claude Code (today and 30 days) and Codex (30 days),
  as text or `--json`.
- **OpenAI API and Anthropic API providers**: whole-organization token
  usage (today and the last 7 days) and month-to-date cost, from each
  provider's documented Administration / Usage & Cost endpoints. Each
  needs an Admin API key (`set-key openai` / `set-key anthropic`),
  stored encrypted with Windows DPAPI. This is API-platform usage,
  not subscription quota, which has no public API.
- **Cost estimates for Codex and Claude Code**: local usage is priced
  per model at public list prices and shown as a clearly labeled
  estimate; models with no known price produce no estimate.
- **`report` command**: a day-by-day summary from the saved history
  (the last snapshot of each local day), with `--days N` and `--json`.
- **Settings window**: a dark, single-page configuration window opened
  from the tray menu, by double-clicking the tray icon, or with
  `codexbar-windows settings`. It covers all seven providers' on/off
  switches, API-key add/change/remove, the refresh interval, the
  warning threshold (which now drives the tray warning balloon), and
  start-with-Windows. Changes apply to a running tray immediately.
- Config schema is now version 2; version 1 files are migrated upward
  automatically with the new settings at their defaults.
- Per-Monitor v2 DPI awareness, so windows render crisply on scaled
  displays instead of being bitmap-stretched.

## Install

- `CodexBar-Windows-v0.6.0-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.6.0-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

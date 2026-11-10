# Features

- Tray icon that shows the busiest provider window's percent as a
  number (green / amber at 70% / red at 90%), plus a summary tooltip;
  with no percent data it falls back to a usage-bar glyph.
- Popup menu breakdown per provider, with reset countdowns.
- Providers in v0.1.0:
  - Codex (local CLI logs): session (5h) + weekly windows, plan, totals.
  - Claude Code (local transcripts): today / 7-day / 30-day token totals.
  - OpenRouter (API): credit usage, limit, and remaining balance.
  - DeepSeek (API): balances per currency (total / granted / topped up).
- One tray warning balloon when a usage window crosses 90%.
- `providers` command: every provider's on/off and key state at a glance.
- Automatic refresh every 5–120 minutes (default 15), manual Refresh in
  the menu and by command.
- Offline fallback to the last known numbers, labeled as such.
- Organization usage for API accounts: OpenAI API and Anthropic API
  providers report whole-organization tokens (today / 7 days) and
  month-to-date cost from their documented Admin endpoints.
- Cost estimates for Codex and Claude Code, priced per model at public
  list prices and always labeled as estimates.
- `report` command: day-by-day summary from the saved history.
- History: every refresh is snapshotted to `data/history.json` (bounded
  to the newest 500) and can be listed or exported as JSON.
- API keys encrypted with Windows DPAPI; never stored or logged in plain
  text.
- Usage detail window: one card per provider with its headline number
  (percent, month cost, tokens, or credits), from the tray menu or the
  `details` command, with its own Refresh. It also opens on its own
  when the program starts — and starting the program again while it
  runs brings the window up instead of an "already running" note.
- Tokens-per-day chart in the detail window: the last 14 days from
  the saved history, for the providers that keep a daily count.
- `breakdown` command: per-model token totals and cost estimates for
  the local-log providers. The same per-model list (last 30 days,
  busiest first) also appears as a section in the detail window.
- Gemini provider: per-family quotas (Pro / Flash / Flash Lite) and
  the account's plan, read through the Gemini CLI's own local sign-in
  — the same source the original uses. No API key to store.
- Per-Monitor v2 DPI awareness for crisp rendering on scaled displays.
- Its own application icon on the executable, the windows, and the
  installer.
- Full command-line parity for every tray capability, including `--json`.
- Single instance, no console window in tray mode, no registry writes,
  no telemetry, no external runtime.

## Settings window (v0.4.0)

- Dark single-page window: provider switches, per-provider API key
  add/change/remove (encrypted with Windows DPAPI, never displayed),
  refresh interval (5-120 minutes), warning threshold (50-100%),
  start-with-Windows toggle, data-folder open, and clear-history.
- Opens from the tray menu, a double-click on the tray icon, or the
  `settings` command; F1 shows the built-in help.
- Saving applies to a running tray at once, including its timer.
- Every value is equally available from the `settings` command line.

# Features

- Tray icon with a live usage bar (green / amber at 70% / red at 90%) for
  the busiest provider window, plus a summary tooltip.
- Popup menu breakdown per provider, with reset countdowns.
- Providers in v0.1.0:
  - Codex (local CLI logs): session (5h) + weekly windows, plan, totals.
  - Claude Code (local transcripts): today / 7-day / 30-day token totals.
  - OpenRouter (API): credit usage, limit, and remaining balance.
- Automatic refresh every 5–120 minutes (default 15), manual Refresh in
  the menu and by command.
- Offline fallback to the last known numbers, labeled as such.
- History: every refresh is snapshotted to `data/history.json` (bounded
  to the newest 500) and can be listed or exported as JSON.
- API keys encrypted with Windows DPAPI; never stored or logged in plain
  text.
- Full command-line parity for every tray capability, including `--json`.
- Single instance, no console window in tray mode, no registry writes,
  no telemetry, no external runtime.

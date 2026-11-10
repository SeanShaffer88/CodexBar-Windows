# CodexBar-Windows v0.9.0

A grouped release covering v0.7.0, v0.8.0, and v0.9.0.
A native Windows version of CodexBar: AI coding limits in your system tray.

## Highlights

- **Gemini provider**: per-family quotas (Pro, Flash, Flash Lite) and
  the account plan, read through the Gemini CLI's own local sign-in —
  the same source the original uses. No API key to store; the sign-in
  file is only read, never changed. Note: since June 2026 Google no
  longer serves individual accounts through this quota service, and
  the provider says so in plain words when that is the answer;
  Workspace and Code Assist accounts still work.
- **Application icon**: the executable, the windows, and the installer
  now carry an icon — a set of usage meters, the fullest in crimson,
  on dark charcoal.
- **Detail window opens at launch**, and starting the program again
  while it runs brings the window up instead of an "already running"
  note.
- **Tokens-per-day chart** in the detail window: the last 14 days
  from the saved history, for the providers that keep a daily count.
- **Per-model breakdown** in the detail window: the local providers'
  last-30-days totals per model, busiest first, with list-price
  estimates — the same numbers as the `breakdown` command.
- **Tray icon shows the percent**: the busiest window's percent as a
  number, colored green, amber at 70%, and red at 90%. With no
  percent data it falls back to the usage-bar glyph.
- Card text fixes in the detail window: lines wrap inside their cards
  instead of clipping, and a duplicated word in the Codex card is
  fixed.
- The README now shows screenshots from a real run.

## Install

- `CodexBar-Windows-v0.9.0-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.9.0-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

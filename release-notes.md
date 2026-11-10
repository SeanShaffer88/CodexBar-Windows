# CodexBar-Windows v0.10.0

## Added

- A per-file parse cache for the local log scans
  (`scan-cache-claude.json` and `scan-cache-codex.json` in the
  data folder). Each log file's extracted records are stored
  once and replayed on later refreshes; only files that changed
  since the last scan are read again. Refreshes over a large
  history no longer re-parse a month of logs every time. The
  numbers are unchanged, and a missing or damaged cache file is
  simply rebuilt.

## Install

- `CodexBar-Windows-v0.10.0-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.10.0-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

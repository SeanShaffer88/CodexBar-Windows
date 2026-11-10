# CodexBar-Windows v0.9.2

## Fixes

- The usage detail window opens instantly. It used to collect
  everything before appearing — and scanned the local logs twice,
  once for the cards and once for the breakdown — so on a machine
  with large logs it took seconds to show. It now opens with the
  last known numbers from the saved snapshot (breakdown lines are
  stored in the snapshot too) and finishes loading in the
  background; a refresh likewise runs in the background instead of
  freezing the window, and one collection now produces both the
  cards and the breakdown.
- The detail window's "Refreshed" time is the local time. It
  printed UTC before.

## Install

- `CodexBar-Windows-v0.9.2-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.9.2-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

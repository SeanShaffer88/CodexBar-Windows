# CodexBar-Windows v0.10.1

## Fixed

- Quitting from the tray menu while the usage detail window is
  open now actually quits. The detail window's message loop
  used to consume the quit signal for itself, leaving the
  window on screen and the program running with no tray icon.
  The window now closes through its normal path first, and the
  quit reaches the main loop (the settings window already
  worked this way).

## Changed

- The log scan cache now reads only the new tail of a log file
  that grew — the actively-written session files are no longer
  re-read in full on every refresh. A line still being written
  (no newline yet) is counted once it completes: never lost,
  never half-counted. The first refresh after upgrading
  rebuilds the cache once.

## Install

- `CodexBar-Windows-v0.10.1-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.10.1-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

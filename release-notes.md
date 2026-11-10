# CodexBar-Windows v0.9.5

## Changed

- Providers now collect in parallel instead of one after
  another, so a refresh takes about as long as its slowest
  provider rather than the sum of all of them — the difference
  shows most on a slow network, where each provider's connect
  and read timeouts used to add up. Reports still arrive in
  the same fixed order. The detail window also reads its saved
  history once per open instead of twice.

## Install

- `CodexBar-Windows-v0.9.5-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.9.5-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

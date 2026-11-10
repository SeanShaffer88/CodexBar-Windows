# CodexBar-Windows v0.9.1

A patch release that restores the application icon in the shipped
program. Everything else is unchanged from v0.9.0.

## Fix

- The v0.9.0 executable shipped without its embedded application
  icon: the Microsoft resource compiler rejected the resource
  script the build generates in the release environment (the
  script named the icon with an unescaped single-backslash path),
  the build continued with only a warning, and nothing checked
  the result — so the program arrived with the generic placeholder
  icon instead of the usage-meters icon. The path is now escaped,
  the compiler's error output is captured in the build log when
  it fails, and the CI and release pipelines verify that every
  icon image is inside the built program — a release now fails
  instead of shipping without the icon.

## Install

- `CodexBar-Windows-v0.9.1-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.9.1-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

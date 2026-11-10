# Install

## Requirements

- Windows 10 or 11, x64
- Optional: the Codex CLI and/or Claude Code, signed in and used at least
  once (so their local logs exist), and/or an OpenRouter API key

## Installer

Run `CodexBar-Windows-v<version>-x64-Setup.exe`. It installs for the
current user and can add a Start-menu shortcut. Uninstall from Windows
Settings → Apps, or run the program with `clean` first to also remove
its `data/` folder contents.

## Portable

Unzip `CodexBar-Windows-v<version>-x64-Portable.zip` anywhere and run
`codexbar-windows.exe`. Everything it writes stays in the `data/` folder
created beside the executable.

## First run

1. Double-click the executable — it appears in the system tray.
2. Click the tray icon to see your Codex and Claude Code numbers.
3. Optional, for OpenRouter: in a terminal, run
   `codexbar-windows set-key openrouter`, paste your key, press Enter.
   The key is stored encrypted; run `codexbar-windows refresh` after.

Binaries are unsigned; SmartScreen may show a one-time confirmation.

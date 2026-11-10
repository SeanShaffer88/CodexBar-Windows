# CodexBar-Windows

## TL;DR

Every AI coding limit, in your Windows tray. CodexBar-Windows is a native
Windows version of [CodexBar](https://github.com/steipete/CodexBar): it
shows your OpenAI Codex session (5-hour) and weekly usage windows with
reset countdowns, your Claude Code token totals, and your OpenRouter
credit balance — in a lightweight system-tray icon and popup menu, with a
full command line for scripting. No account logins, no telemetry, no
external runtimes: it reads the log files Codex and Claude Code already
write on your PC, plus OpenRouter's own key-usage endpoint with a key you
store encrypted on your machine.

## What it shows

- **Codex** — the real rate-limit windows from the Codex CLI's local
  session logs: session (5h) and weekly used percent, reset times, plan
  name, and total tokens.
- **Claude Code** — real token totals from Claude Code's local
  transcripts: today, last 7 days, last 30 days. (Claude Code does not
  store account limit percentages locally, so none are invented.)
- **OpenRouter** — credit usage and remaining balance for your API key,
  from OpenRouter's `auth/key` endpoint.
- **DeepSeek** — account balances (total, granted, topped-up) for your
  API key, from DeepSeek's balance endpoint.

The tray warns you once when any usage window crosses 90%.

When the network is down, the last known numbers stay on screen with an
"offline" note instead of going blank.

## Install

Download from [GitHub Releases](https://github.com/SeanShaffer88/CodexBar-Windows/releases):

- `CodexBar-Windows-v<version>-x64-Setup.exe` — installer
- `CodexBar-Windows-v<version>-x64-Portable.zip` — unzip anywhere and run

The program is a single self-contained executable. It writes only to a
`data/` folder beside the executable, so the portable copy can live on a
USB stick. Binaries are unsigned (no code-signing certificate); Windows
SmartScreen may ask you to confirm the first run.

## Use

Double-click `codexbar-windows.exe` and it settles into the tray. The icon
shows a usage bar for your busiest window (green, amber at 70%, red at
90%); hover for a summary, click for the full breakdown, Refresh, and
Quit. It refreshes every 15 minutes (changeable) and keeps a bounded
history of past readings in `data/history.json`.

Everything the tray does is also a command:

```text
codexbar-windows status            last collected numbers
codexbar-windows refresh           collect fresh numbers now
codexbar-windows refresh --json    same, machine-readable
codexbar-windows history           past snapshots
codexbar-windows report            day-by-day summary from history
codexbar-windows details           open the usage detail window (cards)
codexbar-windows breakdown         per-model tokens + cost estimates
codexbar-windows set-key openrouter
                                   store a provider key (read from stdin,
                                   encrypted with Windows DPAPI); openai
                                   and anthropic take Admin keys for their
                                   organization usage endpoints
codexbar-windows config refresh 30 set the refresh interval (minutes)
codexbar-windows settings          open the settings window (Windows)
codexbar-windows settings --warn 85  set the warning threshold
codexbar-windows --help            full command list, works offline
```

## Privacy

- Local logs are read, never modified or uploaded.
- The OpenRouter key is encrypted with Windows DPAPI (current-user scope)
  and only ever sent to `openrouter.ai`.
- There is no telemetry, no analytics, and no other network traffic.

## Screenshots

Screenshots will be added from real runs on a Windows machine.

## Build from source

```text
cargo test
cargo build --release --target x86_64-pc-windows-msvc
```

Rust, native Win32, static C runtime — no .NET or other runtime needed.
Windows 10 and 11, x64.

## License

MIT. The original CodexBar for macOS is by Peter Steinberger and
contributors, also MIT — see [docs/CREDITS.md](docs/CREDITS.md).

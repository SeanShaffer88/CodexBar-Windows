# Changelog

## v0.9.5 — 2026-11-10

### Changed

- Providers now collect in parallel instead of one after
  another, so a refresh takes about as long as its slowest
  provider rather than the sum of all of them — the difference
  shows most on a slow network, where each provider's connect
  and read timeouts used to add up. Reports still arrive in
  the same fixed order. The detail window also reads its saved
  history once per open instead of twice.

## v0.9.4 — 2026-11-10

### Changed

- Refreshes no longer re-read the whole transcript history. The
  Claude Code and Codex scans skip log files untouched for more
  than 31 days — an entry's file is modified when the entry is
  appended, so nothing inside the 30-day reporting window can be
  missed. (If a gated Codex pass finds no usage at all, it falls
  back to a full scan, so a dormant log's all-time totals still
  show.) Log files are also parsed in place from a single read —
  no per-line copies — and duplicate detection stores a hash
  instead of the text it is matching. Same numbers, much less
  work per refresh, and the cost no longer grows with your whole
  history.

## v0.9.3 — 2026-11-10

### Fixed
- The usage detail window no longer cuts off its bottom. The
  window was sized as if its height were all client area, but the
  title bar and borders come out of that height, so the footer
  and the Refresh/Close buttons fell below the visible area once
  the content grew tall enough. The window is now sized from the
  measured frame, so the client area matches the layout exactly.
- A failed provider's card no longer cuts its message off
  mid-sentence: the text cap now fits the full guidance (the
  sign-in and set-key sentences), wrapped over two lines.

## v0.9.2 — 2026-11-10

### Fixed
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

## v0.9.1 — 2026-11-10

### Fixed
- The application icon is embedded in the release build again. In
  v0.9.0 the Microsoft resource compiler rejected the resource
  script the build generates (it named the icon with an unescaped
  single-backslash path) and the build carried on with only a
  warning, so the shipped program had no icon. The path is now
  escaped, the compiler's own error text is captured in the build
  log when it fails, and the CI and release pipelines verify every
  icon image is inside the built program and fail if any is
  missing.

## v0.9.0 — 2026-11-10

### Added
- Gemini provider: per-family quotas (Pro, Flash, Flash Lite) and the
  account plan, read through the Gemini CLI's own local sign-in — the
  same source the original uses. No key to store; the sign-in file is
  only read, never changed. Since June 2026 Google no longer serves
  individual accounts through this quota service; when that is the
  answer, the provider says so in plain words.
- Per-model breakdown section in the usage detail window: the local
  providers' last-30-days totals per model, busiest first, with
  list-price estimates — the same numbers as the `breakdown` command.

### Changed
- The tray icon now shows the busiest window's percent as a number,
  colored by the usual bands (green, amber at 70%, red at 90%),
  instead of a fill bar. With no percent data it keeps the bar glyph.

## v0.8.0 — grouped into v0.9.0

### Added
- Tokens-per-day chart in the usage detail window: the last 14 days
  from the saved history (the providers with a daily count — Claude
  Code and the two organization APIs), with the peak day marked and
  today shown in red. Days with no readings stay empty, and the chart
  appears only once some day has a real count.
- The README now shows screenshots from a real run.

### Changed
- Starting the program while it is already running in the tray now
  opens the usage detail window on the running copy, instead of
  showing an "already running" message.

## v0.7.0 — grouped into v0.9.0

### Added
- Application icon: the program now has its own icon (three usage
  meters, the fullest in red) on the executable, on the settings and
  detail windows, and in the installer.
- The usage detail window opens automatically when the program
  starts, alongside the tray icon.

### Fixed
- Detail-window cards: the Codex card no longer repeats the word
  "tokens" ("total tokens tokens"), and card text now wraps inside
  the taller cards instead of running off the edge and being cut off.

## v0.6.0 — 2026-11-10

### Added
- Usage detail window (design B): a dark window of provider cards —
  each provider's headline number big (session percent, month cost,
  tokens today, credits left), a supporting line, and a bar or
  footnote. Opens from the tray menu's "Details…" item or the
  `details` command, and its Refresh button re-collects on the spot.
- `breakdown` command: per-model token totals and list-price cost
  estimates for Claude Code (today and 30 days) and Codex (30 days),
  as text or `--json`.
- Per-Monitor v2 DPI awareness (embedded manifest on MSVC builds plus
  a startup declaration), so windows render crisply on scaled
  displays instead of being bitmap-stretched.

## v0.5.0 — grouped into v0.6.0

### Added
- OpenAI API provider: whole-organization token usage (today and the
  last 7 days) and month-to-date cost, from OpenAI's documented
  Administration usage and costs endpoints. Needs an Admin API key
  (`set-key openai`); this is API-platform usage, not ChatGPT
  subscription quota, which has no public API.
- Anthropic API provider: whole-organization token usage (today and
  the last 7 days) and month-to-date cost, from Anthropic's documented
  Usage & Cost endpoints. Needs an Admin API key (`set-key anthropic`)
  and a Team-plan-or-higher organization.
- Cost estimates for the local-log providers: Codex and Claude Code
  usage is now also priced per model at public list prices and shown
  as a clearly labeled estimate (list prices are not a subscription
  bill; models with no known price produce no estimate).
- `report` command: a day-by-day summary from the saved history (the
  last snapshot of each local day), with `--days N` and `--json`.

### Changed
- The settings window now lists seven providers.

## v0.4.0 — grouped into v0.6.0

- **Settings window**: a dark, single-page configuration window (design B)
  opened from the tray menu's "Settings…" item, by double-clicking the
  tray icon, or with `codexbar-windows settings`. It covers provider
  on/off switches, API-key add/change/remove for OpenRouter, DeepSeek,
  and Moonshot (DPAPI-encrypted, never shown after saving), the refresh
  interval, the warning threshold, start-with-Windows, and data-folder
  actions (open folder, clear history with the usual backup kept).
- The tray warning balloon now uses the configured warning threshold
  (new `warn_threshold_percent` setting, default 90) instead of a fixed
  90%.
- The settings window's changes apply to a running tray immediately,
  including the refresh timer.
- `settings` command: shows every setting (text or `--json`) and changes
  them from the command line with `--refresh`, `--warn`, and
  `--autostart on|off` (start-with-Windows uses the standard per-user
  Run entry and is Windows-only).
- Config schema is now version 2; version 1 files are migrated upward
  automatically with the new settings at their defaults.

## v0.3.0 — 2026-11-09

Grouped release: also covers v0.2.0 and v0.1.0 below, which were merged without individual releases.

- Claude Code "Today" now starts at local midnight (using the machine's
  time-zone offset) instead of UTC midnight.
- New provider: **Moonshot / Kimi API** — available, voucher, and cash
  balances (CNY) from `api.moonshot.cn`, with a DPAPI-stored API key,
  enabled by `set-key moonshot` like the other key providers.
- `status` and `refresh` accept `--provider <id>` to show one provider.

## v0.2.0 — grouped into v0.3.0

- New provider: **DeepSeek** — account balances (total, granted,
  topped-up, per currency) from `api.deepseek.com/user/balance` with a
  DPAPI-stored API key. Storing a key turns the provider on; clearing it
  turns it off.
- API-key handling generalized across key providers (`set-key` /
  `clear-key` now take `openrouter` or `deepseek`), plus a new
  `providers` command listing every provider, its on/off state, and
  whether a key is stored.
- Tray warning: when the busiest usage window crosses 90%, the tray
  shows one warning balloon naming the provider and window. It fires
  once per crossing, not on every refresh.

## v0.1.0 — grouped into v0.3.0

First version. A native Windows version of CodexBar's core idea: AI
coding limits in the tray.

- System tray app (native Win32): usage-bar icon colored by the busiest
  window, summary tooltip, click for a popup menu with the full
  breakdown, Refresh, data-folder shortcut, built-in Help, and Quit.
  Single instance; refreshes on a timer (15 minutes by default) from a
  worker thread so the tray never stalls.
- Codex: session (5-hour) and weekly rate-limit windows with reset
  countdowns, plan name, and total tokens, read from the Codex CLI's
  local session logs (`~/.codex/sessions`).
- Claude Code: token totals for today, the last 7 days, and the last 30
  days, read from local transcripts (`~/.claude/projects`), with
  duplicate messages counted once.
- OpenRouter: credit usage and remaining balance from the `auth/key`
  endpoint. The API key is stored encrypted with Windows DPAPI and never
  written in plain text.
- Offline honesty: when the network is down, OpenRouter shows the last
  known numbers labeled with their age; missing sources show a plain
  explanation instead of empty or invented numbers.
- Full command line: `status`, `refresh`, `history`, `set-key`,
  `clear-key`, `config`, `data-dir`, `clear-history`, `clean`, with
  `--json` output where it matters.
- Portable storage: all state in a `data/` folder beside the executable
  (config with `schema_version` and migration, encrypted keys, bounded
  history). Destructive actions keep a `.bak` backup.

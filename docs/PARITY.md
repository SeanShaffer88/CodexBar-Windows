# Parity with upstream CodexBar (steipete/CodexBar)

Upstream CodexBar is a macOS menu-bar app covering 100+ providers. This
Windows version tracks parity honestly; scores come only from what is
implemented and verified here.

| Upstream capability | Here | State |
|---|---|---|
| Menu-bar/tray status per provider | Tray icon (worst window's percent as a number since v0.9.0) + tooltip + popup menu | Implemented; Windows runtime Unverified |
| Session (5h) + weekly windows with reset countdowns | Codex, from local CLI logs | Implemented |
| Credits / balance tracking | OpenRouter credits, DeepSeek and Moonshot balances | Implemented |
| Organization API usage + cost | OpenAI API and Anthropic API via their documented Admin endpoints (Admin key) | Implemented (v0.5.0) |
| Local-log cost estimates | Codex and Claude Code tokens priced per model at public list prices, labeled estimates | Implemented (v0.5.0) |
| History reporting | `report` day-by-day summary from saved snapshots | Implemented (v0.5.0) |
| Low-limit notifications | One tray balloon when a window crosses 90% | Implemented; Windows runtime Unverified |
| Local cost/token scans | Claude Code token totals (today / 7d / 30d) | Implemented (totals only — Claude Code stores no limit percentages locally) |
| CLI with JSON output | `status` / `refresh` / `history --json` | Implemented |
| Provider enable/disable + API-key config from CLI | `config`, `set-key`, provider toggles in config | Implemented |
| 100+ providers (Cursor, Gemini, Copilot, Groq, ...) | 8 sources | Not yet — providers are added only where a real, readable data source exists |
| Gemini CLI quotas | Pro / Flash / Flash Lite quota windows + plan, via the CLI's local OAuth sign-in and Google's Code Assist quota service (the upstream approach). Deliberate differences: refreshed tokens are kept in memory only (the CLI's credentials file is never rewritten), and there is no Antigravity handoff — a refused quota request reports Google's June 2026 end of individual-account coverage in plain words | Implemented (v0.9.0); Windows runtime Unverified |
| Browser-cookie / OAuth session reuse | — | Not planned for v0.x; local files and API keys only (the Gemini provider's local OAuth file is the one exception, read-only) |
| Per-provider tray icons / merge mode | Single icon showing the worst window | Partial |
| Settings window | Dark single-page window (tray menu, tray double-click, or `settings` command): provider switches, key management, refresh interval, warning threshold, start-with-Windows | Implemented (v0.4.0) |
| Usage detail popover/window | Dark card window (tray "Details…" or `details` command): one card per provider with headline number, bar, and footnote; own Refresh; opens automatically at program launch (v0.7.0); 14-day tokens-per-day chart from history (v0.8.0) | Implemented (v0.6.0) |
| Per-model breakdown | `breakdown` command with per-model tokens and list-price estimates; the same list appears in the detail window (v0.9.0) | Implemented (v0.6.0) |

## Known limitations

- Tray refresh-interval changes need a tray restart to apply.
- Windows runtime behavior: the settings and detail windows were
  field-verified on Windows on 2026-11-10; the tray icon's live
  behavior, DPAPI key storage, autostart, and idle memory use are
  still owed a field check.

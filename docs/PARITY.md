# Parity with upstream CodexBar (steipete/CodexBar)

Upstream CodexBar is a macOS menu-bar app covering 100+ providers. This
Windows version tracks parity honestly; scores come only from what is
implemented and verified here.

| Upstream capability | Here | State |
|---|---|---|
| Menu-bar/tray status per provider | Tray icon + tooltip + popup menu | Implemented; Windows runtime Unverified |
| Session (5h) + weekly windows with reset countdowns | Codex, from local CLI logs | Implemented |
| Credits / balance tracking | OpenRouter credits, DeepSeek and Moonshot balances | Implemented |
| Organization API usage + cost | OpenAI API and Anthropic API via their documented Admin endpoints (Admin key) | Implemented (v0.5.0) |
| Local-log cost estimates | Codex and Claude Code tokens priced per model at public list prices, labeled estimates | Implemented (v0.5.0) |
| History reporting | `report` day-by-day summary from saved snapshots | Implemented (v0.5.0) |
| Low-limit notifications | One tray balloon when a window crosses 90% | Implemented; Windows runtime Unverified |
| Local cost/token scans | Claude Code token totals (today / 7d / 30d) | Implemented (totals only — Claude Code stores no limit percentages locally) |
| CLI with JSON output | `status` / `refresh` / `history --json` | Implemented |
| Provider enable/disable + API-key config from CLI | `config`, `set-key`, provider toggles in config | Implemented |
| 100+ providers (Cursor, Gemini, Copilot, Groq, ...) | 7 sources | Not yet — providers are added only where a real, readable data source exists |
| Browser-cookie / OAuth session reuse | — | Not planned for v0.x; local files and API keys only |
| Per-provider tray icons / merge mode | Single icon showing the worst window | Partial |
| Settings window | Dark single-page window (tray menu, tray double-click, or `settings` command): provider switches, key management, refresh interval, warning threshold, start-with-Windows | Implemented (v0.4.0) |
| Usage detail popover/window | Dark card window (tray "Details…" or `details` command): one card per provider with headline number, bar, and footnote; own Refresh | Implemented (v0.6.0) |
| Per-model breakdown | `breakdown` command with per-model tokens and list-price estimates | Implemented (v0.6.0) |

## Known limitations

- Tray refresh-interval changes need a tray restart to apply.
- Windows runtime behavior (tray, DPAPI, WinHTTP) is verified by build
  and link here; a field run on Windows is still owed.

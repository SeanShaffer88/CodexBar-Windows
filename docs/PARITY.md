# Parity with upstream CodexBar (steipete/CodexBar)

Upstream CodexBar is a macOS menu-bar app covering 100+ providers. This
Windows version tracks parity honestly; scores come only from what is
implemented and verified here.

| Upstream capability | Here | State |
|---|---|---|
| Menu-bar/tray status per provider | Tray icon + tooltip + popup menu | Implemented; Windows runtime Unverified |
| Session (5h) + weekly windows with reset countdowns | Codex, from local CLI logs | Implemented |
| Credits / balance tracking | OpenRouter credits, DeepSeek and Moonshot balances | Implemented |
| Low-limit notifications | One tray balloon when a window crosses 90% | Implemented; Windows runtime Unverified |
| Local cost/token scans | Claude Code token totals (today / 7d / 30d) | Implemented (totals only — Claude Code stores no limit percentages locally) |
| CLI with JSON output | `status` / `refresh` / `history --json` | Implemented |
| Provider enable/disable + API-key config from CLI | `config`, `set-key`, provider toggles in config | Implemented |
| 100+ providers (Cursor, Gemini, Copilot, Groq, ...) | 5 sources | Not yet — providers are added only where a real, readable data source exists |
| Browser-cookie / OAuth session reuse | — | Not planned for v0.x; local files and API keys only |
| Per-provider tray icons / merge mode | Single icon showing the worst window | Partial |
| Settings window | Settings are file + CLI in v0.1.0 | Planned |

## Known limitations

- Tray refresh-interval changes need a tray restart to apply.
- Windows runtime behavior (tray, DPAPI, WinHTTP) is verified by build
  and link here; a field run on Windows is still owed.

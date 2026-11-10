# Troubleshooting

**"no Codex sessions found"** — The Codex CLI has not run yet on this PC,
or it keeps its data somewhere unusual. Run Codex once, then Refresh. If
you moved its data, set `codex_dir` in `data/config.json`.

**"no Claude Code projects found"** — Same idea for Claude Code: run it
once in any project, then Refresh, or set `claude_dir` in the config.

**"no API key stored" (OpenRouter)** — Run
`codexbar-windows set-key openrouter` and paste the key. Keys are read
from stdin so they never appear in your command history.

**"the server refused the API key"** — The stored key is wrong or
revoked. Store a fresh one with `set-key` (it replaces the old one).

**Numbers look stale / "offline — showing last known numbers"** — The
last network check failed, so the previous reading is shown with its
age. Check your connection and Refresh.

**The tray icon is missing** — Windows may have put it in the tray
overflow (the ^ arrow near the clock). Drag it out, or find
CodexBar-Windows in Settings → Personalization → Taskbar.

**Double-clicking shows a console flash** — That is the tray starting;
the console closes itself and the icon stays in the tray.

**Reset everything** — `codexbar-windows clean` removes settings, the
stored key, and history (backups with a `.bak` suffix are kept).

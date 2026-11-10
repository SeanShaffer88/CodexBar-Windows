# Architecture

Single Rust binary, native Win32, no runtime dependencies.

```text
src/
├── main.rs        entry point → CLI dispatch
├── cli.rs         every command (full parity with the tray)
├── ops.rs         operations: collect_all / refresh / render
├── model.rs       ProviderReport / UsageWindow + formatting
├── config.rs      data/ layout, config.json + schema_version migration
├── secrets.rs     DPAPI-encrypted API keys (data/secrets.json)
├── store.rs       bounded snapshot history (data/history.json)
├── timeutil.rs    UTC date math (no date-library dependency)
├── tray.rs        Win32 message-only window, Shell_NotifyIcon, popup menu
└── scrapers/
    ├── mod.rs     shared .jsonl discovery
    ├── codex.rs   ~/.codex/sessions — rate-limit windows from token_count events
    ├── claude_code.rs  ~/.claude/projects — token totals, deduped by message id
    ├── openrouter.rs   GET openrouter.ai/api/v1/auth/key
    └── http.rs    WinHTTP HTTPS GET (Windows); honest error elsewhere
```

Rules that keep it fast and honest:

- Everything is an operation first (`ops.rs`); the tray and CLI are thin
  surfaces over the same code.
- Collection runs on a worker thread; the tray message loop only draws.
- Parsers are pure functions over lines, tested against recorded data
  shapes; a source that cannot produce real numbers returns an error
  report, never an estimate.
- All state lives in `data/` beside the executable; writes are atomic
  (temp file + rename) and destructive actions keep a `.bak`.

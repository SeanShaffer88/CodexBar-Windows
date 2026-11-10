# CodexBar-Windows — Project Rules

## Project Manifest
- Tier: t1b (GUI/tray utility) — declared in `.project-tier`
- Prompt in force: The Elite Software Master Prompt v1.1 · Adaptive Engineering Contract V4.0
- Purpose in one line: AI coding-provider usage limits (Codex, Claude Code, OpenRouter) in the Windows system tray — a Windows version of steipete/CodexBar.
- Stack: Rust, native Win32 (windows-sys), static CRT, WinHTTP, DPAPI. No .NET, no Electron, no WinUI 3.
- Release shape: `CodexBar-Windows-v{VERSION}-x64-Setup.exe` + `CodexBar-Windows-v{VERSION}-x64-Portable.zip` + `SHA256SUMS.txt`. No MSI, ever. Confirm with the owner before every release.
- Frozen foundation: operations live in `src/ops.rs` + `src/scrapers/`; the tray (`src/tray.rs`) and CLI (`src/cli.rs`) are thin surfaces over them. Every capability exists as an operation first, with full CLI parity.
- Build: `cargo build --release --target x86_64-pc-windows-gnu` (local link check) · Full gate: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` (catches Windows-only lints the host pass cannot see — CI caught one in v0.1.0), `cargo test`, Windows-target release link with a fresh-binary check (size + mtime + feature strings).
- Budgets: idle RAM target under 15 MB (owner's brief), near 0% CPU when idle (event-driven message loop, timer refresh only). RAM is Unverified until measured on Windows.
- Storage: portable `data/` beside the exe only; config carries `schema_version` with tested upward migration; destructive actions back up first (`*.bak`).
- Honesty rule: a source that cannot produce real numbers reports an error or an offline-cached note — never an invented percentage. Claude Code limits are not stored locally, so Claude Code reports token totals only.
- Known limitations live at: `docs/PARITY.md` · Decision log: `docs/CHANGELOG.md` · QUALITY.md: `QUALITY.md`

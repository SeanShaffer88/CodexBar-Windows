# Security

- API keys are encrypted with Windows DPAPI (current-user scope) before
  being written to `data/secrets.json`. Plain-text keys are never stored,
  logged, or included in history or JSON output.
- The OpenRouter key is sent only to `openrouter.ai` over HTTPS
  (WinHTTP, system certificate validation).
- Local log files (Codex, Claude Code) are opened read-only.
- There is no telemetry, no auto-update download, and no other network
  traffic.
- All program state sits in one portable `data/` folder, which makes the
  full footprint easy to audit or delete (`codexbar-windows clean`).

Report a vulnerability by opening a private security advisory on the
GitHub repository, or a regular issue for non-sensitive problems.

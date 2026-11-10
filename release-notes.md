# CodexBar-Windows v0.9.4

## Changed

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

## Install

- `CodexBar-Windows-v0.9.4-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.9.4-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

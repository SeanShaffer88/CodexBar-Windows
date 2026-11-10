# CodexBar-Windows v0.9.3

## Fixes

- The usage detail window no longer cuts off its bottom. The
  window was sized as if its height were all client area, but the
  title bar and borders come out of that height, so the footer
  and the Refresh/Close buttons fell below the visible area once
  the content grew tall enough. The window is now sized from the
  measured frame, so the client area matches the layout exactly.
- A failed provider's card no longer cuts its message off
  mid-sentence: the text cap now fits the full guidance (the
  sign-in and set-key sentences), wrapped over two lines.

## Install

- `CodexBar-Windows-v0.9.3-x64-Setup.exe` — installer
- `CodexBar-Windows-v0.9.3-x64-Portable.zip` — unzip anywhere and run
- `SHA256SUMS.txt` — checksums for both

Binaries are unsigned (no code-signing certificate); Windows SmartScreen
may ask you to confirm the first run.

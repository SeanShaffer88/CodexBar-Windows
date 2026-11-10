# Contributing

- Build and gate locally before proposing a change:
  `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`, and a
  Windows-target release build that links (`x86_64-pc-windows-gnu` or
  `-msvc`).
- New data sources go in `src/scrapers/` as pure line/response parsers
  plus a thin collector, with tests against recorded data shapes. A
  source that cannot produce real numbers must return an error report —
  never estimate.
- Every capability is implemented as an operation in `src/ops.rs` first,
  then surfaced in the tray and the CLI together.
- Keep dependencies minimal; justify any addition in the change itself.

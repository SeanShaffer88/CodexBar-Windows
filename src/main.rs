//! CodexBar-Windows: AI coding-provider usage limits in the Windows tray.
//! A Windows version of CodexBar (steipete/CodexBar).
//!
//! No arguments starts the tray app; every capability is also a command
//! (see `cli::HELP`). All state lives in a portable `data/` folder beside
//! the executable.

mod cli;
mod config;
mod detail;
#[cfg(windows)]
mod icon;
mod model;
mod ops;
mod pricing;
mod scrapers;
mod secrets;
mod settings;
mod store;
mod timeutil;
mod tray;

fn main() {
    set_dpi_awareness();
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(cli::run(&args));
}

/// Ask Windows for Per-Monitor v2 DPI awareness before any window is
/// created, so the tray windows render crisply on scaled displays.
/// MSVC builds also carry the same declaration in the embedded
/// manifest; this call covers the rest and is a no-op elsewhere.
#[cfg(windows)]
fn set_dpi_awareness() {
    use windows_sys::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[cfg(not(windows))]
fn set_dpi_awareness() {}

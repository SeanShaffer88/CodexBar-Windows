//! CodexBar-Windows: AI coding-provider usage limits in the Windows tray.
//! A Windows version of CodexBar (steipete/CodexBar).
//!
//! No arguments starts the tray app; every capability is also a command
//! (see `cli::HELP`). All state lives in a portable `data/` folder beside
//! the executable.

mod cli;
mod config;
mod model;
mod ops;
mod scrapers;
mod secrets;
mod settings;
mod store;
mod timeutil;
mod tray;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(cli::run(&args));
}

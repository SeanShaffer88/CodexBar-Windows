//! Data sources. Each scraper is an operation: it reads one source and
//! returns a `ProviderReport`, never panicking and never guessing — a source
//! that cannot be read produces an honest error report instead of numbers.

pub mod claude_code;
pub mod codex;
pub mod http;
pub mod openrouter;

use std::path::{Path, PathBuf};

/// Collect every `.jsonl` file under `dir` (recursively), oldest-modified
/// first so "last event wins" parsing sees events in time order.
pub fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((mtime, p));
            }
        }
    }
    out.sort_by_key(|(t, _)| *t);
    out.into_iter().map(|(_, p)| p).collect()
}

/// Read a file to text, tolerating a trailing partial line (a writer may be
/// mid-append); the parsers skip malformed lines themselves.
pub fn read_lines(path: &Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => text.lines().map(|l| l.to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

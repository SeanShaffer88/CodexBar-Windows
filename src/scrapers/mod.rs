//! Data sources. Each scraper is an operation: it reads one source and
//! returns a `ProviderReport`, never panicking and never guessing — a source
//! that cannot be read produces an honest error report instead of numbers.

pub mod anthropic;
pub mod claude_code;
pub mod codex;
pub mod deepseek;
pub mod gemini;
pub mod http;
pub mod moonshot;
pub mod openai;
pub mod openrouter;

use std::path::{Path, PathBuf};

/// One `.jsonl` log file and when it was last modified. The
/// modification time matters: a file can only gain entries by being
/// appended to, which moves it — so scans can skip files untouched
/// since before their window and lose nothing.
pub struct LogFile {
    pub path: PathBuf,
    pub modified: std::time::SystemTime,
}

impl LogFile {
    /// Modification time as Unix seconds (0 when unknown).
    pub fn modified_secs(&self) -> i64 {
        self.modified
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// Collect every `.jsonl` file under `dir` (recursively), oldest-modified
/// first so "last event wins" parsing sees events in time order.
pub fn jsonl_files(dir: &Path) -> Vec<LogFile> {
    let mut out: Vec<LogFile> = Vec::new();
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
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push(LogFile { path: p, modified });
            }
        }
    }
    out.sort_by_key(|f| f.modified);
    out
}

/// Read a whole log file to text (empty when unreadable). Parsers
/// iterate its lines as `&str` slices — the file exists in memory
/// once, with no per-line copies. A trailing partial line (a writer
/// mid-append) is yielded like any other and skipped by the parsers
/// as malformed.
pub fn load_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// A stable hash of a string for dedupe sets: membership is all
/// that matters, so the sets store `u64`s instead of allocating a
/// `String` per entry in the scan hot loop.
pub fn str_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

//! Persisted per-file parse cache for the local log scans.
//!
//! A refresh used to re-read and re-parse every recent transcript
//! file from scratch; at real-world history sizes that dominated
//! the refresh (seconds of JSON parsing for numbers that mostly
//! had not changed). Instead, each scraper now extracts compact
//! records from a file once and stores them here, keyed by the
//! file's path and stamped with its modification time and size.
//! A later scan replays the stored records — plain arithmetic —
//! and only re-reads files whose stamp moved (appends move both).
//! Records carry raw timestamps, so the reporting windows still
//! slide correctly at replay time; nothing is pre-aggregated.
//!
//! The cache is an optimization only: a missing, corrupt, or
//! stale-version file is ignored and the scan falls back to
//! reading the logs, so it can never change the numbers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 2;

/// Safety valve: past this many cached records in one cache, the
/// file would be too large to be worth loading — stop saving it
/// and let the scans read the logs directly.
pub const MAX_RECORDS: usize = 2_000_000;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileEntry<R> {
    pub mtime: i64,
    /// The file's size when the entry was written.
    pub size: u64,
    /// How many leading bytes of the file `records` covers: the
    /// offset just past the last complete (newline-terminated)
    /// line extracted. A line still being written at the end of
    /// the file stays uncovered — and uncounted — until its
    /// newline lands, when the tail read picks it up whole.
    /// A file that only grew is re-read from here, not from 0.
    pub covered: u64,
    pub records: Vec<R>,
}

#[derive(Serialize, Deserialize)]
pub struct Cache<R> {
    pub schema_version: u32,
    pub files: HashMap<String, FileEntry<R>>,
}

impl<R> Default for Cache<R> {
    fn default() -> Self {
        Cache {
            schema_version: SCHEMA_VERSION,
            files: HashMap::new(),
        }
    }
}

pub fn cache_path(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join(name)
}

/// Load a cache file; anything unreadable or from another schema
/// version yields an empty cache (the scan then re-reads the logs
/// and repopulates it).
pub fn load<R: serde::de::DeserializeOwned>(path: &Path) -> Cache<R> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Cache::default();
    };
    match serde_json::from_str::<Cache<R>>(&text) {
        Ok(c) if c.schema_version == SCHEMA_VERSION => c,
        _ => Cache::default(),
    }
}

/// Persist a cache (best effort — a failed save only costs the
/// next scan its speed). Written to a sibling temp file and
/// renamed over the old one, like the history store.
pub fn save<R: Serialize>(path: &Path, cache: &Cache<R>) {
    let Ok(text) = serde_json::to_string(cache) else {
        return;
    };
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

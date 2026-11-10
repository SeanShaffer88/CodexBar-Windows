//! Snapshot history: every refresh appends one entry to `data/history.json`
//! (bounded, newest last) so past usage stays available offline and the tray
//! can show the last known numbers when a source is unreachable.

use crate::model::ProviderReport;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_SNAPSHOTS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub at: i64,
    pub reports: Vec<ProviderReport>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    schema_version: u32,
    #[serde(default)]
    snapshots: Vec<Snapshot>,
}

fn history_path(dir: &Path) -> PathBuf {
    dir.join("history.json")
}

fn load_file(dir: &Path) -> HistoryFile {
    std::fs::read_to_string(history_path(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Append one snapshot, keeping at most MAX_SNAPSHOTS.
pub fn append(dir: &Path, reports: &[ProviderReport], now: i64) -> Result<(), String> {
    let mut f = load_file(dir);
    f.schema_version = 1;
    f.snapshots.push(Snapshot {
        at: now,
        reports: reports.to_vec(),
    });
    if f.snapshots.len() > MAX_SNAPSHOTS {
        let drop = f.snapshots.len() - MAX_SNAPSHOTS;
        f.snapshots.drain(0..drop);
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create data folder: {e}"))?;
    let tmp = dir.join("history.json.tmp");
    let text = serde_json::to_string(&f).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write history: {e}"))?;
    std::fs::rename(&tmp, history_path(dir)).map_err(|e| format!("cannot replace history: {e}"))
}

/// The most recent snapshot, if any.
pub fn latest(dir: &Path) -> Option<Snapshot> {
    load_file(dir).snapshots.pop()
}

/// All snapshots, oldest first.
pub fn all(dir: &Path) -> Vec<Snapshot> {
    load_file(dir).snapshots
}

/// Delete the whole history. The file is renamed to `history.json.bak`
/// first, so the action can be undone by renaming it back.
pub fn clear(dir: &Path) -> Result<(), String> {
    let p = history_path(dir);
    if p.exists() {
        std::fs::rename(&p, dir.join("history.json.bak"))
            .map_err(|e| format!("cannot back up history: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProviderReport;

    fn report(id: &str) -> ProviderReport {
        ProviderReport {
            id: id.into(),
            name: id.into(),
            ok: true,
            offline: false,
            error: None,
            windows: vec![],
            fetched_at: 0,
        }
    }

    #[test]
    fn append_bounds_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("cbw-hist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..5 {
            append(&dir, &[report("codex")], i).unwrap();
        }
        let snaps = all(&dir);
        assert_eq!(snaps.len(), 5);
        assert_eq!(latest(&dir).unwrap().at, 4);
        clear(&dir).unwrap();
        assert!(all(&dir).is_empty());
        assert!(dir.join("history.json.bak").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

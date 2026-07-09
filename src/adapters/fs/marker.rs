use std::path::Path;

use async_trait::async_trait;

use crate::app::extract::{fingerprint, marker_is_current, Marker};
use crate::domain::Failure;
use crate::ports::MarkerStore;

/// The marker file name, written as a sibling of `Current`.
const MARKER_FILE: &str = ".acd-version";

/// A `MarkerStore` backed by the local filesystem. The marker lives at
/// `<package_dir>/.acd-version`; the Extracted Copy at `<package_dir>/Current`.
/// The fingerprint is a stat-only walk (file contents are never read).
pub struct LocalMarkerStore;

#[async_trait]
impl MarkerStore for LocalMarkerStore {
    async fn is_current(&self, package_dir: &Path, version: &str) -> bool {
        let marker = tokio::fs::read_to_string(package_dir.join(MARKER_FILE))
            .await
            .ok()
            .and_then(|text| Marker::parse(&text));
        let current = fingerprint(&scan_files(&package_dir.join("Current")));
        marker_is_current(marker.as_ref(), version, &current)
    }

    async fn record(&self, package_dir: &Path, version: &str) -> Result<(), Failure> {
        let marker = Marker {
            version: version.to_string(),
            fingerprint: fingerprint(&scan_files(&package_dir.join("Current"))),
        };
        tokio::fs::write(package_dir.join(MARKER_FILE), marker.to_file_string())
            .await
            .map_err(|e| Failure::fatal(format!("write marker in {}: {e}", package_dir.display())))
    }
}

/// Stat-walk `root`, returning `(relative-path, size)` for every file beneath it
/// (recursively). A missing `root` yields an empty list.
fn scan_files(root: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => walk(root, &path, out),
            Ok(ft) if ft.is_file() => {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                out.push((rel, size));
            }
            _ => {}
        }
    }
}

//! The Extract Phase: unpack each Entry's archive from its Cache Root version
//! folder into the iOS repo's `PodLocals/<package>/Current`. See ADR 0007.

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::domain::{Asset, FailedAsset};

/// The Extraction Marker: the pinned version plus the fingerprint of the
/// Extracted Copy it was written for. Serialized to `PodLocals/<pkg>/.acd-version`
/// (a sibling of `Current`). See ADR 0007.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub version: String,
    pub fingerprint: String,
}

impl Marker {
    /// Render the marker to its on-disk file form.
    pub fn to_file_string(&self) -> String {
        serde_yaml::to_string(self).expect("marker serializes")
    }

    /// Parse a marker from its on-disk file form.
    pub fn parse(text: &str) -> Option<Self> {
        serde_yaml::from_str(text).ok()
    }
}

/// Whether the Extracted Copy can be skipped: a marker must exist, be for the
/// pinned version, and match the current stat fingerprint. Any mismatch (wrong
/// version, tampered/deleted files, or no marker) means re-extract.
pub fn marker_is_current(
    marker: Option<&Marker>,
    version: &str,
    current_fingerprint: &str,
) -> bool {
    matches!(marker, Some(m) if m.version == version && m.fingerprint == current_fingerprint)
}

/// A stat-only fingerprint of an Extracted Copy: a hash over the sorted
/// `(relative-path, size)` of every file. Order-independent; changes when any
/// file is added, removed, renamed, or resized. File contents are never read.
pub fn fingerprint(entries: &[(String, u64)]) -> String {
    let mut sorted: Vec<&(String, u64)> = entries.iter().collect();
    sorted.sort();
    let mut hasher = Md5::new();
    for (path, size) in sorted {
        hasher.update(path.as_bytes());
        hasher.update([0]); // separator so ("ab",1) and ("a",1)+("b",…) differ
        hasher.update(size.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

/// The Extract Phase's contribution to the Run Summary: how many archives were
/// unpacked, and the per-package failures collected along the way.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExtractReport {
    pub extracted: usize,
    /// Entries skipped because the Extraction Marker was already current.
    pub skipped: usize,
    pub failed: Vec<FailedAsset>,
}

/// Why an Entry's single archive could not be identified.
#[derive(Debug, PartialEq, Eq)]
pub enum NoSingleZip {
    /// No `.zip` Asset present.
    None,
    /// More than one `.zip` Asset (their names, for the message).
    Multiple(Vec<String>),
}

/// Pick the one `.zip` Asset to extract for an Entry. Exactly one is required;
/// zero or more than one is an error (surfaced per package).
pub fn single_zip(assets: &[Asset]) -> Result<&Asset, NoSingleZip> {
    let zips: Vec<&Asset> = assets
        .iter()
        .filter(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        .collect();
    match zips.as_slice() {
        [one] => Ok(one),
        [] => Err(NoSingleZip::None),
        _ => Err(NoSingleZip::Multiple(
            zips.iter().map(|a| a.name.clone()).collect(),
        )),
    }
}

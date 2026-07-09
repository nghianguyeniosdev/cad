//! The Extract Phase: unpack each Entry's archive from its Cache Root version
//! folder into the iOS repo's `PodLocals/<package>/Current`. See ADR 0007.

use crate::domain::{Asset, FailedAsset};

/// The Extract Phase's contribution to the Run Summary: how many archives were
/// unpacked, and the per-package failures collected along the way.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExtractReport {
    pub extracted: usize,
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

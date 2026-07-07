use crate::domain::{Asset, Entry};

/// One Asset to fetch, paired with the Entry it belongs to (which carries the
/// destination folder and Package Version coordinates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAsset {
    pub entry: Entry,
    pub asset: Asset,
}

/// The complete set of Assets to fetch, produced by the Enumerate Phase before
/// any download begins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadPlan {
    pub items: Vec<PlannedAsset>,
}

impl DownloadPlan {
    /// Total number of Assets to fetch.
    pub fn total_files(&self) -> usize {
        self.items.len()
    }

    /// Sum of all Asset sizes, in bytes.
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|item| item.asset.size).sum()
    }

    /// Number of distinct Package Versions (Entries) contributing Assets.
    pub fn package_count(&self) -> usize {
        let mut keys: Vec<_> = self
            .items
            .iter()
            .map(|item| {
                (
                    &item.entry.namespace,
                    &item.entry.package,
                    &item.entry.version,
                )
            })
            .collect();
        keys.sort();
        keys.dedup();
        keys.len()
    }
}

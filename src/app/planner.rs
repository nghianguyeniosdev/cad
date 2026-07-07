use std::sync::Arc;

use crate::domain::{DownloadPlan, FailureKind, Manifest, PlannedAsset};
use crate::ports::PackageSource;

/// The Enumerate Phase: lists every Entry's Assets and aggregates them into a
/// Download Plan. A listing failure aborts enumeration (honest totals can't be
/// produced without a complete listing).
pub struct Planner {
    source: Arc<dyn PackageSource>,
}

impl Planner {
    pub fn new(source: Arc<dyn PackageSource>) -> Self {
        Self { source }
    }

    pub async fn plan(&self, manifest: &Manifest) -> Result<DownloadPlan, FailureKind> {
        let mut items = Vec::new();
        for entry in &manifest.packages {
            let assets = self.source.list_assets(entry).await?;
            for asset in assets {
                items.push(PlannedAsset {
                    entry: entry.clone(),
                    asset,
                });
            }
        }
        Ok(DownloadPlan { items })
    }
}

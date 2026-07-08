use std::sync::Arc;

use futures::stream::{self, StreamExt, TryStreamExt};

use crate::domain::{DownloadPlan, Failure, Manifest, PlannedAsset};
use crate::ports::PackageSource;

/// The Enumerate Phase: lists every Entry's Assets and aggregates them into a
/// Download Plan. A listing failure aborts enumeration (honest totals can't be
/// produced without a complete listing).
pub struct Planner {
    source: Arc<dyn PackageSource>,
    concurrency: usize,
}

impl Planner {
    pub fn new(source: Arc<dyn PackageSource>, concurrency: usize) -> Self {
        Self {
            source,
            concurrency: concurrency.max(1),
        }
    }

    pub async fn plan(&self, manifest: &Manifest) -> Result<DownloadPlan, Failure> {
        // List every Entry's Assets concurrently (bounded), then flatten into
        // planned items. Any listing failure aborts the whole enumerate.
        let per_entry: Vec<Vec<PlannedAsset>> = stream::iter(&manifest.packages)
            .map(|entry| async move {
                let assets = self.source.list_assets(entry).await?;
                Ok::<_, Failure>(
                    assets
                        .into_iter()
                        .map(|asset| PlannedAsset {
                            entry: entry.clone(),
                            asset,
                        })
                        .collect(),
                )
            })
            .buffer_unordered(self.concurrency)
            .try_collect()
            .await?;

        let items = per_entry.into_iter().flatten().collect();
        Ok(DownloadPlan { items })
    }
}

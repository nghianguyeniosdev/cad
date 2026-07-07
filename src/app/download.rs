use std::sync::Arc;

use futures::stream::{self, StreamExt};
use md5::{Digest, Md5};

use crate::app::Planner;
use crate::domain::{Asset, DownloadPlan, Entry, FailureKind, Manifest, RunSummary};
use crate::ports::{FileStore, PackageSource};

/// The default number of Assets downloaded concurrently.
pub const DEFAULT_CONCURRENCY: usize = 10;

/// Orchestrates the download of a Manifest through the ports.
pub struct DownloadService {
    source: Arc<dyn PackageSource>,
    files: Arc<dyn FileStore>,
    concurrency: usize,
}

impl DownloadService {
    pub fn new(source: Arc<dyn PackageSource>, files: Arc<dyn FileStore>) -> Self {
        Self {
            source,
            files,
            concurrency: DEFAULT_CONCURRENCY,
        }
    }

    /// Set the maximum number of Assets downloaded concurrently.
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// Run the Enumerate Phase, producing the Download Plan. Aborts (returns an
    /// error) if any Entry's listing fails.
    pub async fn enumerate(&self, manifest: &Manifest) -> Result<DownloadPlan, FailureKind> {
        Planner::new(self.source.clone()).plan(manifest).await
    }

    /// Run the Download Phase over a plan, fetching Assets concurrently (bounded
    /// by the configured concurrency), verifying MD5, and returning a summary.
    pub async fn download(&self, plan: &DownloadPlan) -> RunSummary {
        let outcomes = stream::iter(plan.items.iter())
            .map(|item| async move {
                self.download_one(&item.entry, &item.asset)
                    .await
                    .map_err(|_| item.asset.name.clone())
            })
            .buffer_unordered(self.concurrency)
            .collect::<Vec<_>>()
            .await;

        let mut summary = RunSummary::default();
        for outcome in outcomes {
            match outcome {
                Ok(bytes) => {
                    summary.downloaded += 1;
                    summary.bytes += bytes;
                }
                Err(name) => {
                    summary.failed += 1;
                    summary.failed_assets.push(name);
                }
            }
        }
        summary
    }

    /// Enumerate then download. On an enumerate failure, returns a summary with
    /// a single failure recorded (the run is aborted before any download).
    pub async fn run(&self, manifest: &Manifest) -> RunSummary {
        match self.enumerate(manifest).await {
            Ok(plan) => self.download(&plan).await,
            Err(_) => RunSummary {
                failed: 1,
                failed_assets: vec!["enumerate failed".to_string()],
                ..RunSummary::default()
            },
        }
    }

    /// Fetch one Asset, verify its MD5, and persist it. Returns the byte count.
    async fn download_one(&self, entry: &Entry, asset: &Asset) -> Result<u64, FailureKind> {
        let bytes = self.source.fetch_asset(entry, asset).await?;

        let mut hasher = Md5::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        if actual != asset.expected_md5 {
            return Err(FailureKind::Fatal);
        }

        let dest = entry.dest.join(&asset.name);
        self.files.write(&dest, &bytes).await?;
        Ok(bytes.len() as u64)
    }
}

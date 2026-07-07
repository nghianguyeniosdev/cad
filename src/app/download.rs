use std::sync::Arc;

use futures::stream::{self, StreamExt};
use md5::{Digest, Md5};

use crate::app::{Planner, RetryPolicy};
use crate::domain::{Asset, AssetOutcome, DownloadPlan, Entry, FailureKind, Manifest, RunSummary};
use crate::ports::{FileStore, PackageSource};

/// The default number of Assets downloaded concurrently.
pub const DEFAULT_CONCURRENCY: usize = 10;

/// Orchestrates the download of a Manifest through the ports.
pub struct DownloadService {
    source: Arc<dyn PackageSource>,
    files: Arc<dyn FileStore>,
    concurrency: usize,
    retry: RetryPolicy,
}

impl DownloadService {
    pub fn new(source: Arc<dyn PackageSource>, files: Arc<dyn FileStore>) -> Self {
        Self {
            source,
            files,
            concurrency: DEFAULT_CONCURRENCY,
            retry: RetryPolicy::default(),
        }
    }

    /// Set the maximum number of Assets downloaded concurrently.
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// Set the retry policy for `Transient` failures.
    pub fn with_retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
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
                (
                    item.asset.name.clone(),
                    self.download_one(&item.entry, &item.asset).await,
                )
            })
            .buffer_unordered(self.concurrency)
            .collect::<Vec<_>>()
            .await;

        let mut summary = RunSummary::default();
        for (name, outcome) in outcomes {
            match outcome {
                AssetOutcome::Downloaded(bytes) => {
                    summary.downloaded += 1;
                    summary.bytes += bytes;
                }
                AssetOutcome::Cached => summary.cached += 1,
                AssetOutcome::Failed(_) => {
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

    /// Fetch one Asset, verify its MD5, and persist it. Skips (as `Cached`) when
    /// a file already present at `dest` matches the expected MD5.
    async fn download_one(&self, entry: &Entry, asset: &Asset) -> AssetOutcome {
        let dest = entry.dest.join(&asset.name);

        if self.files.existing_md5(&dest).await.as_deref() == Some(asset.expected_md5.as_str()) {
            return AssetOutcome::Cached;
        }

        let mut attempt = 0;
        loop {
            match self.fetch_verify_write(entry, asset, &dest).await {
                Ok(bytes) => return AssetOutcome::Downloaded(bytes),
                Err(FailureKind::Transient) if attempt < self.retry.max_retries => {
                    attempt += 1;
                    if !self.retry.backoff.is_zero() {
                        tokio::time::sleep(self.retry.backoff).await;
                    }
                }
                Err(kind) => return AssetOutcome::Failed(kind),
            }
        }
    }

    /// Fetch the Asset's bytes, verify MD5, and write atomically. A mismatch is
    /// a `Transient` failure (a corrupt transfer is retryable).
    async fn fetch_verify_write(
        &self,
        entry: &Entry,
        asset: &Asset,
        dest: &std::path::Path,
    ) -> Result<u64, FailureKind> {
        let bytes = self.source.fetch_asset(entry, asset).await?;

        let mut hasher = Md5::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        if actual != asset.expected_md5 {
            return Err(FailureKind::Transient);
        }

        self.files.write(dest, &bytes).await?;
        Ok(bytes.len() as u64)
    }
}

use std::sync::Arc;

use futures::stream::{self, StreamExt};
use md5::{Digest, Md5};

use crate::app::session::NoLoginAuthenticator;
use crate::app::{Planner, RetryPolicy, SessionCoordinator};
use crate::domain::{
    Asset, AssetOutcome, DownloadPlan, Entry, FailedAsset, Failure, FailureKind, Manifest,
    RunSummary,
};
use crate::ports::{Authenticator, FileStore, NoopReporter, PackageSource, ProgressReporter};

/// The default number of Assets downloaded concurrently.
pub const DEFAULT_CONCURRENCY: usize = 10;

/// Orchestrates the download of a Manifest through the ports.
pub struct DownloadService {
    source: Arc<dyn PackageSource>,
    files: Arc<dyn FileStore>,
    reporter: Arc<dyn ProgressReporter>,
    coordinator: Arc<SessionCoordinator>,
    concurrency: usize,
    retry: RetryPolicy,
}

impl DownloadService {
    pub fn new(source: Arc<dyn PackageSource>, files: Arc<dyn FileStore>) -> Self {
        Self {
            source,
            files,
            reporter: Arc::new(NoopReporter),
            coordinator: Arc::new(SessionCoordinator::new(
                Arc::new(NoLoginAuthenticator),
                None,
            )),
            concurrency: DEFAULT_CONCURRENCY,
            retry: RetryPolicy::default(),
        }
    }

    /// Configure mid-run re-login recovery with the given authenticator/profile.
    pub fn with_authenticator(
        mut self,
        authenticator: Arc<dyn Authenticator>,
        profile: Option<String>,
    ) -> Self {
        self.coordinator = Arc::new(SessionCoordinator::new(authenticator, profile));
        self
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

    /// Set the progress reporter (defaults to a no-op reporter).
    pub fn with_reporter(mut self, reporter: Arc<dyn ProgressReporter>) -> Self {
        self.reporter = reporter;
        self
    }

    /// Run the Enumerate Phase, producing the Download Plan. Aborts (returns an
    /// error) if any Entry's listing fails.
    pub async fn enumerate(&self, manifest: &Manifest) -> Result<DownloadPlan, Failure> {
        Planner::new(self.source.clone()).plan(manifest).await
    }

    /// Run the Download Phase over a plan, fetching Assets concurrently (bounded
    /// by the configured concurrency), verifying MD5, and returning a summary.
    pub async fn download(&self, plan: &DownloadPlan) -> RunSummary {
        self.reporter.start(plan.total_files(), plan.total_bytes());

        let outcomes = stream::iter(plan.items.iter().enumerate())
            .map(|(index, item)| async move {
                (
                    item.asset.name.clone(),
                    self.download_one(index, &item.entry, &item.asset).await,
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
                AssetOutcome::Failed(failure) => {
                    summary.failed += 1;
                    summary.failed_assets.push(FailedAsset {
                        name,
                        reason: failure.message,
                    });
                }
            }
        }
        self.reporter.finish(&summary);
        summary
    }

    /// Enumerate then download. On an enumerate failure, returns a summary with
    /// a single failure recorded (the run is aborted before any download).
    pub async fn run(&self, manifest: &Manifest) -> RunSummary {
        match self.enumerate(manifest).await {
            Ok(plan) => self.download(&plan).await,
            Err(failure) => RunSummary {
                failed: 1,
                failed_assets: vec![FailedAsset {
                    name: "(enumerate)".to_string(),
                    reason: failure.message,
                }],
                ..RunSummary::default()
            },
        }
    }

    /// Fetch one Asset, verify its MD5, and persist it. Skips (as `Cached`) when
    /// a file already present at `dest` matches the expected MD5. Reports its
    /// lifecycle to the progress reporter.
    async fn download_one(&self, index: usize, entry: &Entry, asset: &Asset) -> AssetOutcome {
        self.reporter.asset_started(index, &asset.name, asset.size);

        let dest = entry.dest.join(&asset.name);
        let outcome = if self.files.existing_md5(&dest).await.as_deref()
            == Some(asset.expected_md5.as_str())
        {
            AssetOutcome::Cached
        } else {
            self.download_with_retry(index, entry, asset, &dest).await
        };

        self.reporter.asset_finished(index, &outcome);
        outcome
    }

    /// Attempt `fetch_verify_write`, retrying `Transient` failures per policy.
    async fn download_with_retry(
        &self,
        index: usize,
        entry: &Entry,
        asset: &Asset,
        dest: &std::path::Path,
    ) -> AssetOutcome {
        let mut attempt = 0;
        loop {
            let generation = self.coordinator.generation();
            match self.fetch_verify_write(index, entry, asset, dest).await {
                Ok(bytes) => {
                    self.coordinator.note_progress();
                    return AssetOutcome::Downloaded(bytes);
                }
                // Auth expiry: pause-the-world single-flight re-login, then
                // retry WITHOUT consuming the transient retry budget.
                Err(failure) if failure.kind == FailureKind::AuthExpired => {
                    match self.coordinator.reauth(generation).await {
                        Ok(_) => continue,
                        Err(abort) => return AssetOutcome::Failed(abort),
                    }
                }
                Err(failure)
                    if failure.kind == FailureKind::Transient
                        && attempt < self.retry.max_retries =>
                {
                    attempt += 1;
                    if !self.retry.backoff.is_zero() {
                        tokio::time::sleep(self.retry.backoff).await;
                    }
                }
                Err(failure) => return AssetOutcome::Failed(failure),
            }
        }
    }

    /// Fetch the Asset's bytes, verify MD5, and write atomically. A mismatch is
    /// a `Transient` failure (a corrupt transfer is retryable).
    async fn fetch_verify_write(
        &self,
        index: usize,
        entry: &Entry,
        asset: &Asset,
        dest: &std::path::Path,
    ) -> Result<u64, Failure> {
        let mut stream = self.source.fetch_asset(entry, asset).await?;

        let mut hasher = Md5::new();
        let mut buffer = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            self.reporter.asset_advanced(index, chunk.len() as u64);
            buffer.extend_from_slice(&chunk);
        }

        let actual = hex::encode(hasher.finalize());
        if actual != asset.expected_md5 {
            return Err(Failure::transient(format!(
                "md5 mismatch for {}: expected {}, got {actual}",
                asset.name, asset.expected_md5
            )));
        }

        self.files.write(dest, &buffer).await?;
        Ok(buffer.len() as u64)
    }
}

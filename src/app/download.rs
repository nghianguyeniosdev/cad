use std::sync::Arc;

use md5::{Digest, Md5};

use crate::domain::{Asset, Entry, FailureKind, Manifest, RunSummary};
use crate::ports::{FileStore, PackageSource};

/// Orchestrates the download of a Manifest through the ports.
pub struct DownloadService {
    source: Arc<dyn PackageSource>,
    files: Arc<dyn FileStore>,
}

impl DownloadService {
    pub fn new(source: Arc<dyn PackageSource>, files: Arc<dyn FileStore>) -> Self {
        Self { source, files }
    }

    /// Download every Asset of every Entry, verifying MD5, and return a summary.
    pub async fn run(&self, manifest: &Manifest) -> RunSummary {
        let mut summary = RunSummary::default();

        for entry in &manifest.packages {
            let assets = match self.source.list_assets(entry).await {
                Ok(assets) => assets,
                Err(_) => {
                    summary.failed += 1;
                    summary
                        .failed_assets
                        .push(format!("{} (listing failed)", entry.package));
                    continue;
                }
            };

            for asset in assets {
                match self.download_one(entry, &asset).await {
                    Ok(bytes) => {
                        summary.downloaded += 1;
                        summary.bytes += bytes;
                    }
                    Err(_) => {
                        summary.failed += 1;
                        summary.failed_assets.push(asset.name.clone());
                    }
                }
            }
        }

        summary
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

use async_trait::async_trait;

use crate::domain::{Asset, Entry, FailureKind};

/// The CodeArtifact seam: enumerate a Package Version's Assets and fetch their
/// bytes. Unified per ADR 0004 (listing and fetching are the same system).
#[async_trait]
pub trait PackageSource: Send + Sync {
    /// List all Assets of the Entry's Package Version (the Enumerate Phase).
    async fn list_assets(&self, entry: &Entry) -> Result<Vec<Asset>, FailureKind>;

    /// Fetch the full bytes of one Asset.
    ///
    /// Returns the whole payload for now; streaming (for byte-level progress)
    /// is introduced with the progress slice.
    async fn fetch_asset(&self, entry: &Entry, asset: &Asset) -> Result<Vec<u8>, FailureKind>;
}

use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::domain::{Asset, Entry, Failure};

/// A stream of an Asset's bytes, delivered in chunks so callers can report
/// byte-level progress and hash incrementally. Per-chunk errors are `Failure`s.
pub type AssetStream = BoxStream<'static, Result<Vec<u8>, Failure>>;

/// The CodeArtifact seam: enumerate a Package Version's Assets and fetch their
/// bytes. Unified per ADR 0004 (listing and fetching are the same system).
#[async_trait]
pub trait PackageSource: Send + Sync {
    /// List all Assets of the Entry's Package Version (the Enumerate Phase).
    async fn list_assets(&self, entry: &Entry) -> Result<Vec<Asset>, Failure>;

    /// Begin fetching one Asset, returning a stream of its bytes in chunks. The
    /// initial `Result` covers request setup; per-chunk failures arrive as
    /// stream items.
    async fn fetch_asset(&self, entry: &Entry, asset: &Asset) -> Result<AssetStream, Failure>;
}

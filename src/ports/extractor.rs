use async_trait::async_trait;
use std::path::Path;

use crate::domain::Failure;

/// The archive-extraction seam (Extract Phase). The adapter wipes `into` and
/// unzips `archive` into it; the wipe+unzip is the adapter's responsibility
/// (analogous to `FileStore` owning write atomicity). See ADR 0007.
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Wipe `into`, then unzip `archive` into it.
    async fn extract(&self, archive: &Path, into: &Path) -> Result<(), Failure>;
}

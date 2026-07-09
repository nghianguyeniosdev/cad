use async_trait::async_trait;
use std::path::Path;

use crate::domain::Failure;

/// The Extraction Marker seam. Reads/writes `<package_dir>/.acd-version` and
/// stat-walks `<package_dir>/Current` to decide whether the Extracted Copy is
/// already the pinned version and untampered. See ADR 0007.
#[async_trait]
pub trait MarkerStore: Send + Sync {
    /// Whether extraction can be skipped: a marker exists for `version` and the
    /// current stat fingerprint of `Current` matches it.
    async fn is_current(&self, package_dir: &Path, version: &str) -> bool;

    /// Record the marker for the freshly-extracted `Current` at `package_dir`.
    async fn record(&self, package_dir: &Path, version: &str) -> Result<(), Failure>;
}

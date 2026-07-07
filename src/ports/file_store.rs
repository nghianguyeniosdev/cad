use async_trait::async_trait;
use std::path::Path;

use crate::domain::FailureKind;

/// The filesystem seam. The adapter is responsible for atomicity (write to a
/// `.part` temp file, then rename into place); callers only ever hand it bytes
/// that have already passed MD5 verification.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// Persist verified bytes at `dest`, atomically.
    async fn write(&self, dest: &Path, bytes: &[u8]) -> Result<(), FailureKind>;
}

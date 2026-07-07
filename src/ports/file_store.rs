use async_trait::async_trait;
use std::path::Path;

use crate::domain::FailureKind;

/// The filesystem seam. The adapter is responsible for atomicity (write to a
/// `.part` temp file, then rename into place); callers only ever hand it bytes
/// that have already passed MD5 verification.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// The MD5 (lowercase hex) of the file already at `dest`, or `None` if no
    /// file is present. Used for Verify-and-Skip.
    async fn existing_md5(&self, dest: &Path) -> Option<String>;

    /// Persist verified bytes at `dest`, atomically.
    async fn write(&self, dest: &Path, bytes: &[u8]) -> Result<(), FailureKind>;
}

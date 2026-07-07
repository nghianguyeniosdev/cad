use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::domain::FailureKind;
use crate::ports::FileStore;

/// A `FileStore` backed by the local filesystem. Writes go to a `<dest>.part`
/// temp file first and are atomically renamed into place, so an interrupted
/// write never leaves a corrupt file at the final path.
pub struct LocalFileStore;

#[async_trait]
impl FileStore for LocalFileStore {
    async fn write(&self, dest: &Path, bytes: &[u8]) -> Result<(), FailureKind> {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|_| FailureKind::Fatal)?;
        }

        let part = part_path(dest);
        tokio::fs::write(&part, bytes)
            .await
            .map_err(|_| FailureKind::Fatal)?;
        tokio::fs::rename(&part, dest)
            .await
            .map_err(|_| FailureKind::Fatal)?;
        Ok(())
    }
}

/// The sibling temp path (`<dest>.part`) used while a write is in flight.
fn part_path(dest: &Path) -> PathBuf {
    let mut p = dest.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

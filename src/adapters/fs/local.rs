use std::path::{Path, PathBuf};

use async_trait::async_trait;
use md5::{Digest, Md5};

use crate::domain::Failure;
use crate::ports::FileStore;

/// A `FileStore` backed by the local filesystem. Writes go to a `<dest>.part`
/// temp file first and are atomically renamed into place, so an interrupted
/// write never leaves a corrupt file at the final path.
pub struct LocalFileStore;

#[async_trait]
impl FileStore for LocalFileStore {
    async fn existing_md5(&self, dest: &Path) -> Option<String> {
        let bytes = tokio::fs::read(dest).await.ok()?;
        let mut hasher = Md5::new();
        hasher.update(&bytes);
        Some(hex::encode(hasher.finalize()))
    }

    async fn write(&self, dest: &Path, bytes: &[u8]) -> Result<(), Failure> {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Failure::fatal(format!("create {}: {e}", parent.display())))?;
        }

        let part = part_path(dest);
        tokio::fs::write(&part, bytes)
            .await
            .map_err(|e| Failure::fatal(format!("write {}: {e}", part.display())))?;
        tokio::fs::rename(&part, dest)
            .await
            .map_err(|e| Failure::fatal(format!("rename to {}: {e}", dest.display())))?;
        Ok(())
    }
}

/// The sibling temp path (`<dest>.part`) used while a write is in flight.
fn part_path(dest: &Path) -> PathBuf {
    let mut p = dest.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

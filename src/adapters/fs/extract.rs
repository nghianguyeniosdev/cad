use std::path::Path;

use async_trait::async_trait;

use crate::domain::Failure;
use crate::ports::Extractor;

/// An `Extractor` backed by the local filesystem: it wipes `into` and unzips
/// `archive` into it by shelling out to `unzip` (matching the iOS scripts;
/// avoids a new dependency). See ADR 0007.
pub struct LocalExtractor;

#[async_trait]
impl Extractor for LocalExtractor {
    async fn extract(&self, archive: &Path, into: &Path) -> Result<(), Failure> {
        // Wipe: remove any prior extraction, then recreate an empty Current so
        // the result is exactly this archive's contents and nothing else.
        if into.exists() {
            tokio::fs::remove_dir_all(into)
                .await
                .map_err(|e| Failure::fatal(format!("wipe {}: {e}", into.display())))?;
        }
        tokio::fs::create_dir_all(into)
            .await
            .map_err(|e| Failure::fatal(format!("create {}: {e}", into.display())))?;

        // `-o` overwrite, `-q` quiet, `-d` target dir.
        let output = tokio::process::Command::new("unzip")
            .arg("-o")
            .arg("-q")
            .arg(archive)
            .arg("-d")
            .arg(into)
            .output()
            .await
            .map_err(|e| Failure::fatal(format!("spawn unzip: {e}")))?;

        if !output.status.success() {
            // On failure leave nothing behind (an empty/partial Current is worse
            // than none — it would look "extracted").
            let _ = tokio::fs::remove_dir_all(into).await;
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Failure::fatal(format!(
                "unzip {} failed: {}",
                archive.display(),
                stderr.trim()
            )));
        }
        Ok(())
    }
}

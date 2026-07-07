//! Composition root: constructs concrete adapters and injects them into `app`
//! services. The only place (besides `cli`/`main`) allowed to depend on every
//! layer. See ADR 0004.

use std::sync::Arc;

use crate::adapters::aws::CodeArtifactSource;
use crate::adapters::fs::LocalFileStore;
use crate::app::DownloadService;
use crate::domain::{Manifest, RunSummary};
use crate::ports::{FileStore, PackageSource};

/// Wire the real adapters and run a download for the given Manifest.
pub async fn run_download(
    manifest: &Manifest,
    profile: Option<String>,
) -> Result<RunSummary, String> {
    let source: Arc<dyn PackageSource> = Arc::new(
        CodeArtifactSource::new(manifest.connection.clone(), profile)
            .await
            .map_err(|_| "failed to initialize AWS CodeArtifact client".to_string())?,
    );
    let files: Arc<dyn FileStore> = Arc::new(LocalFileStore);

    Ok(DownloadService::new(source, files).run(manifest).await)
}

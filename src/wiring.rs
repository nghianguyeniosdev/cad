//! Composition root: constructs concrete adapters and injects them into `app`
//! services. The only place (besides `cli`/`main`) allowed to depend on every
//! layer. See ADR 0004.

use std::sync::Arc;

use crate::adapters::aws::CodeArtifactSource;
use crate::adapters::fs::LocalFileStore;
use crate::app::DownloadService;
use crate::domain::ConnectionSettings;
use crate::ports::{FileStore, PackageSource};

/// Wire the real adapters into a `DownloadService` for the given connection.
pub async fn build_download_service(
    connection: ConnectionSettings,
    profile: Option<String>,
    concurrency: usize,
) -> Result<DownloadService, String> {
    let source: Arc<dyn PackageSource> = Arc::new(
        CodeArtifactSource::new(connection, profile)
            .await
            .map_err(|_| "failed to initialize AWS CodeArtifact client".to_string())?,
    );
    let files: Arc<dyn FileStore> = Arc::new(LocalFileStore);

    Ok(DownloadService::new(source, files).with_concurrency(concurrency))
}

//! Composition root: constructs concrete adapters and injects them into `app`
//! services. The only place (besides `cli`/`main`) allowed to depend on every
//! layer. See ADR 0004.

use std::io::IsTerminal;
use std::sync::Arc;

use crate::adapters::aws::CodeArtifactSource;
use crate::adapters::cache::{CachingPackageSource, SqliteAssetListCache};
use crate::adapters::fs::LocalFileStore;
use crate::adapters::progress::{IndicatifReporter, PlainReporter};
use crate::app::DownloadService;
use crate::domain::ConnectionSettings;
use crate::ports::{AssetListCache, Authenticator, FileStore, PackageSource, ProgressReporter};

/// Wire the real adapters into a `DownloadService` for the given connection.
/// Chooses the hybrid `indicatif` reporter on a TTY, plain lines otherwise, and
/// installs `authenticator` for mid-run re-login recovery.
pub async fn build_download_service(
    connection: ConnectionSettings,
    profile: Option<String>,
    concurrency: usize,
    authenticator: Arc<dyn Authenticator>,
) -> Result<DownloadService, String> {
    let aws: Arc<dyn PackageSource> = Arc::new(
        CodeArtifactSource::new(connection.clone(), profile.clone())
            .await
            .map_err(|failure| format!("failed to initialize AWS client: {}", failure.message))?,
    );
    // Cache immutable Package Versions' asset lists to skip repeat listings.
    let cache: Arc<dyn AssetListCache> = Arc::new(SqliteAssetListCache::open(
        &SqliteAssetListCache::default_path(),
    ));
    let source: Arc<dyn PackageSource> =
        Arc::new(CachingPackageSource::new(aws, cache, connection));

    let files: Arc<dyn FileStore> = Arc::new(LocalFileStore);

    let reporter: Arc<dyn ProgressReporter> = if std::io::stderr().is_terminal() {
        Arc::new(IndicatifReporter::new())
    } else {
        Arc::new(PlainReporter)
    };

    Ok(DownloadService::new(source, files)
        .with_concurrency(concurrency)
        .with_reporter(reporter)
        .with_authenticator(authenticator, profile))
}

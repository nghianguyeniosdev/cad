use async_trait::async_trait;
use aws_sdk_codeartifact::error::{DisplayErrorContext, ProvideErrorMetadata, SdkError};
use aws_sdk_codeartifact::types::{HashAlgorithm, PackageFormat};
use aws_sdk_codeartifact::Client;
use futures::stream::StreamExt;
use tokio_util::io::ReaderStream;

use crate::domain::{Asset, ConnectionSettings, Entry, Failure, FailureKind};
use crate::ports::{AssetStream, PackageSource};

/// A `PackageSource` backed by AWS CodeArtifact generic packages, using the
/// AWS SDK for Rust. Credentials/region come from the resolved AWS profile
/// (SSO session). SDK errors are classified into `FailureKind` (see `classify`)
/// so the SessionCoordinator can recover from an expired session.
pub struct CodeArtifactSource {
    client: Client,
    connection: ConnectionSettings,
}

impl CodeArtifactSource {
    /// Build the client from the given profile (SSO session assumed valid).
    pub async fn new(
        connection: ConnectionSettings,
        profile: Option<String>,
    ) -> Result<Self, Failure> {
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(profile) = profile.as_deref() {
            loader = loader.profile_name(profile);
        }
        if let Some(region) = connection.region.clone() {
            loader = loader.region(aws_config::Region::new(region));
        }
        let config = loader.load().await;
        Ok(Self {
            client: Client::new(&config),
            connection,
        })
    }
}

#[async_trait]
impl PackageSource for CodeArtifactSource {
    async fn list_assets(&self, entry: &Entry) -> Result<Vec<Asset>, Failure> {
        let mut request = self
            .client
            .list_package_version_assets()
            .domain(&self.connection.domain)
            .domain_owner(&self.connection.domain_owner)
            .repository(&self.connection.repository)
            .format(PackageFormat::Generic)
            .package(&entry.package)
            .package_version(&entry.version);
        if let Some(namespace) = &entry.namespace {
            request = request.namespace(namespace);
        }

        let response = request.send().await.map_err(|err| {
            Failure::new(
                classify(&err),
                format!(
                    "list assets for {}: {}",
                    describe_entry(entry),
                    sdk_message(&err)
                ),
            )
        })?;

        let assets = response
            .assets()
            .iter()
            .map(|summary| Asset {
                name: summary.name().to_string(),
                size: summary.size().unwrap_or(0).max(0) as u64,
                expected_md5: summary
                    .hashes()
                    .and_then(|hashes| hashes.get(&HashAlgorithm::Md5))
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        Ok(assets)
    }

    async fn fetch_asset(&self, entry: &Entry, asset: &Asset) -> Result<AssetStream, Failure> {
        let mut request = self
            .client
            .get_package_version_asset()
            .domain(&self.connection.domain)
            .domain_owner(&self.connection.domain_owner)
            .repository(&self.connection.repository)
            .format(PackageFormat::Generic)
            .package(&entry.package)
            .package_version(&entry.version)
            .asset(&asset.name);
        if let Some(namespace) = &entry.namespace {
            request = request.namespace(namespace);
        }

        let response = request.send().await.map_err(|err| {
            Failure::new(
                classify(&err),
                format!(
                    "fetch {} of {}: {}",
                    asset.name,
                    describe_entry(entry),
                    sdk_message(&err)
                ),
            )
        })?;

        let asset_name = asset.name.clone();
        let stream = ReaderStream::new(response.asset.into_async_read()).map(move |chunk| {
            chunk
                .map(|bytes| bytes.to_vec())
                .map_err(|err| Failure::transient(format!("read {asset_name}: {err}")))
        });
        Ok(Box::pin(stream))
    }
}

/// Classify an SDK error into a `FailureKind`. An expired SSO token (surfaced
/// as a credential-resolution failure or `ExpiredTokenException`) is
/// `AuthExpired` so the SessionCoordinator can re-login; throttling and
/// dispatch/timeout are `Transient`; everything else (validation, not-found,
/// permissions) is `Fatal` — re-login wouldn't help and would loop.
fn classify<E, R>(err: &SdkError<E, R>) -> FailureKind
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
    R: std::fmt::Debug,
{
    if let Some(code) = err.as_service_error().and_then(ProvideErrorMetadata::code) {
        return match code {
            "ExpiredTokenException" => FailureKind::AuthExpired,
            "ThrottlingException" | "TooManyRequestsException" => FailureKind::Transient,
            _ => FailureKind::Fatal,
        };
    }
    // Non-service error (dispatch/timeout/credential resolution).
    let message = sdk_message(err).to_lowercase();
    if message.contains("expired")
        || message.contains("sso")
        || message.contains("token")
        || message.contains("credential")
    {
        FailureKind::AuthExpired
    } else {
        FailureKind::Transient
    }
}

/// Extract the concise service-error message from an SDK error, falling back to
/// the full context for non-service (dispatch/timeout) errors.
fn sdk_message<E, R>(err: &SdkError<E, R>) -> String
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
    R: std::fmt::Debug,
{
    match err
        .as_service_error()
        .and_then(ProvideErrorMetadata::message)
    {
        Some(message) => message.to_string(),
        None => DisplayErrorContext(err).to_string(),
    }
}

/// A short "namespace/package@version" description of an Entry for messages.
fn describe_entry(entry: &Entry) -> String {
    match &entry.namespace {
        Some(ns) => format!("{ns}/{}@{}", entry.package, entry.version),
        None => format!("{}@{}", entry.package, entry.version),
    }
}

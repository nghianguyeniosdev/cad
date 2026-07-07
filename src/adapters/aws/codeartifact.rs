use async_trait::async_trait;
use aws_sdk_codeartifact::types::{HashAlgorithm, PackageFormat};
use aws_sdk_codeartifact::Client;

use crate::domain::{Asset, ConnectionSettings, Entry, FailureKind};
use crate::ports::PackageSource;

/// A `PackageSource` backed by AWS CodeArtifact generic packages, using the
/// AWS SDK for Rust. Credentials/region come from the resolved AWS profile
/// (SSO session). Error classification (auth vs transient vs fatal) is
/// deliberately minimal here — richer classification arrives with the
/// SessionCoordinator slice.
pub struct CodeArtifactSource {
    client: Client,
    connection: ConnectionSettings,
}

impl CodeArtifactSource {
    /// Build the client from the given profile (SSO session assumed valid).
    pub async fn new(
        connection: ConnectionSettings,
        profile: Option<String>,
    ) -> Result<Self, FailureKind> {
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
    async fn list_assets(&self, entry: &Entry) -> Result<Vec<Asset>, FailureKind> {
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

        let response = request.send().await.map_err(|_| FailureKind::Fatal)?;

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

    async fn fetch_asset(&self, entry: &Entry, asset: &Asset) -> Result<Vec<u8>, FailureKind> {
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

        let response = request.send().await.map_err(|_| FailureKind::Fatal)?;
        let bytes = response
            .asset
            .collect()
            .await
            .map_err(|_| FailureKind::Transient)?
            .into_bytes();
        Ok(bytes.to_vec())
    }
}

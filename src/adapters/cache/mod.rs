pub mod sqlite;

pub use sqlite::SqliteAssetListCache;

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::{Asset, ConnectionSettings, Entry, Failure};
use crate::ports::{AssetListCache, AssetListKey, AssetStream, PackageSource};

/// Whether a Package Version is immutable and therefore safe to cache. Snapshot
/// versions are mutable (re-published in place), so they are never cached.
pub fn is_cacheable(version: &str) -> bool {
    !version.to_lowercase().contains("snapshot")
}

/// A `PackageSource` decorator that serves `list_assets` from an
/// `AssetListCache` for immutable Package Versions, falling back to the inner
/// source on a miss (and populating the cache). `fetch_asset` delegates.
pub struct CachingPackageSource {
    inner: Arc<dyn PackageSource>,
    cache: Arc<dyn AssetListCache>,
    connection: ConnectionSettings,
    refresh: bool,
}

impl CachingPackageSource {
    pub fn new(
        inner: Arc<dyn PackageSource>,
        cache: Arc<dyn AssetListCache>,
        connection: ConnectionSettings,
    ) -> Self {
        Self {
            inner,
            cache,
            connection,
            refresh: false,
        }
    }

    /// When set, bypass the cache read and always re-query the inner source,
    /// repopulating the cache with the fresh result.
    pub fn with_refresh(mut self, refresh: bool) -> Self {
        self.refresh = refresh;
        self
    }

    fn key(&self, entry: &Entry) -> AssetListKey {
        AssetListKey {
            domain: self.connection.domain.clone(),
            domain_owner: self.connection.domain_owner.clone(),
            repository: self.connection.repository.clone(),
            namespace: entry.namespace.clone(),
            package: entry.package.clone(),
            version: entry.version.clone(),
        }
    }
}

#[async_trait]
impl PackageSource for CachingPackageSource {
    async fn list_assets(&self, entry: &Entry) -> Result<Vec<Asset>, Failure> {
        // Snapshot versions are mutable — never cache them.
        if !is_cacheable(&entry.version) {
            return self.inner.list_assets(entry).await;
        }

        let key = self.key(entry);
        if !self.refresh {
            if let Some(cached) = self.cache.get(&key).await {
                return Ok(cached);
            }
        }

        let assets = self.inner.list_assets(entry).await?;
        self.cache.put(&key, &assets).await;
        Ok(assets)
    }

    async fn fetch_asset(&self, entry: &Entry, asset: &Asset) -> Result<AssetStream, Failure> {
        self.inner.fetch_asset(entry, asset).await
    }
}

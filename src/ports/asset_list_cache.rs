use async_trait::async_trait;

use crate::domain::Asset;

/// Identifies one immutable Package Version's asset list within a repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetListKey {
    pub domain: String,
    pub domain_owner: String,
    pub repository: String,
    /// `None` when the package has no namespace.
    pub namespace: Option<String>,
    pub package: String,
    pub version: String,
}

/// The Asset List Cache seam: store/lookup enumerated Assets for immutable
/// Package Versions. Implementations are best-effort — a miss (or a store that
/// silently drops) simply falls back to a live listing.
#[async_trait]
pub trait AssetListCache: Send + Sync {
    /// The cached Assets for `key`, or `None` on a miss.
    async fn get(&self, key: &AssetListKey) -> Option<Vec<Asset>>;

    /// Store the Assets for `key`.
    async fn put(&self, key: &AssetListKey, assets: &[Asset]);
}

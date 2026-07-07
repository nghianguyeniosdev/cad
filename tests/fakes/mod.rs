//! In-memory fakes for the ports, used to drive the app pipeline end-to-end
//! without touching AWS.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;

use acd::domain::{Asset, Entry, FailureKind};
use acd::ports::{FileStore, PackageSource};

/// A `PackageSource` that returns a scripted set of Assets for every Entry and
/// serves each Asset's bytes from an in-memory map (missing name -> `Fatal`).
pub struct FakePackageSource {
    pub assets: Vec<Asset>,
    pub bytes: HashMap<String, Vec<u8>>,
}

#[async_trait]
impl PackageSource for FakePackageSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, FailureKind> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, asset: &Asset) -> Result<Vec<u8>, FailureKind> {
        self.bytes
            .get(&asset.name)
            .cloned()
            .ok_or(FailureKind::Fatal)
    }
}

/// A `FileStore` that records every write in memory so tests can assert what
/// was persisted (and what was not).
#[derive(Default)]
pub struct FakeFileStore {
    pub written: Mutex<HashMap<PathBuf, Vec<u8>>>,
}

#[async_trait]
impl FileStore for FakeFileStore {
    async fn write(&self, dest: &Path, bytes: &[u8]) -> Result<(), FailureKind> {
        self.written
            .lock()
            .unwrap()
            .insert(dest.to_path_buf(), bytes.to_vec());
        Ok(())
    }
}

//! In-memory fakes for the ports, used to drive the app pipeline end-to-end
//! without touching AWS.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

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

/// A `PackageSource` that measures how many `fetch_asset` calls run at once, so
/// tests can assert the Downloader honors its concurrency limit. Each fetch
/// briefly sleeps to create an overlap window.
pub struct ConcurrencyProbeSource {
    pub assets: Vec<Asset>,
    pub in_flight: AtomicUsize,
    pub max_in_flight: AtomicUsize,
}

impl ConcurrencyProbeSource {
    pub fn new(assets: Vec<Asset>) -> Self {
        Self {
            assets,
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl PackageSource for ConcurrencyProbeSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, FailureKind> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<Vec<u8>, FailureKind> {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(b"data".to_vec())
    }
}

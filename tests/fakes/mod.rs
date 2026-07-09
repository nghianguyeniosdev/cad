//! In-memory fakes for the ports, used to drive the app pipeline end-to-end
//! without touching AWS.
//!
//! Shared by multiple integration-test crates; each uses a different subset, so
//! unused-in-one-crate items are expected.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use std::collections::HashSet;

use acd::domain::{Asset, Entry, Failure};
use acd::ports::{
    AssetListCache, AssetListKey, AssetStream, Authenticator, Extractor, FileStore, PackageSource,
    SessionStatus,
};

/// Wrap owned bytes as a single-chunk `AssetStream`.
fn one_chunk(bytes: Vec<u8>) -> AssetStream {
    Box::pin(futures::stream::once(async move { Ok(bytes) }))
}

/// Wrap owned bytes as a multi-chunk `AssetStream` of `chunk_size` bytes each.
pub fn chunked_stream(bytes: Vec<u8>, chunk_size: usize) -> AssetStream {
    let chunks: Vec<Vec<u8>> = bytes
        .chunks(chunk_size.max(1))
        .map(<[u8]>::to_vec)
        .collect();
    Box::pin(futures::stream::iter(chunks.into_iter().map(Ok)))
}

/// A `PackageSource` that returns a scripted set of Assets for every Entry and
/// serves each Asset's bytes from an in-memory map (missing name -> `Fatal`).
pub struct FakePackageSource {
    pub assets: Vec<Asset>,
    pub bytes: HashMap<String, Vec<u8>>,
}

#[async_trait]
impl PackageSource for FakePackageSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, asset: &Asset) -> Result<AssetStream, Failure> {
        match self.bytes.get(&asset.name) {
            Some(bytes) => Ok(one_chunk(bytes.clone())),
            None => Err(Failure::fatal(format!("no bytes for {}", asset.name))),
        }
    }
}

/// A `FileStore` that records every write in memory so tests can assert what
/// was persisted (and what was not). `existing` presets the MD5s of files
/// considered already-present on disk (for Verify-and-Skip tests).
#[derive(Default)]
pub struct FakeFileStore {
    pub written: Mutex<HashMap<PathBuf, Vec<u8>>>,
    pub existing: HashMap<PathBuf, String>,
}

#[async_trait]
impl FileStore for FakeFileStore {
    async fn existing_md5(&self, dest: &Path) -> Option<String> {
        self.existing.get(dest).cloned()
    }

    async fn write(&self, dest: &Path, bytes: &[u8]) -> Result<(), Failure> {
        self.written
            .lock()
            .unwrap()
            .insert(dest.to_path_buf(), bytes.to_vec());
        Ok(())
    }
}

/// An `Extractor` that records every `(archive, into)` call in memory, and can
/// be told to fail for specific archive paths (to exercise per-package failure).
#[derive(Default)]
pub struct FakeExtractor {
    calls: Mutex<Vec<(PathBuf, PathBuf)>>,
    fail_for: HashSet<PathBuf>,
}

impl FakeExtractor {
    /// An extractor that fails for the given archive paths, succeeds otherwise.
    pub fn failing_for(archives: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            calls: Mutex::default(),
            fail_for: archives.into_iter().collect(),
        }
    }

    /// A snapshot of the recorded `(archive, into)` calls.
    pub fn calls(&self) -> Vec<(PathBuf, PathBuf)> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl Extractor for FakeExtractor {
    async fn extract(&self, archive: &Path, into: &Path) -> Result<(), Failure> {
        self.calls
            .lock()
            .unwrap()
            .push((archive.to_path_buf(), into.to_path_buf()));
        if self.fail_for.contains(archive) {
            Err(Failure::fatal(format!("bad zip: {}", archive.display())))
        } else {
            Ok(())
        }
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
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(one_chunk(b"data".to_vec()))
    }
}

/// A `PackageSource` that fails the first `remaining_failures` fetches with a
/// `Transient` error, then serves `bytes`. Records total fetch attempts so
/// tests can assert the retry bound.
pub struct FlakyFetchSource {
    pub assets: Vec<Asset>,
    pub bytes: Vec<u8>,
    pub remaining_failures: AtomicUsize,
    pub fetch_calls: AtomicUsize,
}

impl FlakyFetchSource {
    pub fn new(assets: Vec<Asset>, bytes: Vec<u8>, initial_failures: usize) -> Self {
        Self {
            assets,
            bytes,
            remaining_failures: AtomicUsize::new(initial_failures),
            fetch_calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl PackageSource for FlakyFetchSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        self.fetch_calls.fetch_add(1, Ordering::SeqCst);
        let should_fail = self
            .remaining_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                if n > 0 {
                    Some(n - 1)
                } else {
                    None
                }
            })
            .is_ok();
        if should_fail {
            Err(Failure::transient("flaky transient failure"))
        } else {
            Ok(one_chunk(self.bytes.clone()))
        }
    }
}

/// A `PackageSource` whose enumerate (`list_assets`) always fails with a given
/// message, for testing that the reason is surfaced.
pub struct EnumerateFailSource {
    pub message: String,
}

impl EnumerateFailSource {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait]
impl PackageSource for EnumerateFailSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Err(Failure::fatal(self.message.clone()))
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        unreachable!("enumerate fails before any fetch")
    }
}

/// A `PackageSource` that streams one Asset's bytes in fixed-size chunks, so
/// tests can assert byte-level progress reporting.
pub struct ChunkedByteSource {
    pub assets: Vec<Asset>,
    pub bytes: Vec<u8>,
    pub chunk_size: usize,
}

#[async_trait]
impl PackageSource for ChunkedByteSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        Ok(chunked_stream(self.bytes.clone(), self.chunk_size))
    }
}

/// A `ProgressReporter` that records the events it receives.
#[derive(Default)]
pub struct RecordingReporter {
    pub started: Mutex<Option<(usize, u64)>>,
    pub advanced: Mutex<HashMap<usize, u64>>,
    pub finished: Mutex<Vec<(usize, String)>>,
}

impl acd::ports::ProgressReporter for RecordingReporter {
    fn start(&self, total_files: usize, total_bytes: u64) {
        *self.started.lock().unwrap() = Some((total_files, total_bytes));
    }

    fn asset_started(&self, _index: usize, _name: &str, _size: u64) {}

    fn asset_advanced(&self, index: usize, bytes: u64) {
        *self.advanced.lock().unwrap().entry(index).or_default() += bytes;
    }

    fn asset_finished(&self, index: usize, _name: &str, outcome: &acd::domain::AssetOutcome) {
        let label = match outcome {
            acd::domain::AssetOutcome::Downloaded(_) => "downloaded",
            acd::domain::AssetOutcome::Cached => "cached",
            acd::domain::AssetOutcome::Failed(_) => "failed",
        };
        self.finished
            .lock()
            .unwrap()
            .push((index, label.to_string()));
    }

    fn finish(&self, _summary: &acd::domain::RunSummary) {}
}

/// A fake `Authenticator`: reports a scripted status and records login calls,
/// optionally failing the login.
pub struct FakeAuthenticator {
    pub status: acd::ports::SessionStatus,
    pub login_fails: bool,
    pub login_calls: AtomicUsize,
}

impl FakeAuthenticator {
    pub fn new(status: acd::ports::SessionStatus, login_fails: bool) -> Self {
        Self {
            status,
            login_fails,
            login_calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl acd::ports::Authenticator for FakeAuthenticator {
    async fn session_status(&self, _profile: Option<&str>) -> acd::ports::SessionStatus {
        self.status
    }

    async fn login(&self, _profile: Option<&str>) -> Result<(), Failure> {
        self.login_calls.fetch_add(1, Ordering::SeqCst);
        if self.login_fails {
            Err(Failure::fatal("aws sso login failed"))
        } else {
            Ok(())
        }
    }
}

/// A `PackageSource` that fails every fetch with `AuthExpired` until the shared
/// `logins` counter reaches `succeed_after_logins`, then serves `bytes`. Used
/// with `SharedLoginAuthenticator` to drive mid-run re-login behavior.
pub struct AuthExpiringSource {
    pub assets: Vec<Asset>,
    pub bytes: Vec<u8>,
    pub logins: Arc<AtomicUsize>,
    pub succeed_after_logins: usize,
    pub fetch_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl PackageSource for AuthExpiringSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        self.fetch_calls.fetch_add(1, Ordering::SeqCst);
        if self.logins.load(Ordering::SeqCst) >= self.succeed_after_logins {
            Ok(one_chunk(self.bytes.clone()))
        } else {
            Err(Failure::auth_expired("the SSO token has expired"))
        }
    }
}

/// An `Authenticator` that counts logins into a shared counter (observed by
/// `AuthExpiringSource`) and can be made to fail.
pub struct SharedLoginAuthenticator {
    pub logins: Arc<AtomicUsize>,
    pub login_fails: bool,
}

#[async_trait]
impl Authenticator for SharedLoginAuthenticator {
    async fn session_status(&self, _profile: Option<&str>) -> SessionStatus {
        SessionStatus::Valid
    }

    async fn login(&self, _profile: Option<&str>) -> Result<(), Failure> {
        self.logins.fetch_add(1, Ordering::SeqCst);
        if self.login_fails {
            Err(Failure::fatal("aws sso login failed"))
        } else {
            Ok(())
        }
    }
}

/// A step in a `ScriptedSource` fetch sequence.
#[derive(Clone)]
pub enum FetchStep {
    AuthExpired,
    Transient,
    Ok(Vec<u8>),
}

/// A `PackageSource` that returns a predetermined sequence of fetch outcomes
/// (for sequential single-Asset scenarios).
pub struct ScriptedSource {
    pub assets: Vec<Asset>,
    pub steps: Mutex<std::collections::VecDeque<FetchStep>>,
}

#[async_trait]
impl PackageSource for ScriptedSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        let step = self.steps.lock().unwrap().pop_front();
        match step {
            Some(FetchStep::AuthExpired) => Err(Failure::auth_expired("token expired")),
            Some(FetchStep::Transient) => Err(Failure::transient("blip")),
            Some(FetchStep::Ok(bytes)) => Ok(one_chunk(bytes)),
            None => Err(Failure::fatal("script exhausted")),
        }
    }
}

/// A `PackageSource` that measures how many `list_assets` calls run at once, so
/// tests can assert the Enumerate Phase parallelizes (bounded).
pub struct ListConcurrencyProbeSource {
    pub assets: Vec<Asset>,
    pub in_flight: AtomicUsize,
    pub max_in_flight: AtomicUsize,
}

impl ListConcurrencyProbeSource {
    pub fn new(assets: Vec<Asset>) -> Self {
        Self {
            assets,
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl PackageSource for ListConcurrencyProbeSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        unreachable!("enumerate-only probe")
    }
}

/// A `PackageSource` that counts `list_assets` calls (for cache hit/miss tests).
pub struct ListCountingSource {
    pub assets: Vec<Asset>,
    pub list_calls: AtomicUsize,
}

impl ListCountingSource {
    pub fn new(assets: Vec<Asset>) -> Self {
        Self {
            assets,
            list_calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl PackageSource for ListCountingSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        self.list_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, _asset: &Asset) -> Result<AssetStream, Failure> {
        unreachable!("list-counting source is enumerate-only")
    }
}

/// An in-memory `AssetListCache`.
#[derive(Default)]
pub struct InMemoryAssetListCache {
    map: Mutex<HashMap<AssetListKey, Vec<Asset>>>,
}

#[async_trait]
impl AssetListCache for InMemoryAssetListCache {
    async fn get(&self, key: &AssetListKey) -> Option<Vec<Asset>> {
        self.map.lock().unwrap().get(key).cloned()
    }

    async fn put(&self, key: &AssetListKey, assets: &[Asset]) {
        self.map
            .lock()
            .unwrap()
            .insert(key.clone(), assets.to_vec());
    }
}

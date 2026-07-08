mod fakes;

use acd::adapters::cache::is_cacheable;

#[test]
fn only_non_snapshot_versions_are_cacheable() {
    assert!(is_cacheable("1.4.2"), "a release version is cacheable");
    assert!(is_cacheable("0.4.0"), "a release version is cacheable");
    assert!(
        !is_cacheable("0.42.0.201-snapshot"),
        "a snapshot version is not cacheable"
    );
    assert!(
        !is_cacheable("1.0.0-SNAPSHOT"),
        "snapshot detection is case-insensitive"
    );
}

use std::sync::atomic::Ordering;
use std::sync::Arc;

use acd::adapters::cache::CachingPackageSource;
use acd::domain::{Asset, ConnectionSettings, Entry};
use acd::ports::PackageSource;

fn conn() -> ConnectionSettings {
    ConnectionSettings {
        domain: "d".into(),
        domain_owner: "111122223333".into(),
        repository: "r".into(),
        region: None,
    }
}

fn entry(version: &str) -> Entry {
    Entry {
        namespace: Some("ns".into()),
        package: "pkg".into(),
        version: version.into(),
        dest: std::path::PathBuf::from("out"),
    }
}

fn two_assets() -> Vec<Asset> {
    vec![Asset {
        name: "a.bin".into(),
        size: 5,
        expected_md5: "5d41402abc4b2a76b9719d911017c592".into(),
    }]
}

#[tokio::test]
async fn cacheable_version_lists_once_then_serves_from_cache() {
    let source = Arc::new(fakes::ListCountingSource::new(two_assets()));
    let cache = Arc::new(fakes::InMemoryAssetListCache::default());
    let decorator = CachingPackageSource::new(source.clone(), cache, conn());
    let entry = entry("1.0.0");

    let first = decorator.list_assets(&entry).await.unwrap();
    let second = decorator.list_assets(&entry).await.unwrap();

    assert_eq!(first, second, "cache returns the same assets");
    assert_eq!(
        source.list_calls.load(Ordering::SeqCst),
        1,
        "second listing should be served from cache, not the inner source"
    );
}

#[tokio::test]
async fn snapshot_version_is_never_cached() {
    let source = Arc::new(fakes::ListCountingSource::new(two_assets()));
    let cache = Arc::new(fakes::InMemoryAssetListCache::default());
    let decorator = CachingPackageSource::new(source.clone(), cache, conn());
    let entry = entry("0.42.0.201-snapshot");

    let _ = decorator.list_assets(&entry).await.unwrap();
    let _ = decorator.list_assets(&entry).await.unwrap();

    assert_eq!(
        source.list_calls.load(Ordering::SeqCst),
        2,
        "a snapshot version must hit the inner source every time (never cached)"
    );
}

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
use acd::ports::{AssetListCache, PackageSource};

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

#[tokio::test]
async fn refresh_bypasses_cache_and_repopulates() {
    // Cache holds a STALE list; the inner source has the FRESH list.
    let stale = vec![Asset {
        name: "stale.bin".into(),
        size: 1,
        expected_md5: "00000000000000000000000000000000".into(),
    }];
    let fresh = two_assets();

    let cache = Arc::new(fakes::InMemoryAssetListCache::default());
    let source = Arc::new(fakes::ListCountingSource::new(fresh.clone()));
    let decorator =
        CachingPackageSource::new(source.clone(), cache.clone(), conn()).with_refresh(true);
    let entry = entry("1.0.0");

    // Pre-seed the cache with the stale value.
    cache
        .put(
            &acd::ports::AssetListKey {
                domain: "d".into(),
                domain_owner: "111122223333".into(),
                repository: "r".into(),
                namespace: Some("ns".into()),
                package: "pkg".into(),
                version: "1.0.0".into(),
            },
            &stale,
        )
        .await;

    let result = decorator.list_assets(&entry).await.unwrap();

    assert_eq!(
        result, fresh,
        "refresh returns the fresh list, not the cached stale one"
    );
    assert_eq!(
        source.list_calls.load(Ordering::SeqCst),
        1,
        "refresh always queries the inner source"
    );
}

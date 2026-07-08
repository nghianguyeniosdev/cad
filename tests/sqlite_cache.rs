use acd::adapters::cache::SqliteAssetListCache;
use acd::domain::Asset;
use acd::ports::{AssetListCache, AssetListKey};

fn key(version: &str) -> AssetListKey {
    AssetListKey {
        domain: "d".into(),
        domain_owner: "111122223333".into(),
        repository: "r".into(),
        namespace: Some("ns".into()),
        package: "pkg".into(),
        version: version.into(),
    }
}

fn assets() -> Vec<Asset> {
    vec![
        Asset {
            name: "a.bin".into(),
            size: 5,
            expected_md5: "5d41402abc4b2a76b9719d911017c592".into(),
        },
        Asset {
            name: "b.zip".into(),
            size: 1024,
            expected_md5: "7d793037a0760186574b0282f2f435e7".into(),
        },
    ]
}

#[tokio::test]
async fn stores_and_returns_assets_for_a_key() {
    let dir = tempfile::tempdir().unwrap();
    let cache = SqliteAssetListCache::open(&dir.path().join("cache.db"));

    assert_eq!(cache.get(&key("1.0.0")).await, None, "absent key -> None");

    cache.put(&key("1.0.0"), &assets()).await;

    assert_eq!(
        cache.get(&key("1.0.0")).await,
        Some(assets()),
        "a stored key returns exactly the assets that were put"
    );
    assert_eq!(
        cache.get(&key("2.0.0")).await,
        None,
        "a different version is a distinct key"
    );
}

#[tokio::test]
async fn persists_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.db");

    // Write with one instance, drop it, reopen a fresh instance on the same file.
    {
        let cache = SqliteAssetListCache::open(&path);
        cache.put(&key("1.0.0"), &assets()).await;
    }
    let reopened = SqliteAssetListCache::open(&path);

    assert_eq!(
        reopened.get(&key("1.0.0")).await,
        Some(assets()),
        "the cache persists on disk across instances (cross-run)"
    );
}

#[tokio::test]
async fn unopenable_db_degrades_to_noop() {
    // Parent is a regular file, so the DB dir can't be created.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("not-a-dir");
    std::fs::write(&file, b"x").unwrap();
    let cache = SqliteAssetListCache::open(&file.join("nested/cache.db"));

    // Must not panic or error.
    cache.put(&key("1.0.0"), &assets()).await;
    assert_eq!(
        cache.get(&key("1.0.0")).await,
        None,
        "a cache that couldn't open is a silent no-op"
    );
}

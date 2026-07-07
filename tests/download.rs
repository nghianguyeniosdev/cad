mod fakes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use acd::app::{DownloadService, Planner};
use acd::domain::{Asset, ConnectionSettings, Entry, Manifest};

use fakes::{FakeFileStore, FakePackageSource};

fn two_asset_list() -> Vec<Asset> {
    vec![
        Asset {
            name: "a.bin".into(),
            size: 5,
            expected_md5: "5d41402abc4b2a76b9719d911017c592".into(),
        },
        Asset {
            name: "b.bin".into(),
            size: 5,
            expected_md5: "7d793037a0760186574b0282f2f435e7".into(),
        },
    ]
}

fn two_entry_manifest() -> Manifest {
    Manifest {
        connection: ConnectionSettings {
            domain: "my-domain".into(),
            domain_owner: "111122223333".into(),
            repository: "my-repo".into(),
            region: None,
        },
        packages: vec![
            Entry {
                namespace: Some("mobile".into()),
                package: "core-rgp".into(),
                version: "1.4.2".into(),
                dest: PathBuf::from("out/core-rgp"),
            },
            Entry {
                namespace: None,
                package: "utils".into(),
                version: "2.0.0".into(),
                dest: PathBuf::from("out/utils"),
            },
        ],
    }
}

fn single_entry_manifest() -> Manifest {
    Manifest {
        connection: ConnectionSettings {
            domain: "my-domain".into(),
            domain_owner: "111122223333".into(),
            repository: "my-repo".into(),
            region: Some("ap-southeast-1".into()),
        },
        packages: vec![Entry {
            namespace: Some("mobile".into()),
            package: "core-rgp".into(),
            version: "1.4.2".into(),
            dest: PathBuf::from("out/core-rgp"),
        }],
    }
}

#[tokio::test]
async fn downloads_all_assets_of_a_single_entry() {
    // Expected MD5s are well-known published vectors (independent source of
    // truth), not recomputed via the same hasher the code uses:
    //   md5("hello") = 5d41402abc4b2a76b9719d911017c592
    //   md5("world") = 7d793037a0760186574b0282f2f435e7
    let assets = vec![
        Asset {
            name: "a.bin".into(),
            size: 5,
            expected_md5: "5d41402abc4b2a76b9719d911017c592".into(),
        },
        Asset {
            name: "b.bin".into(),
            size: 5,
            expected_md5: "7d793037a0760186574b0282f2f435e7".into(),
        },
    ];
    let bytes = HashMap::from([
        ("a.bin".to_string(), b"hello".to_vec()),
        ("b.bin".to_string(), b"world".to_vec()),
    ]);

    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(FakeFileStore::default());
    let service = DownloadService::new(source, files.clone());

    let summary = service.run(&single_entry_manifest()).await;

    assert_eq!(summary.downloaded, 2, "both Assets should download");
    assert_eq!(summary.failed, 0, "no Assets should fail");
    assert_eq!(summary.exit_code(), 0);

    let written = files.written.lock().unwrap();
    assert_eq!(
        written
            .get(&PathBuf::from("out/core-rgp/a.bin"))
            .map(Vec::as_slice),
        Some(&b"hello"[..]),
        "a.bin should land under the Entry's dest with its verified bytes"
    );
    assert_eq!(
        written
            .get(&PathBuf::from("out/core-rgp/b.bin"))
            .map(Vec::as_slice),
        Some(&b"world"[..]),
        "b.bin should land under the Entry's dest with its verified bytes"
    );
}

#[tokio::test]
async fn asset_failing_md5_is_not_committed_and_run_fails() {
    // Expected md5("hello"), but the bytes served are "tampered" — a mismatch.
    let assets = vec![Asset {
        name: "corrupt.bin".into(),
        size: 8,
        expected_md5: "5d41402abc4b2a76b9719d911017c592".into(),
    }];
    let bytes = HashMap::from([("corrupt.bin".to_string(), b"tampered".to_vec())]);

    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(FakeFileStore::default());
    let service = DownloadService::new(source, files.clone());

    let summary = service.run(&single_entry_manifest()).await;

    assert_eq!(
        summary.downloaded, 0,
        "a mismatched Asset must not count as downloaded"
    );
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.exit_code(), 1, "any Asset failure exits non-zero");
    assert!(
        summary.failed_assets.iter().any(|n| n == "corrupt.bin"),
        "the failed Asset should be named in the summary"
    );
    assert!(
        files.written.lock().unwrap().is_empty(),
        "a corrupt Asset must never be committed"
    );
}

#[tokio::test]
async fn planner_aggregates_totals_across_entries() {
    // Fake returns the same 2 assets for each Entry; 2 entries -> 4 assets.
    let source = Arc::new(FakePackageSource {
        assets: two_asset_list(),
        bytes: HashMap::new(),
    });
    let planner = Planner::new(source);

    let plan = planner
        .plan(&two_entry_manifest())
        .await
        .expect("enumerate should succeed");

    assert_eq!(plan.total_files(), 4, "2 entries x 2 assets");
    assert_eq!(plan.total_bytes(), 20, "4 assets x 5 bytes");
    assert_eq!(plan.package_count(), 2, "two distinct Package Versions");
}

#[tokio::test]
async fn downloads_assets_across_multiple_entries() {
    let bytes = HashMap::from([
        ("a.bin".to_string(), b"hello".to_vec()),
        ("b.bin".to_string(), b"world".to_vec()),
    ]);
    let source = Arc::new(FakePackageSource {
        assets: two_asset_list(),
        bytes,
    });
    let files = Arc::new(FakeFileStore::default());
    let service = DownloadService::new(source, files.clone());

    let summary = service.run(&two_entry_manifest()).await;

    assert_eq!(summary.downloaded, 4, "2 entries x 2 assets all download");
    assert_eq!(summary.failed, 0);

    let written = files.written.lock().unwrap();
    for path in [
        "out/core-rgp/a.bin",
        "out/core-rgp/b.bin",
        "out/utils/a.bin",
        "out/utils/b.bin",
    ] {
        assert!(
            written.contains_key(&PathBuf::from(path)),
            "{path} should be committed"
        );
    }
}

#[tokio::test]
async fn download_respects_the_concurrency_limit() {
    use std::sync::atomic::Ordering;

    // 12 assets, concurrency 3: never more than 3 fetches in flight at once,
    // but genuinely concurrent (more than 1).
    let assets: Vec<Asset> = (0..12)
        .map(|i| Asset {
            name: format!("f{i}.bin"),
            size: 4,
            expected_md5: String::new(),
        })
        .collect();

    let source = Arc::new(fakes::ConcurrencyProbeSource::new(assets));
    let files = Arc::new(FakeFileStore::default());
    let service = DownloadService::new(source.clone(), files).with_concurrency(3);

    let _ = service.run(&single_entry_manifest()).await;

    let max = source.max_in_flight.load(Ordering::SeqCst);
    assert!(
        max <= 3,
        "must never exceed the configured concurrency; saw {max}"
    );
    assert!(
        max >= 2,
        "should actually run Assets concurrently; saw {max}"
    );
}

#[tokio::test]
async fn present_asset_with_matching_md5_is_skipped_as_cached() {
    // The Asset's expected MD5 is md5("hello"); the file already on disk reports
    // that same MD5, so it must be skipped WITHOUT fetching. The source serves
    // WRONG bytes, so any fetch would fail — cached==1 & failed==0 proves skip.
    let assets = vec![Asset {
        name: "a.bin".into(),
        size: 5,
        expected_md5: "5d41402abc4b2a76b9719d911017c592".into(),
    }];
    let bytes = HashMap::from([("a.bin".to_string(), b"WRONG-BYTES".to_vec())]);

    let mut files = FakeFileStore::default();
    files.existing.insert(
        PathBuf::from("out/core-rgp/a.bin"),
        "5d41402abc4b2a76b9719d911017c592".into(),
    );

    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(files);
    let service = DownloadService::new(source, files.clone());

    let summary = service.run(&single_entry_manifest()).await;

    assert_eq!(summary.cached, 1, "present matching Asset should be cached");
    assert_eq!(summary.downloaded, 0, "cached Asset is not re-downloaded");
    assert_eq!(summary.failed, 0, "skip means no fetch, so no failure");
    assert!(
        files.written.lock().unwrap().is_empty(),
        "a cached Asset must not be re-written"
    );
}

fn one_asset(name: &str, md5: &str) -> Vec<Asset> {
    vec![Asset {
        name: name.into(),
        size: 5,
        expected_md5: md5.into(),
    }]
}

fn zero_backoff() -> acd::app::RetryPolicy {
    acd::app::RetryPolicy {
        max_retries: 3,
        backoff: std::time::Duration::ZERO,
    }
}

#[tokio::test]
async fn transient_failures_are_retried_until_success() {
    // Fails transiently twice, then serves md5("hello"). With retries, the 3rd
    // fetch succeeds.
    let source = Arc::new(fakes::FlakyFetchSource::new(
        one_asset("a.bin", "5d41402abc4b2a76b9719d911017c592"),
        b"hello".to_vec(),
        2,
    ));
    let files = Arc::new(FakeFileStore::default());
    let service = DownloadService::new(source.clone(), files).with_retry_policy(zero_backoff());

    let summary = service.run(&single_entry_manifest()).await;

    assert_eq!(summary.downloaded, 1, "should succeed after retries");
    assert_eq!(summary.failed, 0);
    assert_eq!(
        source.fetch_calls.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "2 failures + 1 success"
    );
}

#[tokio::test]
async fn transient_failures_exhaust_retries_then_fail() {
    // Always fails transiently. With max_retries=3, that's 1 initial + 3 retries
    // = 4 fetch attempts, then the Asset is marked failed and the run continues.
    let source = Arc::new(fakes::FlakyFetchSource::new(
        one_asset("a.bin", "5d41402abc4b2a76b9719d911017c592"),
        b"hello".to_vec(),
        1000,
    ));
    let files = Arc::new(FakeFileStore::default());
    let service = DownloadService::new(source.clone(), files).with_retry_policy(zero_backoff());

    let summary = service.run(&single_entry_manifest()).await;

    assert_eq!(summary.downloaded, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.exit_code(), 1);
    assert_eq!(
        source.fetch_calls.load(std::sync::atomic::Ordering::SeqCst),
        4,
        "1 initial attempt + 3 retries"
    );
}

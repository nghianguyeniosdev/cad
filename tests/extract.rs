mod fakes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use acd::app::extract::{single_zip, NoSingleZip};
use acd::app::DownloadService;
use acd::domain::{Asset, ConnectionSettings, Entry, Layout, Manifest};

use fakes::{FakeExtractor, FakeFileStore, FakePackageSource};

fn asset(name: &str) -> Asset {
    Asset {
        name: name.into(),
        size: 1,
        expected_md5: "00000000000000000000000000000000".into(),
    }
}

const HELLO_MD5: &str = "5d41402abc4b2a76b9719d911017c592";
const WORLD_MD5: &str = "7d793037a0760186574b0282f2f435e7";

/// One `.zip` asset (real "hello" MD5) plus a non-zip companion, served by the
/// fake source for every Entry.
fn zip_and_companion() -> (Vec<Asset>, HashMap<String, Vec<u8>>) {
    let assets = vec![
        Asset {
            name: "artifact.zip".into(),
            size: 5,
            expected_md5: HELLO_MD5.into(),
        },
        Asset {
            name: "meta.txt".into(),
            size: 5,
            expected_md5: WORLD_MD5.into(),
        },
    ];
    let bytes = HashMap::from([
        ("artifact.zip".to_string(), b"hello".to_vec()),
        ("meta.txt".to_string(), b"world".to_vec()),
    ]);
    (assets, bytes)
}

/// A versioned manifest whose Entry dests are the Cache Root version folders
/// (`cache/<package>/<version>`), exactly as `resolve` would derive them.
fn versioned_manifest(packages: &[(&str, &str)]) -> Manifest {
    Manifest {
        layout: Layout::Versioned,
        connection: ConnectionSettings {
            domain: "d".into(),
            domain_owner: "111122223333".into(),
            repository: "r".into(),
            region: None,
        },
        packages: packages
            .iter()
            .map(|(package, version)| Entry {
                namespace: None,
                package: (*package).into(),
                version: (*version).into(),
                dest: PathBuf::from("cache").join(package).join(version),
            })
            .collect(),
    }
}

#[test]
fn picks_the_one_zip_among_mixed_assets() {
    let assets = vec![asset("README.md"), asset("core-rgp.zip"), asset("LICENSE")];

    let chosen = single_zip(&assets).expect("exactly one .zip should be pickable");

    assert_eq!(chosen.name, "core-rgp.zip");
}

#[test]
fn no_zip_is_an_error() {
    let assets = vec![asset("README.md"), asset("LICENSE")];

    assert_eq!(single_zip(&assets), Err(NoSingleZip::None));
}

#[test]
fn more_than_one_zip_is_an_error() {
    let assets = vec![asset("a.zip"), asset("b.zip")];

    assert_eq!(
        single_zip(&assets),
        Err(NoSingleZip::Multiple(vec!["a.zip".into(), "b.zip".into()]))
    );
}

fn flat_manifest(dests: &[(&str, &str)]) -> Manifest {
    Manifest {
        layout: Layout::Flat,
        connection: ConnectionSettings {
            domain: "d".into(),
            domain_owner: "111122223333".into(),
            repository: "r".into(),
            region: None,
        },
        packages: dests
            .iter()
            .map(|(package, dest)| Entry {
                namespace: None,
                package: (*package).into(),
                version: "1.0.0".into(),
                dest: PathBuf::from(dest),
            })
            .collect(),
    }
}

#[tokio::test]
async fn flat_layout_never_extracts() {
    let (assets, bytes) = zip_and_companion();
    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(FakeFileStore::default());
    let extractor = Arc::new(FakeExtractor::default());

    let manifest = flat_manifest(&[("core-rgp", "out/core-rgp")]);
    let service = DownloadService::new(source, files).with_extractor(extractor.clone());

    let summary = service.run(&manifest).await;

    assert_eq!(summary.extracted, 0);
    assert!(
        extractor.calls().is_empty(),
        "flat layout must not invoke the extractor, got: {:?}",
        extractor.calls()
    );
}

#[tokio::test]
async fn an_entry_with_no_zip_surfaces_a_per_package_failure() {
    // The source serves only a non-zip asset for the entry.
    let assets = vec![Asset {
        name: "meta.txt".into(),
        size: 5,
        expected_md5: WORLD_MD5.into(),
    }];
    let bytes = HashMap::from([("meta.txt".to_string(), b"world".to_vec())]);
    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(FakeFileStore::default());
    let extractor = Arc::new(FakeExtractor::default());

    let manifest = versioned_manifest(&[("core-rgp", "1.4.2")]);
    let service = DownloadService::new(source, files).with_extractor(extractor.clone());

    let summary = service.run(&manifest).await;

    assert_eq!(summary.extracted, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.failed_assets[0].name, "core-rgp");
    assert!(summary.failed_assets[0].reason.contains("no .zip"));
    assert!(
        extractor.calls().is_empty(),
        "nothing to unzip, so the extractor is never called"
    );
}

#[tokio::test]
async fn a_failing_extract_is_collected_and_the_phase_continues() {
    let (assets, bytes) = zip_and_companion();
    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(FakeFileStore::default());
    // Fail only core-rgp's archive; utils must still extract.
    let extractor = Arc::new(FakeExtractor::failing_for([PathBuf::from(
        "cache/core-rgp/1.4.2/artifact.zip",
    )]));

    let manifest = versioned_manifest(&[("core-rgp", "1.4.2"), ("utils", "2.0.0")]);
    let service = DownloadService::new(source, files).with_extractor(extractor.clone());

    let summary = service.run(&manifest).await;

    assert_eq!(summary.extracted, 1, "utils should still extract");
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.exit_code(), 1);
    let failed = &summary.failed_assets;
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].name, "core-rgp");
    assert!(
        failed[0].reason.contains("bad zip"),
        "reason should carry the failure cause, got: {:?}",
        failed[0].reason
    );
    // Both entries were attempted (the phase did not abort on the first failure).
    assert_eq!(extractor.calls().len(), 2);
}

#[tokio::test]
async fn versioned_run_extracts_each_entry_into_podlocals() {
    let (assets, bytes) = zip_and_companion();
    let source = Arc::new(FakePackageSource { assets, bytes });
    let files = Arc::new(FakeFileStore::default());
    let extractor = Arc::new(FakeExtractor::default());

    let manifest = versioned_manifest(&[("core-rgp", "1.4.2"), ("utils", "2.0.0")]);
    let service = DownloadService::new(source, files).with_extractor(extractor.clone());

    let summary = service.run(&manifest).await;

    // 4 assets downloaded (2 per entry), 2 zips extracted, none failed.
    assert_eq!(summary.downloaded, 4);
    assert_eq!(summary.extracted, 2);
    assert_eq!(summary.failed, 0);

    // The Extract Phase unzipped each Entry's archive from its Cache Root version
    // folder into ./PodLocals/<package>/Current.
    let mut calls = extractor.calls();
    calls.sort();
    assert_eq!(
        calls,
        vec![
            (
                PathBuf::from("cache/core-rgp/1.4.2/artifact.zip"),
                PathBuf::from("PodLocals/core-rgp/Current"),
            ),
            (
                PathBuf::from("cache/utils/2.0.0/artifact.zip"),
                PathBuf::from("PodLocals/utils/Current"),
            ),
        ]
    );
}

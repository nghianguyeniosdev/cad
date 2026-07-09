use std::path::Path;

use acd::adapters::fs::{LocalExtractor, LocalFileStore, LocalMarkerStore};
use acd::ports::{Extractor, FileStore, MarkerStore};

/// Build a real `.zip` at `zip_path` containing one file `name` with `contents`,
/// using the system `zip`. Skips the test if `zip` is unavailable.
fn make_zip(zip_path: &Path, name: &str, contents: &[u8]) {
    let dir = zip_path.parent().unwrap();
    std::fs::write(dir.join(name), contents).unwrap();
    let status = std::process::Command::new("zip")
        .arg("-q")
        .arg(zip_path)
        .arg(name)
        .current_dir(dir)
        .status()
        .expect("`zip` should be available on the test host");
    assert!(status.success(), "zip failed to build the fixture");
    std::fs::remove_file(dir.join(name)).unwrap();
}

#[tokio::test]
async fn writes_bytes_creating_parent_dirs_and_leaves_no_part_file() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("nested/sub/asset.bin");

    LocalFileStore
        .write(&dest, b"payload")
        .await
        .expect("write should succeed");

    assert_eq!(
        tokio::fs::read(&dest).await.unwrap(),
        b"payload",
        "the final file should contain the written bytes"
    );

    let part = {
        let mut p = dest.clone().into_os_string();
        p.push(".part");
        std::path::PathBuf::from(p)
    };
    assert!(
        !Path::new(&part).exists(),
        "no leftover .part temp file should remain after a successful write"
    );
}

#[tokio::test]
async fn wipes_the_destination_then_unzips_the_archive() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("artifact.zip");
    make_zip(&archive, "payload.txt", b"fresh");

    // A pre-existing Current with a stale file that must not survive.
    let into = root.path().join("Current");
    std::fs::create_dir_all(&into).unwrap();
    std::fs::write(into.join("stale.txt"), b"old").unwrap();

    LocalExtractor
        .extract(&archive, &into)
        .await
        .expect("extraction should succeed");

    assert_eq!(
        std::fs::read(into.join("payload.txt")).unwrap(),
        b"fresh",
        "the archive contents should be present in Current"
    );
    assert!(
        !into.join("stale.txt").exists(),
        "the stale file should be gone after a wipe-then-unzip"
    );
}

#[tokio::test]
async fn a_corrupt_archive_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("artifact.zip");
    std::fs::write(&archive, b"this is not a zip file").unwrap();
    let into = root.path().join("Current");

    let result = LocalExtractor.extract(&archive, &into).await;

    assert!(result.is_err(), "a corrupt archive should fail extraction");
}

/// Create `package_dir/Current` with one file.
fn make_current(package_dir: &Path, name: &str, contents: &[u8]) {
    let current = package_dir.join("Current");
    std::fs::create_dir_all(&current).unwrap();
    std::fs::write(current.join(name), contents).unwrap();
}

#[tokio::test]
async fn a_fresh_package_dir_is_not_current() {
    let root = tempfile::tempdir().unwrap();
    let package_dir = root.path().join("core-rgp");
    std::fs::create_dir_all(&package_dir).unwrap();

    assert!(
        !LocalMarkerStore.is_current(&package_dir, "1.0.0").await,
        "with no marker and no Current, extraction is required"
    );
}

#[tokio::test]
async fn after_record_the_marker_is_current_and_sits_beside_current() {
    let root = tempfile::tempdir().unwrap();
    let package_dir = root.path().join("core-rgp");
    make_current(&package_dir, "payload.txt", b"hi");

    LocalMarkerStore
        .record(&package_dir, "1.0.0")
        .await
        .expect("record should succeed");

    assert!(LocalMarkerStore.is_current(&package_dir, "1.0.0").await);
    // The marker is a sibling of Current, never inside it.
    assert!(package_dir.join(".acd-version").exists());
    assert!(!package_dir.join("Current/.acd-version").exists());
}

#[tokio::test]
async fn deleting_a_file_from_current_makes_it_not_current() {
    let root = tempfile::tempdir().unwrap();
    let package_dir = root.path().join("core-rgp");
    make_current(&package_dir, "payload.txt", b"hi");
    std::fs::write(package_dir.join("Current/extra.txt"), b"more").unwrap();
    LocalMarkerStore
        .record(&package_dir, "1.0.0")
        .await
        .unwrap();

    std::fs::remove_file(package_dir.join("Current/extra.txt")).unwrap();

    assert!(
        !LocalMarkerStore.is_current(&package_dir, "1.0.0").await,
        "a deleted file must invalidate the marker"
    );
}

#[tokio::test]
async fn a_version_bump_is_not_current() {
    let root = tempfile::tempdir().unwrap();
    let package_dir = root.path().join("core-rgp");
    make_current(&package_dir, "payload.txt", b"hi");
    LocalMarkerStore
        .record(&package_dir, "1.0.0")
        .await
        .unwrap();

    assert!(
        !LocalMarkerStore.is_current(&package_dir, "2.0.0").await,
        "a different pinned version must force re-extraction"
    );
}

use std::path::Path;

use acd::adapters::fs::LocalFileStore;
use acd::ports::FileStore;

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

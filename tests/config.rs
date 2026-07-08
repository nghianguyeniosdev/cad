use std::path::PathBuf;

use acd::config::resolve_cache_root;

#[test]
fn flag_wins_over_file_over_default_and_tilde_expands() {
    let home = dirs::home_dir().expect("home dir");
    let dir = tempfile::tempdir().unwrap();

    // A config file that sets a cache_root.
    let cfg = dir.path().join("config.yml");
    std::fs::write(&cfg, "cache_root: /from/file\n").unwrap();

    // 1. Flag wins over the file, and `~` expands.
    assert_eq!(
        resolve_cache_root(Some("~/flagged".into()), &cfg).unwrap(),
        home.join("flagged")
    );

    // 2. No flag -> the file's cache_root is used.
    assert_eq!(
        resolve_cache_root(None, &cfg).unwrap(),
        PathBuf::from("/from/file")
    );

    // 3. No flag + missing file -> the built-in default (tilde-expanded).
    let missing = dir.path().join("does-not-exist.yml");
    assert_eq!(
        resolve_cache_root(None, &missing).unwrap(),
        home.join("Library/Caches/CocoaPods/iOSArtifactPods")
    );
}

#[test]
fn malformed_config_is_a_hard_error() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.yml");
    // cache_root expects a string; a mapping is a type error.
    std::fs::write(&cfg, "cache_root:\n  unexpected: mapping\n").unwrap();

    assert!(
        resolve_cache_root(None, &cfg).is_err(),
        "a malformed config must error, not silently fall back to the default"
    );
}

use std::path::{Path, PathBuf};

use acd::domain::{ConnectionSettings, Entry, RawManifest};

fn resolve(
    yaml: &str,
    cache_root: &str,
) -> Result<acd::domain::Manifest, acd::domain::ManifestError> {
    RawManifest::from_yaml(yaml)?.resolve(Path::new(cache_root))
}

#[test]
fn parses_and_resolves_a_flat_manifest() {
    let yaml = r#"
domain: my-domain
domain_owner: "111122223333"
repository: my-repo
region: ap-southeast-1
packages:
  - namespace: mobile
    package: core-rgp
    version: "1.4.2"
    dest: ./artifacts/core-rgp
"#;

    let manifest = resolve(yaml, "/unused").expect("valid flat manifest should resolve");

    assert_eq!(
        manifest.connection,
        ConnectionSettings {
            domain: "my-domain".into(),
            domain_owner: "111122223333".into(),
            repository: "my-repo".into(),
            region: Some("ap-southeast-1".into()),
        }
    );
    assert_eq!(
        manifest.packages,
        vec![Entry {
            namespace: Some("mobile".into()),
            package: "core-rgp".into(),
            version: "1.4.2".into(),
            dest: PathBuf::from("./artifacts/core-rgp"),
        }]
    );
}

#[test]
fn region_and_namespace_are_optional() {
    let yaml = r#"
domain: d
domain_owner: "111122223333"
repository: r
packages:
  - package: p
    version: "1.0.0"
    dest: ./out
"#;
    let manifest = resolve(yaml, "/unused").expect("region-less manifest should resolve");
    assert_eq!(manifest.connection.region, None);
    assert_eq!(manifest.packages[0].namespace, None);
}

#[test]
fn rejects_a_manifest_missing_a_required_field() {
    // Missing `repository`.
    let yaml = r#"
domain: d
domain_owner: "111122223333"
packages:
  - package: p
    version: "1.0.0"
    dest: ./out
"#;
    assert!(resolve(yaml, "/unused").is_err());
}

#[test]
fn versioned_layout_derives_dest_from_cache_root_package_version() {
    let yaml = r#"
layout: versioned
domain: d
domain_owner: "111122223333"
repository: r
packages:
  - namespace: com.tymex
    package: UpFrontCheck
    version: "1.64.0"
  - package: Loyalty
    version: "1.7.0"
"#;

    let manifest = resolve(yaml, "/cache").expect("versioned manifest should resolve");

    // Derived path is <cache_root>/<package>/<version>, no namespace segment.
    assert_eq!(
        manifest.packages[0].dest,
        PathBuf::from("/cache/UpFrontCheck/1.64.0")
    );
    assert_eq!(
        manifest.packages[1].dest,
        PathBuf::from("/cache/Loyalty/1.7.0")
    );
}

#[test]
fn flat_layout_missing_dest_is_an_error() {
    let yaml = r#"
domain: d
domain_owner: "111122223333"
repository: r
packages:
  - package: p
    version: "1.0.0"
"#;
    assert!(
        resolve(yaml, "/unused").is_err(),
        "flat layout requires an explicit dest per entry"
    );
}

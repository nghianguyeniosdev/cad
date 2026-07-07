use std::path::PathBuf;

use acd::domain::{ConnectionSettings, Entry, Manifest};

#[test]
fn parses_a_valid_manifest() {
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

    let manifest = Manifest::from_yaml(yaml).expect("valid manifest should parse");

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
fn region_is_optional() {
    let yaml = r#"
domain: d
domain_owner: "111122223333"
repository: r
packages:
  - package: p
    version: "1.0.0"
    dest: ./out
"#;

    let manifest = Manifest::from_yaml(yaml).expect("region-less manifest should parse");

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

    assert!(
        Manifest::from_yaml(yaml).is_err(),
        "a manifest missing a required field should be rejected"
    );
}

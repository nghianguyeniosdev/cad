use acd::app::init::{init_manifest, render_template};
use acd::domain::Manifest;

#[test]
fn template_parses_as_a_manifest_with_an_example_entry() {
    let template = render_template();
    let manifest =
        Manifest::from_yaml(&template).expect("the init template must parse as a Manifest");

    assert!(
        !manifest.packages.is_empty(),
        "the template should include an example Entry"
    );
    assert!(
        !manifest.connection.domain.is_empty(),
        "the template should include a domain placeholder"
    );
    assert!(
        !manifest.connection.repository.is_empty(),
        "the template should include a repository placeholder"
    );
}

#[test]
fn init_writes_absent_refuses_without_force_and_overwrites_with_force() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codeartifact.yaml");

    // Absent -> writes.
    init_manifest(&path, false).expect("should write when absent");
    assert!(path.exists());

    // Tamper, then a non-force init must refuse and leave the file untouched.
    std::fs::write(&path, "SENTINEL").unwrap();
    assert!(
        init_manifest(&path, false).is_err(),
        "must refuse to overwrite without --force"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "SENTINEL",
        "a refused init must not modify the file"
    );

    // With force -> overwrites.
    init_manifest(&path, true).expect("force should overwrite");
    assert_ne!(
        std::fs::read_to_string(&path).unwrap(),
        "SENTINEL",
        "force must overwrite the file with the template"
    );
}

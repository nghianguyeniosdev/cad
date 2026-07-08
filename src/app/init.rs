use std::path::Path;

use crate::domain::Failure;

/// Render the starter `codeartifact.yaml` template. It parses as a valid
/// Manifest as-is (placeholder values) so `acd init` output is never broken;
/// the user replaces the placeholders with real coordinates.
pub fn render_template() -> String {
    r#"# acd Manifest — replace the placeholders with your real values.
#
#   aws sso login --profile <your-profile>
#   acd download --manifest codeartifact.yaml --profile <your-profile>

# ─── Connection Settings ─────────────────────────────────────────────────────
# domain        : the CodeArtifact domain      (aws codeartifact list-domains)
# domain_owner  : 12-digit AWS account id       (MUST be quoted)
# repository    : the repository in the domain  (aws codeartifact list-repositories)
# region        : optional; omit for the profile's default region
domain: my-domain
domain_owner: "111122223333"
repository: my-repo
region: ap-southeast-1

# ─── Packages: one Entry per pinned Package Version ──────────────────────────
# Each Entry downloads ALL assets of one generic Package Version into `dest`.
# Generic packages require a `namespace`. Quote `version` so "1.0.0" stays a string.
packages:
  - namespace: my-namespace
    package: my-package
    version: "1.0.0"
    dest: ./artifacts/my-package
"#
    .to_string()
}

/// Write the template to `path`. Refuses to overwrite an existing file unless
/// `force` is set.
pub fn init_manifest(path: &Path, force: bool) -> Result<(), Failure> {
    if path.exists() && !force {
        return Err(Failure::fatal(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    std::fs::write(path, render_template())
        .map_err(|e| Failure::fatal(format!("cannot write {}: {e}", path.display())))
}

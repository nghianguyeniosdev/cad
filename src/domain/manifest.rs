use std::path::PathBuf;

use serde::Deserialize;

/// The per-run CodeArtifact coordinates, declared at the top of the Manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectionSettings {
    pub domain: String,
    pub domain_owner: String,
    pub repository: String,
    /// Falls back to the profile's default region when absent.
    #[serde(default)]
    pub region: Option<String>,
}

/// One line in the Manifest's `packages` list: a single pinned Package Version
/// and the local destination folder its Assets land in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub namespace: Option<String>,
    pub package: String,
    /// Always a pinned, exact version (never "latest").
    pub version: String,
    pub dest: PathBuf,
}

/// The parsed `codeartifact.yaml`: Connection Settings plus the Entries to fetch.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    #[serde(flatten)]
    pub connection: ConnectionSettings,
    pub packages: Vec<Entry>,
}

/// Failure to parse a Manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid manifest: {0}")]
    Parse(#[from] serde_yaml::Error),
}

impl Manifest {
    /// Parse a Manifest from YAML text.
    pub fn from_yaml(yaml: &str) -> Result<Self, ManifestError> {
        Ok(serde_yaml::from_str(yaml)?)
    }
}

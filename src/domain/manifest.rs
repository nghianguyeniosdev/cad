use std::path::{Path, PathBuf};

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

/// How each Entry's destination is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Each Entry carries an explicit `dest` (default).
    #[default]
    Flat,
    /// Each Entry's dest is derived: `<cache_root>/<package>/<version>`.
    Versioned,
}

/// One resolved Entry: a pinned Package Version and the concrete local folder
/// its Assets land in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub namespace: Option<String>,
    pub package: String,
    /// Always a pinned, exact version (never "latest").
    pub version: String,
    pub dest: PathBuf,
}

/// A resolved Manifest: Connection Settings plus Entries with concrete dests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub layout: Layout,
    pub connection: ConnectionSettings,
    pub packages: Vec<Entry>,
}

/// One Entry as written in `codeartifact.yaml` (dest optional; derived in
/// versioned layout).
#[derive(Debug, Clone, Deserialize)]
pub struct RawEntry {
    #[serde(default)]
    pub namespace: Option<String>,
    pub package: String,
    pub version: String,
    #[serde(default)]
    pub dest: Option<PathBuf>,
}

/// The Manifest exactly as parsed from YAML, before dest resolution.
#[derive(Debug, Clone, Deserialize)]
pub struct RawManifest {
    #[serde(default)]
    pub layout: Layout,
    #[serde(flatten)]
    pub connection: ConnectionSettings,
    pub packages: Vec<RawEntry>,
}

/// Failure to parse or resolve a Manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid manifest: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("entry '{0}' is missing a `dest` (required in flat layout)")]
    MissingDest(String),
}

impl RawManifest {
    /// Parse a Manifest from YAML text (no dest resolution yet).
    pub fn from_yaml(yaml: &str) -> Result<Self, ManifestError> {
        Ok(serde_yaml::from_str(yaml)?)
    }

    /// Resolve each Entry's dest: in flat layout the explicit `dest` is required;
    /// in versioned layout it is derived as `<cache_root>/<package>/<version>`.
    pub fn resolve(self, cache_root: &Path) -> Result<Manifest, ManifestError> {
        let mut packages = Vec::with_capacity(self.packages.len());
        for raw in self.packages {
            let dest = match self.layout {
                Layout::Flat => raw
                    .dest
                    .clone()
                    .ok_or_else(|| ManifestError::MissingDest(raw.package.clone()))?,
                Layout::Versioned => cache_root.join(&raw.package).join(&raw.version),
            };
            packages.push(Entry {
                namespace: raw.namespace,
                package: raw.package,
                version: raw.version,
                dest,
            });
        }
        Ok(Manifest {
            layout: self.layout,
            connection: self.connection,
            packages,
        })
    }
}

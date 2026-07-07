use std::path::PathBuf;

/// The per-run CodeArtifact coordinates, declared at the top of the Manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSettings {
    pub domain: String,
    pub domain_owner: String,
    pub repository: String,
    /// Falls back to the profile's default region when absent.
    pub region: Option<String>,
}

/// One line in the Manifest's `packages` list: a single pinned Package Version
/// and the local destination folder its Assets land in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub namespace: Option<String>,
    pub package: String,
    /// Always a pinned, exact version (never "latest").
    pub version: String,
    pub dest: PathBuf,
}

/// The parsed `codeartifact.yaml`: Connection Settings plus the Entries to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub connection: ConnectionSettings,
    pub packages: Vec<Entry>,
}

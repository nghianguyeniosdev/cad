use serde::{Deserialize, Serialize};

/// A single downloadable file within a Package Version — the unit that is
/// fetched, sized, and MD5-verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// The asset file name (also its relative name within the Entry's dest).
    pub name: String,
    /// Size in bytes, as reported by the source.
    pub size: u64,
    /// Expected MD5 (lowercase hex) as reported by the source.
    pub expected_md5: String,
}

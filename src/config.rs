//! The global `~/.acd/config.yml` — currently just the Cache Root for
//! Versioned Layout. See ADR 0006.

use std::path::{Path, PathBuf};

/// Built-in default Cache Root when neither the flag nor the config file sets one.
pub const DEFAULT_CACHE_ROOT: &str = "~/Library/Caches/CocoaPods/iOSArtifactPods";

/// Failure to load the config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid config {path}: {message}")]
    Parse { path: PathBuf, message: String },
}

/// Resolve the Cache Root: `flag` wins, else `cache_root` in the config file,
/// else the built-in default. `~` is expanded to the home directory. A missing
/// config file falls back to the default; a malformed one is an error.
pub fn resolve_cache_root(
    flag: Option<String>,
    config_path: &Path,
) -> Result<PathBuf, ConfigError> {
    if let Some(flag) = flag {
        return Ok(expand_tilde(&flag));
    }
    if config_path.exists() {
        let text = std::fs::read_to_string(config_path).map_err(|e| ConfigError::Parse {
            path: config_path.to_path_buf(),
            message: e.to_string(),
        })?;
        let cfg: ConfigFile = serde_yaml::from_str(&text).map_err(|e| ConfigError::Parse {
            path: config_path.to_path_buf(),
            message: e.to_string(),
        })?;
        if let Some(root) = cfg.cache_root {
            return Ok(expand_tilde(&root));
        }
    }
    Ok(expand_tilde(DEFAULT_CACHE_ROOT))
}

#[derive(serde::Deserialize)]
struct ConfigFile {
    cache_root: Option<String>,
}

/// The default config file location: `~/.acd/config.yml`.
pub fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".acd")
        .join("config.yml")
}

/// Expand a leading `~` / `~/` to the home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

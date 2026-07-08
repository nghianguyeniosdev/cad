use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use rusqlite::Connection;

use crate::domain::Asset;
use crate::ports::{AssetListCache, AssetListKey};

/// A SQLite-backed `AssetListCache`. Best-effort: if the database cannot be
/// opened, the cache degrades to a no-op (`conn` is `None`) so it can never
/// fail a download.
pub struct SqliteAssetListCache {
    conn: Option<Mutex<Connection>>,
}

impl SqliteAssetListCache {
    /// Open (or create) the cache at `path`, degrading to a no-op on failure.
    pub fn open(path: &Path) -> Self {
        Self {
            conn: Self::try_open(path).ok().map(Mutex::new),
        }
    }

    fn try_open(path: &Path) -> rusqlite::Result<Connection> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                    Some(e.to_string()),
                )
            })?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA user_version = 1;
             CREATE TABLE IF NOT EXISTS asset_list_cache (
                 domain TEXT NOT NULL, domain_owner TEXT NOT NULL, repository TEXT NOT NULL,
                 namespace TEXT NOT NULL, package TEXT NOT NULL, version TEXT NOT NULL,
                 assets TEXT NOT NULL, cached_at INTEGER NOT NULL,
                 PRIMARY KEY (domain, domain_owner, repository, namespace, package, version)
             );",
        )?;
        Ok(conn)
    }

    /// The default cache path: `$ACD_CACHE_DIR/acd/cache.db` or the OS cache dir.
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("ACD_CACHE_DIR")
            .map(PathBuf::from)
            .or_else(dirs::cache_dir)
            .unwrap_or_else(std::env::temp_dir);
        base.join("acd").join("cache.db")
    }
}

/// Namespace column value ("" when the package has no namespace).
fn ns(key: &AssetListKey) -> &str {
    key.namespace.as_deref().unwrap_or("")
}

#[async_trait]
impl AssetListCache for SqliteAssetListCache {
    async fn get(&self, key: &AssetListKey) -> Option<Vec<Asset>> {
        let conn = self.conn.as_ref()?.lock().ok()?;
        let json: String = conn
            .query_row(
                "SELECT assets FROM asset_list_cache
                 WHERE domain=?1 AND domain_owner=?2 AND repository=?3
                   AND namespace=?4 AND package=?5 AND version=?6",
                rusqlite::params![
                    key.domain,
                    key.domain_owner,
                    key.repository,
                    ns(key),
                    key.package,
                    key.version
                ],
                |row| row.get(0),
            )
            .ok()?;
        serde_json::from_str(&json).ok()
    }

    async fn put(&self, key: &AssetListKey, assets: &[Asset]) {
        let Some(conn) = self.conn.as_ref() else {
            return;
        };
        let Ok(json) = serde_json::to_string(assets) else {
            return;
        };
        let cached_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(conn) = conn.lock() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO asset_list_cache
                 (domain, domain_owner, repository, namespace, package, version, assets, cached_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                rusqlite::params![
                    key.domain,
                    key.domain_owner,
                    key.repository,
                    ns(key),
                    key.package,
                    key.version,
                    json,
                    cached_at as i64
                ],
            );
        }
    }
}

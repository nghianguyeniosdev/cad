# 5. Cache the Enumerate Phase (parallelize first, then SQLite for immutable versions)

Date: 2026-07-08

## Status

Accepted

## Context

The Enumerate Phase (`Planner::plan`) runs to completion before any download
begins, so its latency is on the critical path of every run. It makes one
`list_package_version_assets` call per Entry.

Two problems with the original code:

1. It enumerates **sequentially** — N Entries mean N back-to-back round-trips.
2. Re-running the same Manifest re-pays those round-trips even though a
   pinned release version's asset list never changes.

Caching is tempting but has a correctness trap: **Snapshot versions are
mutable**. CodeArtifact can re-publish a `-SNAPSHOT` version in place with new
content and new MD5s, and those MD5s drive verification. A version-keyed cache
that ignored this would serve stale asset lists / stale expected-MD5s.

## Decision

**Parallelize first, cache second.**

1. **Parallelize enumerate** — `Planner` lists Assets across Entries
   concurrently (`buffer_unordered`, reusing the configured concurrency). This
   is a universal, dependency-free win and helps the first run too.

2. **Asset List Cache** — a caching **decorator** over `PackageSource`
   (`CachingPackageSource`) backed by a new **`AssetListCache`** port with a
   **SQLite adapter** (`rusqlite`, `bundled` feature — no system SQLite). The
   `Planner` and `DownloadService` are unchanged; caching is transparent.

   - **Cacheability gate**: a version is cacheable iff
     `!version.to_lowercase().contains("snapshot")` (pure, unit-testable). The
     decorator queries AWS live for Snapshot Versions and never reads/writes
     the cache for them.
   - **Key**: `(domain, domain_owner, repository, namespace, package, version)`
     with `""` as the namespace sentinel (region is not part of the key). 
   - **Value**: a JSON blob of `[{name, size, expected_md5}]` plus `cached_at`.
   - **Location**: `dirs::cache_dir()/acd/cache.db` (override `ACD_CACHE_DIR`);
     it is a regenerable cache, so it lives under the OS cache dir and is always
     safe to delete.
   - **Concurrency**: a single `rusqlite` connection behind `Arc<Mutex<…>>`
     (sub-millisecond queries, no `.await` under the lock; no pool/WAL needed).
   - **Graceful degradation**: if the DB cannot be opened/written, the cache is
     a **no-op** — caching is an optimization, never a hard dependency.
   - **Escape hatch**: `--refresh-cache` on `download` bypasses and rebuilds the
     cache for a run.

## Schema

```sql
PRAGMA user_version = 1;

CREATE TABLE IF NOT EXISTS asset_list_cache (
    domain        TEXT NOT NULL,
    domain_owner  TEXT NOT NULL,
    repository    TEXT NOT NULL,
    namespace     TEXT NOT NULL,   -- "" when the package has no namespace
    package       TEXT NOT NULL,
    version       TEXT NOT NULL,
    assets        TEXT NOT NULL,   -- JSON: [{"name","size","expected_md5"}]
    cached_at     INTEGER NOT NULL,
    PRIMARY KEY (domain, domain_owner, repository, namespace, package, version)
);
```

## Consequences

- Cold runs get faster from parallelization alone; repeat runs of pinned
  release manifests skip the listing round-trips entirely.
- No staleness risk: only immutable versions are cached; Snapshots always go
  live.
- New dependencies: `rusqlite` (bundled) and `dirs`. `Asset` gains
  `Serialize`/`Deserialize` for the JSON blob.
- The cache is best-effort — a broken/unwritable DB degrades to live queries,
  so it can never break a download.
- No automatic TTL (immutability makes it unnecessary); `--refresh-cache` covers
  the rare "a release was deleted and re-published" case.

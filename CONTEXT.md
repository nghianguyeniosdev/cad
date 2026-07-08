# acd — AWS CodeArtifact Downloader

A Rust CLI that downloads pinned sets of generic-package assets from a private
AWS CodeArtifact repository, verifies their integrity, and reports progress.

## Language

**Manifest**:
The `codeartifact.yaml` file that fully describes one fetch — connection
settings plus the list of package versions to download. The single source of
truth for a run.
_Avoid_: config, spec, lockfile

**Entry**:
One line in the manifest's `packages` list. Identifies a single package version
and the local destination folder its assets land in. One Entry = one folder.
_Avoid_: item, target

**Package Version**:
A CodeArtifact generic package pinned to an exact version (never "latest"). The
unit an Entry refers to. Maps locally to a folder.
_Avoid_: artifact, module, dependency

**Asset**:
An individual file inside a Package Version. The thing that is actually
downloaded, sized, and MD5-verified. What the user informally calls a "file".
_Avoid_: file, object, blob

**Enumerate Phase**:
The first phase of a run: list every Asset of every Entry to compute the total
file count, total size, and expected MD5s — before any download begins.
_Avoid_: scan, index

**Download Phase**:
The second phase: fetch Assets concurrently (default 10), verify, and report.

**Verify-and-Skip**:
On re-run, an Asset already present on disk whose MD5 matches is skipped and
counted as "cached" rather than re-downloaded.
_Avoid_: cache hit, resume

**Download Plan**:
The complete set of Assets to fetch, with per-Asset expected MD5 and size plus
aggregate totals, produced by the Enumerate Phase before any download begins.
_Avoid_: queue, batch, job list

**Snapshot Version**:
A mutable Package Version — its `version` contains "snapshot" (case-insensitive).
Its assets can be re-published in place, so it is never cached and is always
enumerated live.
_Avoid_: pre-release, dev build

**Asset List Cache**:
A local store (SQLite) of the enumerated Assets of *immutable* (non-Snapshot)
Package Versions, consulted before querying CodeArtifact to skip the listing
round-trip. Safe because a non-Snapshot version's asset list never changes.
_Avoid_: index, database

**Versioned Layout**:
A Manifest mode (`layout: versioned`) where each Entry's Assets download into a
derived `<Cache Root>/<package>/<version>/` folder instead of an explicit `dest`.
The default is Flat Layout — an explicit per-Entry `dest`.
_Avoid_: pods layout, cache mode

**Cache Root**:
The base directory for Versioned Layout, where downloaded Assets are stored under
`<package>/<version>/`. Read from `~/.acd/config.yml` (`cache_root`), default
`~/Library/Caches/CocoaPods/TymePods`. Distinct from the Asset List Cache (the
SQLite listing cache).
_Avoid_: output dir, cache dir

**Run Summary**:
The end-of-run tally — counts of downloaded / cached / failed Assets, total
bytes, elapsed time, and the failed-Asset list — that determines the exit code.
_Avoid_: report, result

**Doctor**:
The preflight health check (aws CLI present, v2, profile exists, dest writable).
Runs both as the `acd doctor` command and automatically before `download`.
_Avoid_: healthcheck, precheck, validate

**Connection Settings**:
The per-run CodeArtifact coordinates — domain, domain-owner, repository, region
— declared at the top of the Manifest, overridable by CLI flags.

**Auth-Expiry Error**:
An auth-class download failure (expired SSO session / `UnauthorizedException`),
classified distinctly from transient-network and hard failures. Triggers a
Session Re-login and does **not** consume an Asset's retry budget.
_Avoid_: 403, token error

**Session Re-login**:
The pause-the-world flow when the SSO session expires mid-run: the first worker
to hit an Auth-Expiry Error pauses the whole pool, runs a single `aws sso login`,
waits for it to exit, then all workers resume with refreshed credentials.
Repeatable within a run; aborts after 2 consecutive re-logins that yield no
download progress.
_Avoid_: refresh, re-auth, token renewal

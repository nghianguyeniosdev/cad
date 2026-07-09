# 6. Versioned Layout + global config file

Date: 2026-07-08

## Status

Accepted. The "acd never extracts" boundary is partially superseded by ADR 0007
(versioned layout now extracts into `PodLocals`).

## Context

`acd` is being pointed at the iOS dependency workflow, whose artifacts live in a
version-keyed cache:

```
~/Library/Caches/CocoaPods/iOSArtifactPods/<package>/<version>/artifact.zip
```

`acd`'s current model requires an explicit `dest` per Entry. Reproducing the
layout above by hand — an explicit `dest` for each of ~286 packages, changing
every version bump — is impractical. The path is naturally *derivable* from the
package + version, so `acd` should derive it.

The cache-root base directory varies by machine/setup, so it needs to be
configurable, with a sensible default.

`acd` downloads the archive (e.g. `artifact.zip`) into the version folder and
**keeps it as-is**. Extracting/unzipping the archive is **out of scope for
`acd`** — it happens in a later phase inside the iOS repo, not here.

## Decision

**Versioned Layout mode.** A Manifest gains a top-level `layout` field:

- `layout: flat` (default) — current behavior; each Entry has an explicit `dest`.
- `layout: versioned` — each Entry's Assets download into a derived
  `<cache_root>/<package>/<version>/` folder. `dest` is optional and ignored in
  this mode. No namespace segment appears in the path (matching the iOSArtifactPods
  layout `<package>/<version>`). Assets keep their real CodeArtifact names.

Consequently `Entry.dest` becomes optional in the domain (required only in flat
mode; validated per mode).

**Global config `~/.acd/config.yml`.** A single key for now:

```yaml
cache_root: ~/Library/Caches/CocoaPods/iOSArtifactPods
```

- **Precedence** (highest first): `--cache-root` flag → `cache_root` in the
  config file → built-in default `~/Library/Caches/CocoaPods/iOSArtifactPods`.
- `~` in the value (and default) expands to `$HOME`.
- A **missing** config file uses the default silently (config is optional).
- A **malformed** config file (bad YAML / wrong type) is a **hard error** — a
  typo'd-and-ignored config is a worse surprise than a clear failure.

## Consequences

- A whole Manifest is either "iOS pods → the versioned cache" or "arbitrary
  files → explicit dests"; the layout rule lives in one place.
- `acd` can populate the iOSArtifactPods cache without per-entry dests, a step toward
  replacing `iosPrepareArtifact.sh`.
- New global config file (distinct from the Manifest, which stays per-fetch).
- "Cache Root" (downloaded-asset store) is distinct from the "Asset List Cache"
  (SQLite listing cache, ADR 0005) — two different caches, don't conflate.
- `acd` stores the downloaded archive unchanged; **it never extracts**.
  Unzipping is done downstream by the iOS repo in a later phase.

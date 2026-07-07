# 2. Verify asset integrity with MD5

Date: 2026-07-07

## Status

Accepted

## Context

`ListPackageVersionAssets` returns a `hashes` map per asset containing MD5,
SHA-1, SHA-256, and SHA-512. During the Download Phase each asset's bytes are
hashed and compared to an expected hash captured during the Enumerate Phase, to
detect corruption before the temp file is renamed into place.

We had to pick which hash is the integrity check of record.

## Decision

Verify against **MD5**.

- Computed by streaming downloaded bytes through the MD5 hasher in a single
  pass (no second read of the file).
- On mismatch: retry the download up to 3 times with backoff, then mark the
  asset failed and continue the run.

## Consequences

- Meets the explicit product requirement ("check md5, display download ok").
- MD5 is sufficient for detecting accidental corruption in transit/storage.
- SHA-256 (also available) is more robust and avoids MD5's known unreliability
  on some multipart-uploaded assets. It was consciously **not** chosen; if
  integrity guarantees ever need strengthening, switching the expected-hash
  source to SHA-256 is a localized change.

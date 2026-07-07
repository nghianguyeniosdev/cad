# 1. Hybrid: aws CLI for SSO login, AWS SDK for downloads

Date: 2026-07-07

## Status

Accepted

## Context

The tool must authenticate against a private AWS CodeArtifact repository via AWS
SSO, then list and download generic-package assets while showing apt-style
per-file and overall progress.

Two architectures were possible:

- **Pure CLI wrapper** — shell out to `aws codeartifact ...` for every
  operation. Simple, reuses the CLI's SSO session, but subprocess output gives
  essentially no byte-level progress and spawns a process per call (costly at
  10 concurrent downloads).
- **Pure SDK** — do everything (including SSO) in-process. Best data-plane
  control, but reimplements the SSO browser login flow, which is fiddly and
  already solved well by the CLI.

## Decision

Use a hybrid:

- The `aws` CLI is used **only** for `aws sso login` (spawned when the token is
  missing or expired).
- The **AWS SDK for Rust** (`aws-sdk-codeartifact`) performs all
  `ListPackageVersionAssets` and `GetPackageVersionAsset` calls and streams
  downloads.

Consequently, `doctor` checks for the `aws` CLI as a **login dependency**, not
a data-plane one.

## Consequences

- Real byte-level streaming progress feeds the per-file `indicatif` bars.
- Clean async concurrency (default 10) without per-call process spawning.
- Adds the AWS Rust SDK dependency and in-process credential/profile
  resolution.
- SSO's interactive browser flow stays delegated to the supported, maintained
  CLI path.

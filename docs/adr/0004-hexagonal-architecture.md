# 4. Lean hexagonal architecture (ports & adapters)

Date: 2026-07-07

## Status

Accepted

## Context

The tool must apply SOLID and remain testable without hitting AWS, while staying
proportionate to a single-purpose CLI. The design has clear I/O boundaries
(CodeArtifact, SSO login, filesystem, terminal UI) and non-trivial orchestration
(two-phase enumerate→download, 10-way concurrency, retry, mid-run re-login).

A flatter by-feature layout was considered but mixes I/O with logic inside each
feature, making the pipeline hard to unit-test.

## Decision

**Library + thin binary.** All logic lives in `lib`; `main.rs` only parses args,
calls the library, and maps results to exit codes.

**Lean hexagonal layering** with a strict dependency rule:

- `domain` → nothing. Pure types and logic: Manifest, Entry, Asset,
  DownloadPlan, AssetOutcome, RunSummary, and `FailureKind{AuthExpired,
  Transient, Fatal}`.
- `ports` → `domain`. The trait seams: `PackageSource` (unified
  `list_assets` + `fetch_asset`), `Authenticator`, `FileStore`,
  `ProgressReporter`.
- `app` → `domain` + `ports`. Orchestration: `Planner`, `Downloader`,
  `AssetTask`, `RetryPolicy` (Strategy), and `SessionCoordinator`.
- `adapters` → `ports` + `domain`. Concrete impls: AWS SDK, `aws sso login`
  subprocess, local filesystem (`.part` + atomic rename), `indicatif`/plain
  progress.
- `doctor` is a cohesive module (Check trait + concrete checks + Composite
  runner) rather than being split across layers — proportionate to its size.
- `cli` / `wiring` / `main` are the composition root and may depend on
  everything.

**Cross-cutting choices:**

- Ports use `Arc<dyn Trait>` (dynamic dispatch) + the `async-trait` crate. The
  vtable cost is irrelevant for I/O-bound work; workers share ports by cloning
  `Arc` into `tokio` tasks, and mocks swap in trivially for tests.
- `SessionCoordinator` is the **sole gateway** for credentialed calls: it owns
  error classification (auth vs transient vs fatal) and the single-flight
  pause-the-world re-login, so that logic lives in exactly one place (SRP).
- `thiserror` per-layer error enums + `FailureKind` in the library; `anyhow`
  only at the binary edge for context and exit-code mapping.
- Composition root in `wiring.rs`: constructs concrete adapters and injects
  them into `app` services.

## Directory layout

```
src/
├── main.rs            # thin binary: args → app → exit codes (anyhow)
├── lib.rs             # module decls + run() entry
├── wiring.rs          # composition root: build adapters, inject into app
├── cli/               # clap defs + command handlers (download, doctor, init)
├── domain/            # manifest, asset, plan, outcome, error (pure)
├── ports/             # package_source, authenticator, file_store, progress
├── app/               # planner, downloader, asset_task, retry, session
├── adapters/          # aws/, sso/, fs/, progress/
└── doctor/            # Check trait + checks + Composite runner
tests/
├── download_flow.rs   # integration: fakes drive app end-to-end (no AWS)
└── fakes/             # in-memory ports for tests
```

## Consequences

- The AWS/SSO/filesystem/terminal boundaries are mockable, so the pipeline
  (concurrency, retry, re-login) is unit- and integration-testable without AWS.
- More layers and trait indirection than a by-feature layout; the layering is
  kept lean (traits only at real I/O boundaries, cohesive `doctor/`) to avoid
  ceremony.
- Dynamic dispatch means the dependency direction is enforced by discipline and
  module structure rather than by separate crates; can be promoted to a
  workspace later if any piece needs independent reuse.

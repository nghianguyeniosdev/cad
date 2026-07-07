# PRD: `acd` — AWS CodeArtifact Downloader (MVP)

Labels: `ready-for-agent`
Status: Draft (local — not yet published to tracker)

## Problem Statement

Engineers need to pull sets of files (generic-package Assets) from a private AWS
CodeArtifact repository onto their machines. Today this means hand-running
`aws codeartifact` commands per package, manually tracking which files to fetch,
having no visibility into total download size or per-file progress, no integrity
verification, and repeated friction around AWS SSO sessions — including sessions
that expire in the middle of a long download and force starting over. There is
no single, reproducible, reviewable description of "what to download," and no
fast, safe way to re-run a fetch.

## Solution

A single Rust binary, `acd`, installable via Homebrew, that reads a **Manifest**
(`codeartifact.yaml`) describing the CodeArtifact **Connection Settings** and a
pinned list of **Entries** (each an exact Package Version → a destination
folder), then:

- runs a **Doctor** preflight (aws CLI present + v2, profile exists, destination
  writable),
- authenticates via AWS SSO (auto-triggering `aws sso login` when the session is
  missing or expired),
- performs an **Enumerate Phase** to build a **Download Plan** and print an
  apt-style summary (total Assets, total size),
- performs a concurrent **Download Phase** (default 10) with a hybrid progress UI
  (up to 10 live per-Asset bars + an overall bar + persistent `✓` log lines),
- verifies each Asset by **MD5**, skips already-present valid Assets
  (**Verify-and-Skip**), writes atomically via a `.part` temp file,
- survives mid-run SSO expiry via **Session Re-login** (pause-the-world,
  single-flight), and
- prints a **Run Summary** (downloaded / cached / failed, bytes, elapsed) and
  exits with a meaningful code.

## User Stories

1. As an engineer, I want to install `acd` via `brew install`, so that I don't
   need a Rust toolchain to use it.
2. As an engineer, I want to describe everything to download in one
   `codeartifact.yaml` Manifest, so that a fetch is reproducible and reviewable.
3. As an engineer, I want to scaffold a starter Manifest with `acd init`, so
   that I don't have to author the file from scratch.
4. As an engineer, I want the Manifest to pin exact Package Versions, so that
   re-running produces identical results.
5. As an engineer, I want each Entry to declare a destination folder, so that
   Assets from different packages don't collide.
6. As an engineer, I want the Manifest to carry Connection Settings (domain,
   domain-owner, repository, region), so that one file fully describes the fetch.
7. As an engineer, I want to override Connection Settings and the profile via CLI
   flags, so that I can reuse a Manifest across environments.
8. As an engineer, I want `acd doctor` to check my environment (aws CLI present,
   v2, profile exists, destination writable), so that I learn about problems
   before a long download starts.
9. As an engineer, I want the same Doctor checks to run automatically before
   `download`, so that I can't accidentally skip them.
10. As an engineer, I want a Doctor failure to tell me exactly what to fix, so
    that I can resolve it quickly.
11. As an engineer, I want the tool to authenticate with my AWS SSO profile, so
    that I can access the private repository.
12. As an engineer, I want `acd` to auto-run `aws sso login` when my session is
    missing or expired, so that I'm not dumped out to fix auth myself.
13. As an engineer, I want the tool to choose my profile from `--profile` (or
    `AWS_PROFILE`, or the Manifest default), so that profile selection is
    explicit and scriptable.
14. As an engineer, I want an upfront summary of total Assets and total size
    before downloading, so that I know what I'm committing to (apt-style).
15. As an engineer, I want up to 10 Assets downloading concurrently, so that
    large fetches finish quickly.
16. As an engineer, I want to tune concurrency with `--concurrency`, so that I
    can back off when the network or CodeArtifact throttles.
17. As an engineer, I want a live progress bar per in-flight Asset showing name,
    size, and percent, so that I can see what's happening.
18. As an engineer, I want an overall progress bar (files done / total, bytes
    done / total), so that I can gauge total remaining work.
19. As an engineer, I want each completed Asset to print a persistent `✓` line
    with its size and `md5 ok`, so that I have a durable log of what succeeded.
20. As an engineer, I want progress to degrade to plain log lines when output
    isn't a TTY (CI, piped), so that logs stay readable.
21. As an engineer, I want each downloaded Asset verified by MD5 against the
    hash from CodeArtifact, so that I can trust the files are intact.
22. As an engineer, I want a failed MD5 to retry up to 3 times, so that
    transient corruption self-heals.
23. As an engineer, I want a single bad Asset to not abort the whole run, so
    that one failure doesn't waste a large fetch.
24. As an engineer, I want interrupted downloads to never leave a corrupt
    "complete" file (temp `.part` + atomic rename), so that re-runs are safe.
25. As an engineer, I want re-running to skip Assets already present with a
    matching MD5 (Verify-and-Skip), so that re-runs are cheap and resumable.
26. As an engineer, I want cached Assets counted separately (not as downloads),
    so that the Run Summary reflects reality.
27. As an engineer, when my SSO session expires mid-download, I want the tool to
    re-login and continue rather than fail, so that long fetches survive expiry.
28. As an engineer, I want exactly one browser login even when many concurrent
    Assets hit the expiry at once (pause-the-world, single-flight), so that I'm
    not flooded with browser tabs.
29. As an engineer, I want a clear "SSO session expired — waiting for browser
    login…" indication during Session Re-login, so that I know why it paused.
30. As an engineer, I want an Auth-Expiry Error to not consume an Asset's retry
    budget, so that one expiry event doesn't prematurely fail many Assets.
31. As an engineer, I want Assets interrupted by expiry to re-download from
    scratch after re-login, so that partial bytes don't corrupt results.
32. As an engineer, I want Assets that already hard-failed to stay failed after
    re-login, so that re-login only revives expiry-interrupted work.
33. As an engineer, I want the run to abort if a login fails (non-zero exit), so
    that I'm not stuck retrying a broken auth setup.
34. As an engineer, I want the run to abort after 2 consecutive re-logins that
    make no progress, so that a misconfig/clock-skew loop can't hang forever.
35. As an engineer, I want a Run Summary at the end (downloaded / cached /
    failed, total bytes, elapsed), so that I know the outcome at a glance.
36. As an engineer, I want the failed-Asset list printed with reasons, so that I
    can act on failures.
37. As an engineer, I want exit code 0 on full success, 1 on Asset failures, and
    2 on Doctor failure, so that CI can branch on the result.
38. As an engineer, I want `acd version`, so that I can report which build I'm
    running.
39. As an engineer, I want Ctrl-C at any point (including during login wait) to
    cancel cleanly with no half-written files, so that aborting is safe.

## Implementation Decisions

- **Distribution:** Prebuilt macOS binaries (Apple Silicon + Intel) via GitHub
  Actions + GitHub Releases; installed from a public Homebrew tap
  (`nghianguyeniosdev/homebrew-tap`) with an `on_macos`/`on_arm`/`on_intel`
  binary formula. Not build-from-source. (ADR 0004 covers code structure; the
  Homebrew tap lives in a separate repo.)
- **Crate topology:** Library + thin binary. `main.rs` only parses args, calls
  the library, and maps outcomes to exit codes.
- **Architecture:** Lean hexagonal (ADR 0004) — `domain` (pure) / `ports`
  (traits) / `app` (orchestration) / `adapters` (concrete I/O) + a cohesive
  `doctor` module. Composition root in `wiring.rs`.
- **Ports (four, `Arc<dyn Trait>` + `async-trait`):**
  - `PackageSource` — unified `list_assets(entry)` (Enumerate) +
    `fetch_asset(asset)` (Download). Adapter: `aws-sdk-codeartifact`.
  - `Authenticator` — `session_status(profile)` + `login(profile)`. Adapter:
    `aws sso login` subprocess.
  - `FileStore` — `existing_md5(dest)` + `stage()` (`.part`) + `commit()`
    (atomic rename). Adapter: local filesystem.
  - `ProgressReporter` — lifecycle hooks. Adapters: `indicatif` multi-bar +
    plain (non-TTY) reporter.
- **Doctor:** cohesive module — a `Check` trait with concrete checks
  (CliInstalled, CliVersion, ProfileExists, DestWritable) run via a Composite
  runner. Invoked standalone (`acd doctor`) and as a `download` preflight.
- **Orchestration (`app`):** `Planner` (Enumerate → Download Plan), `Downloader`
  (bounded worker pool, semaphore, default 10), `AssetTask` (skip? → stage →
  stream-through-MD5 → verify → commit), `RetryPolicy` (Strategy: 3 attempts +
  backoff), `SessionCoordinator`.
- **SessionCoordinator is the sole gateway for credentialed calls.** It owns
  error classification and the single-flight pause-the-world Session Re-login.
  `Downloader`/`AssetTask` never call `Authenticator` directly. (ADR 0003.)
- **Error model:** `thiserror` per-layer enums + a `FailureKind { AuthExpired,
  Transient, Fatal }` classification produced by adapters; `SessionCoordinator`
  and `RetryPolicy` branch on `FailureKind` (never on error strings). `anyhow`
  only at the binary edge for context + exit-code mapping.
- **AWS access model (ADR 0001):** hybrid — `aws` CLI only for `aws sso login`;
  AWS SDK for Rust for all listing and downloading (enables byte-level streaming
  progress and clean async concurrency).
- **Integrity (ADR 0002):** verify against **MD5** from
  `ListPackageVersionAssets`, streamed single-pass during download; mismatch →
  retry 3× then mark failed and continue.
- **Manifest:** `codeartifact.yaml` (default filename; `--manifest` to override).
  Top-level Connection Settings (domain, domain_owner, repository, region) +
  `packages` list of Entries (namespace, package, version, dest). Version always
  pinned. CLI flags override Connection Settings.
- **CLI surface:** `acd download | doctor | version | init`.
- **Exit codes:** 0 success, 1 Asset failures, 2 Doctor failure.

## Testing Decisions

- **What makes a good test here:** it asserts *external behavior* observed at the
  library seam — the resulting Run Summary, the set/contents of Assets committed
  through the `FileStore`, recorded `ProgressReporter` events, and the process
  exit code — never internal method calls or private state.
- **Primary seam (preferred, single):** the library entry point (`run()` /
  command handler) driven with the four ports replaced by **in-memory fakes**
  (`tests/fakes/`): a fake `PackageSource` returning scripted Assets and
  scriptable failures (transient / hash-mismatch / auth-expired), a fake
  `FileStore` backed by an in-memory map, a fake `Authenticator` that can emit
  `AuthExpired` on a schedule and record `login()` calls, and a recording
  `ProgressReporter`. This one seam covers manifest parsing → Enumerate → Plan
  totals → concurrent Download → Verify-and-Skip → MD5 verification → retry
  budget → Session Re-login (single-flight + no-progress guard) → Run Summary +
  exit codes, all without touching AWS.
- **Modules tested through the seam:** all of `app` (Planner, Downloader,
  AssetTask, RetryPolicy, SessionCoordinator), `domain` (Manifest parse/validate,
  Download Plan, Run Summary), and the Doctor Composite (via fake checks).
- **Below the seam (thin adapters):** `adapters/aws`, `adapters/sso`, and
  `adapters/progress` are not exercised through the primary seam. `adapters/fs`
  may get a small focused test for `.part` + atomic-rename semantics. AWS/SSO
  adapters are verified manually against a real repository.
- **Prior art:** none in this greenfield repo — `tests/fakes/` and
  `tests/download_flow.rs` establish the pattern.

## Out of Scope

- Non-macOS platforms (Linux/Windows) and cross-compilation.
- `latest`/floating version resolution — versions are always pinned.
- Cherry-picking individual Assets within a Package Version — an Entry always
  means all Assets of that Package Version.
- SHA-256 (or other) integrity checks — MD5 only for MVP.
- HTTP range-resume of a partially downloaded Asset — interrupted Assets restart
  from scratch.
- Remote-hosted Manifests — the Manifest is a local file.
- Uploading/publishing to CodeArtifact — download only.
- Non-generic CodeArtifact formats (npm/maven/pypi/nuget).
- A custom browser-callback listener for SSO — login is delegated to the
  `aws sso login` subprocess.
- The Homebrew tap repository and release workflow implementation (tracked
  separately; this PRD assumes the binary distribution decision only).

## Further Notes

- Ubiquitous language is defined in `CONTEXT.md`; behavioral decisions are in
  ADRs 0001 (hybrid AWS access), 0002 (MD5 integrity), 0003 (mid-run
  re-login); structure in ADR 0004 (lean hexagonal).
- CodeArtifact's MD5 can be unreliable for some multipart-uploaded assets;
  SHA-256 is also available and switching the expected-hash source is a
  localized change if integrity guarantees ever need strengthening (ADR 0002).
- Publishing: this PRD is saved locally pending `gh auth login`. When published
  to `nghianguyeniosdev/cad`, apply the `ready-for-agent` label.

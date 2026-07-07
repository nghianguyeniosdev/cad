# 3. Mid-run SSO re-login (pause-the-world)

Date: 2026-07-07

## Status

Accepted

## Context

An SSO session can expire *during* the Download Phase, not just at preflight.
On a long run (large asset set, slow link) in-flight `GetPackageVersionAsset`
calls start failing with an auth-class error (expired token /
`UnauthorizedException`). With 10 concurrent workers, expiry hits many workers
within seconds of each other.

Naive handling — each failed worker triggers its own `aws sso login` — would
spawn up to 10 browser tabs. Aborting the whole run on first auth error is
simpler but discards all in-progress work.

## Decision

Handle mid-run expiry with **reactive detection** plus **pause-the-world
single-flight re-login**:

- Downloads classify failures as **Auth-Expiry**, transient-network, or hard.
- The first worker to hit an Auth-Expiry Error acquires a lock and pauses the
  whole pool; the others block on it.
- One `aws sso login` runs; the tool blocks on that subprocess to exit 0.
  Non-zero exit → abort the run (exit code 1). Rely on the CLI's own login
  timeout.
- On success, all workers resume with refreshed credentials.

Accounting rules:

- An Auth-Expiry Error is a **pause/resume, not a retry attempt** — it does not
  decrement an Asset's 3× retry budget.
- The interrupted asset's partial `.part` file is discarded and re-downloaded
  from scratch (no HTTP range-resume mid-file).
- Assets that had already hard-failed (exhausted retries) before the expiry
  stay failed; re-login does not resurrect them.
- Re-login is repeatable within a run, but the run **aborts after 2 consecutive
  re-logins that produce no successful download in between** (guards against a
  misconfig/clock-skew ping-pong). Real progress resets the counter.

## Consequences

- Long downloads survive session expiry without losing in-progress work.
- Exactly one browser login per expiry event.
- Added coordination complexity: a pool-wide pause, a single-flight login lock,
  and no-progress tracking for the loop guard.

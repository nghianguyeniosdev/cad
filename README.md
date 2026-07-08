# acd — AWS CodeArtifact Downloader

A small Rust CLI that downloads pinned sets of **generic-package** assets from a
private AWS CodeArtifact repository, verifies them by MD5, shows apt-style
progress, and recovers from SSO session expiry.

## Install (Homebrew, macOS)

```sh
brew tap nghianguyeniosdev/tap
brew install acd
```

Prebuilt binaries are published for Apple Silicon and Intel macOS.

## Usage

```sh
acd init                        # scaffold a codeartifact.yaml
aws sso login --profile <p>     # (acd will also auto-login if needed)
acd download --manifest codeartifact.yaml --profile <p>
```

Commands: `acd download | doctor | init | version`.

`acd download` flags: `--manifest <path>` (default `codeartifact.yaml`),
`--profile <name>`, `--concurrency <n>` (default 10).

### Manifest (`codeartifact.yaml`)

```yaml
domain: my-domain
domain_owner: "111122223333"       # 12-digit account id, quoted
repository: my-repo
region: ap-southeast-1             # optional
packages:
  - namespace: my-namespace        # generic packages require a namespace
    package: my-package
    version: "1.4.2"               # pinned, quoted
    dest: ./artifacts/my-package
```

Each entry downloads **all** assets of one pinned Package Version into `dest`.
Re-runs skip files already present with a matching MD5 (verify-and-skip).

## Exit codes

`0` success · `1` one or more assets failed · `2` usage / environment
(bad manifest, SSO login failed) · `3` recognized-but-unimplemented command.

## Releasing

Tag a version to trigger `.github/workflows/release.yml`:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

CI cross-builds both macOS targets, attaches the tarballs to a GitHub Release,
and generates a ready-to-paste `acd.rb`. Copy that `acd.rb` into
`Formula/acd.rb` in the `nghianguyeniosdev/homebrew-tap` repo to publish the
new version. A reference formula lives at [`packaging/acd.rb`](packaging/acd.rb).

## Development

```sh
cargo test          # unit + integration tests (in-memory fakes, no AWS)
cargo clippy --all-targets -- -D warnings
cargo run --example relogin_demo -- <profile>   # manual mid-run re-login demo
```

Architecture and decisions: see [`CONTEXT.md`](CONTEXT.md) and
[`docs/adr/`](docs/adr/).

# acd — developer tasks.
#
#   make            # list targets
#   make check      # the full CI gate: fmt-check + clippy + test
#   make test       # run all tests (unit + integration, no AWS)
#   make download   # run a real download (needs AWS SSO)

# Overridable:  make download PROFILE=my-profile MANIFEST=path.yaml
PROFILE ?= TymeX-AWS-Engineer-Wks
MANIFEST ?= codeartifact.yaml

.DEFAULT_GOAL := help

.PHONY: help
help: ## List available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

# ─── Quality gate ────────────────────────────────────────────────────────────
.PHONY: check
check: fmt-check lint test ## Run the full CI gate (fmt-check + clippy + test)

.PHONY: test
test: ## Run all tests
	cargo test

.PHONY: lint
lint: ## Clippy with warnings denied (all targets)
	cargo clippy --all-targets -- -D warnings

.PHONY: fmt
fmt: ## Format the code
	cargo fmt --all

.PHONY: fmt-check
fmt-check: ## Verify formatting (no changes)
	cargo fmt --all -- --check

# ─── Build ───────────────────────────────────────────────────────────────────
.PHONY: build
build: ## Debug build
	cargo build

.PHONY: release
release: ## Optimized release build
	cargo build --release

.PHONY: clean
clean: ## Remove build artifacts and downloaded assets
	cargo clean
	rm -rf artifacts

# ─── Run against real AWS (needs SSO) ────────────────────────────────────────
.PHONY: doctor
doctor: build ## Run environment checks for PROFILE
	./target/debug/acd doctor --profile $(PROFILE)

.PHONY: download
download: build ## Download MANIFEST using PROFILE
	./target/debug/acd download --manifest $(MANIFEST) --profile $(PROFILE)

.PHONY: init
init: build ## Scaffold a codeartifact.yaml
	./target/debug/acd init

.PHONY: demo
demo: ## Manual mid-run Session Re-login demo (opens a browser login)
	cargo run --example relogin_demo -- $(PROFILE)

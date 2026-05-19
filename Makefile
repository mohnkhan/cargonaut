# Cargonaut Makefile — thin wrappers around cargo + dev-ergonomics targets.
#
# Targets (alphabetical within each group):
#
#   Build / test:
#     build             cargo build --workspace
#     build-release     cargo build --release --workspace
#     test              cargo test --workspace --all-targets
#     clippy            cargo clippy --workspace --all-targets -- -D warnings
#     fmt               cargo fmt --all
#     fmt-check         cargo fmt --all -- --check
#     clean             cargo clean (symlink-aware; preserves tmpfs targets)
#
#   CI:
#     ci-local          Run the full CI pipeline locally
#
#   Dev ergonomics (single-user dev box only — auto-skipped on CI):
#     tmpfs-setup       Redirect target/ into /tmp/cargonaut/<hash>/ to spare the SSD
#     tmpfs-status      Show whether target/ is tmpfs-symlinked + disk usage
#     tmpfs-teardown    Remove the symlink; pass WIPE=1 to also rm -rf the tmpfs subdir
#
#   Help:
#     help              Show this help

.PHONY: help build build-release test clippy fmt fmt-check clean \
        ci-local tmpfs-setup tmpfs-status tmpfs-teardown

# Default goal: print help instead of building, so a user who types `make`
# without arguments sees what's available.
.DEFAULT_GOAL := help

help:
	@echo "Cargonaut — 'make help' target reference"
	@echo
	@echo "Build / test:"
	@echo "  build             cargo build --workspace"
	@echo "  build-release     cargo build --release --workspace"
	@echo "  test              cargo test --workspace --all-targets"
	@echo "  clippy            cargo clippy --workspace --all-targets -- -D warnings"
	@echo "  fmt               cargo fmt --all"
	@echo "  fmt-check         cargo fmt --all -- --check"
	@echo "  clean             cargo clean (symlink-aware; preserves tmpfs targets)"
	@echo
	@echo "CI:"
	@echo "  ci-local          Run the full CI pipeline locally (fmt+clippy+test+build+docs-gate)"
	@echo
	@echo "Dev ergonomics (single-user dev box only — auto-skipped on CI):"
	@echo "  tmpfs-setup       Redirect target/ into /tmp/cargonaut/<hash>/ to spare the SSD."
	@echo "                    Reversible. Idempotent. Run once per checkout."
	@echo "  tmpfs-status      Show whether target/ is tmpfs-symlinked + disk usage."
	@echo "  tmpfs-teardown    Remove the symlink; pass WIPE=1 to also rm -rf the tmpfs subdir."
	@echo
	@echo "Variables:"
	@echo "  WIPE=1            For tmpfs-teardown: also rm -rf the tmpfs subdir"

# ── Build / test ──────────────────────────────────────────────────────────────

build:
	cargo build --workspace

build-release:
	cargo build --release --workspace

test:
	cargo test --workspace --all-targets

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

# ── CI ────────────────────────────────────────────────────────────────────────

ci-local:
	@bash scripts/ci/ci-local.sh

# ── tmpfs (dev-ergonomics) ────────────────────────────────────────────────────
# Redirect Cargo's `target/` into tmpfs so heavy build iteration doesn't
# burn SSD writes. Single-user dev convenience; reversible; auto-skipped on CI.
# See docs/dev-tmpfs.md for design + caveats.
#
# The tmpfs subdir is namespaced by a hash of the absolute repo root path so
# multiple checkouts of cargonaut don't fight over the same directory.

CARGONAUT_TMPFS_HASH := $(shell printf '%s' "$(CURDIR)" | sha256sum | cut -c1-12)
CARGONAUT_TMPFS_ROOT := /tmp/cargonaut/$(CARGONAUT_TMPFS_HASH)

tmpfs-setup:
	@if [ "$$CI" = "true" ]; then \
	  echo "[tmpfs-setup] CI detected; skipping (this is a dev-box knob)"; \
	  exit 0; \
	fi
	@bash scripts/tmpfs-setup.sh "$(CARGONAUT_TMPFS_ROOT)"

tmpfs-status:
	@bash scripts/tmpfs-status.sh "$(CARGONAUT_TMPFS_ROOT)"

tmpfs-teardown:
	@bash scripts/tmpfs-teardown.sh "$(CARGONAUT_TMPFS_ROOT)" "$(WIPE)"

# ── Clean ─────────────────────────────────────────────────────────────────────
# Symlink-aware: when target/ is a tmpfs symlink (after `make tmpfs-setup`),
# we empty its contents but leave the symlink intact so the tmpfs
# association survives. When it's a real directory on disk, run cargo clean.
clean:
	@if [ -L target ]; then \
	  echo "[clean] target is a symlink — emptying tmpfs contents"; \
	  find "$$(readlink -f target)" -mindepth 1 -delete 2>/dev/null || true; \
	else \
	  cargo clean; \
	fi
	@rm -rf dist/ci-artifacts

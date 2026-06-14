# Cargonaut Makefile — thin wrappers around cargo + dev-ergonomics targets.
#
# Targets (alphabetical within each group):
#
#   Build / test:
#     build             cargo build --workspace (debug)
#     build-release     cargo build --release --workspace
#     static            fully static release binary via musl (Linux)
#     run               cargo run -p cargonaut -- $(ARGS)
#     test              cargo test --workspace --all-targets
#     clippy            cargo clippy --workspace --all-targets -- -D warnings
#     fmt               cargo fmt --all
#     fmt-check         cargo fmt --all -- --check
#     clean             cargo clean (symlink-aware; preserves tmpfs targets)
#
#   Install / distribute:
#     install           install the release binary to $(DESTDIR)$(BINDIR)
#     uninstall         remove the installed binary
#     dist              stripped static binary + tarball under dist/
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

.PHONY: help build build-release static run test clippy fmt fmt-check clean \
        install uninstall dist \
        ci-local tmpfs-setup tmpfs-status tmpfs-teardown check-tmpfs bench

# Default goal: print help instead of building, so a user who types `make`
# without arguments sees what's available.
.DEFAULT_GOAL := help

# ── Install / distribution knobs (override on the command line) ────────────────
# GNU-conventional install variables so `make install` works for both a local
# install and a packaging build (DESTDIR-staged).
PREFIX      ?= /usr/local
BINDIR      ?= $(PREFIX)/bin
DESTDIR     ?=
# Triple used for the fully-static build (musl). Linux-only.
MUSL_TARGET ?= x86_64-unknown-linux-musl
# Workspace version (from the root Cargo.toml [workspace.package]) for dist naming.
CARGONAUT_VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
STRIP       ?= strip

help:
	@echo "Cargonaut — 'make help' target reference"
	@echo
	@echo "Build / test (all gated by check-tmpfs per Constitution §V):"
	@echo "  build             cargo build --workspace (debug)"
	@echo "  build-release     cargo build --release --workspace (optimized)"
	@echo "  static            fully static release binary via musl (Linux; needs musl target)"
	@echo "  run               cargo run -p cargonaut -- \$$(ARGS)  (e.g. make run ARGS='~ /tmp')"
	@echo "  test              cargo test --workspace --lib --tests"
	@echo "  bench             cargo bench --workspace"
	@echo "  clippy            cargo clippy --workspace --all-targets -- -D warnings"
	@echo "  fmt               cargo fmt --all"
	@echo "  fmt-check         cargo fmt --all -- --check"
	@echo "  clean             cargo clean (symlink-aware; preserves tmpfs targets)"
	@echo "  check-tmpfs       Constitution §V guard — error if target/ is on SSD"
	@echo
	@echo "Install / distribute:"
	@echo "  install           install release binary to \$$(DESTDIR)\$$(BINDIR)  (default /usr/local/bin)"
	@echo "  uninstall         remove the installed binary"
	@echo "  dist              stripped static binary + .tar.gz under dist/  (Linux)"
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
	@echo "  PREFIX=/usr/local   install prefix (BINDIR defaults to \$$(PREFIX)/bin)"
	@echo "  DESTDIR=            staged-install root for packaging (e.g. DESTDIR=/tmp/pkg)"
	@echo "  MUSL_TARGET=x86_64-unknown-linux-musl   triple for static/dist"
	@echo "  WIPE=1              For tmpfs-teardown: also rm -rf the tmpfs subdir"

# ── Build / test ──────────────────────────────────────────────────────────────
# Heavy build targets depend on `check-tmpfs` per Constitution §V — see
# .specify/memory/constitution.md (SSD Preservation, NON-NEGOTIABLE).

build: check-tmpfs
	cargo build --workspace

build-release: check-tmpfs
	cargo build --release --workspace

# Fully static release binary via the musl target — runs on any Linux without
# a libc dependency (verify with `ldd target/$(MUSL_TARGET)/release/cargonaut`
# → "statically linked"). Linux-only; auto-adds the rustup target if missing.
static: check-tmpfs
	@rustup target list --installed 2>/dev/null | grep -qx '$(MUSL_TARGET)' \
	  || { echo "[static] adding rust target $(MUSL_TARGET)"; rustup target add $(MUSL_TARGET); }
	cargo build --release --target $(MUSL_TARGET) -p cargonaut
	@echo "[static] $$(file target/$(MUSL_TARGET)/release/cargonaut)"

# Build + launch the TUI. Pass pane paths (and any other CLI flags) via ARGS,
# e.g. `make run ARGS="~ /tmp"`. With no ARGS the binary's own defaults apply
# (LEFT=$HOME, RIGHT=/tmp). Gated by check-tmpfs per Constitution §V — it builds.
run: check-tmpfs
	cargo run -p cargonaut -- $(ARGS)

test: check-tmpfs
	cargo test --workspace --lib --tests

bench: check-tmpfs
	cargo bench --workspace

clippy: check-tmpfs
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

# ── Install / distribution ────────────────────────────────────────────────────
# `install` follows GNU conventions: PREFIX (default /usr/local), BINDIR, and a
# DESTDIR staging root so distro packagers can build into a fakeroot. Installs
# the optimized release binary (stripped). System prefixes need sudo:
#   sudo make install            # -> /usr/local/bin/cargonaut
#   make install PREFIX=$$HOME/.local   # user-local, no sudo
#   make install DESTDIR=/tmp/pkg PREFIX=/usr   # staged for packaging
install: build-release
	install -d "$(DESTDIR)$(BINDIR)"
	install -m755 target/release/cargonaut "$(DESTDIR)$(BINDIR)/cargonaut"
	@$(STRIP) "$(DESTDIR)$(BINDIR)/cargonaut" 2>/dev/null || true
	@echo "[install] cargonaut -> $(DESTDIR)$(BINDIR)/cargonaut"

uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/cargonaut"
	@echo "[uninstall] removed $(DESTDIR)$(BINDIR)/cargonaut"

# Release artifact: a stripped, fully-static binary + tarball under dist/.
dist: static
	@mkdir -p dist
	@cp target/$(MUSL_TARGET)/release/cargonaut dist/cargonaut
	@$(STRIP) dist/cargonaut 2>/dev/null || true
	@tar -C dist -czf "dist/cargonaut-$(CARGONAUT_VERSION)-$(MUSL_TARGET).tar.gz" cargonaut
	@echo "[dist] dist/cargonaut-$(CARGONAUT_VERSION)-$(MUSL_TARGET).tar.gz  ($$(du -h dist/cargonaut | cut -f1) static binary)"

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

# Guard invoked as a prereq by all heavy build targets (build / test /
# bench / clippy). Errors loudly when target/ is a real on-SSD directory.
# Bypassed by CI=true or CARGONAUT_ALLOW_SSD_TARGET=1. See Constitution §V.
check-tmpfs:
	@bash scripts/check-tmpfs.sh

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
	@rm -rf dist/

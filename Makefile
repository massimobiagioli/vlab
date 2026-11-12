# Developer tasks for VLab. Run `make` or `make help` to list them.
#
# `make run` keeps settings and the database in `.data/` inside the project
# (ignored by git) through the VLAB_CONFIG_DIR and VLAB_DATA_DIR overrides
# (ADR-0014). The installed binary still defaults to per-user directories.

SHELL := /bin/sh
.DEFAULT_GOAL := help

# rustup installs cargo in ~/.cargo/bin; use it when cargo is not on PATH.
# PATH is exported too, so cargo finds its subcommands (cargo-audit, ...).
export PATH := $(HOME)/.cargo/bin:$(PATH)
CARGO ?= $(shell command -v cargo 2>/dev/null || echo "$(HOME)/.cargo/bin/cargo")
CARGO_AUDIT_VERSION := 0.23.0
CARGO_DENY_VERSION := 0.20.2

DATA_DIR := $(CURDIR)/.data
DEV_ENV := VLAB_CONFIG_DIR="$(DATA_DIR)" VLAB_DATA_DIR="$(DATA_DIR)"

# Arguments for `make run`, for example: make run ARGS="--list-labs"
ARGS ?= --help

.PHONY: help build-debug build-release run test lint fmt fmt-check clippy \
	audit deny check tools snapshot clean clean-data

help: ## Show this help
	@echo "Usage: make <target> [ARGS=\"<vlab arguments>\"]"
	@echo
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build-debug: ## Build the debug binary (target/debug/vlab)
	$(CARGO) build --locked

build-release: ## Build the optimized binary (target/release/vlab)
	$(CARGO) build --locked --release

run: build-debug ## Run vlab with data in .data/ (ARGS="..." for arguments)
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab $(ARGS)

test: ## Run all tests
	$(CARGO) test --locked

lint: fmt-check clippy ## Check formatting and run clippy

fmt: ## Format the code
	$(CARGO) fmt --all

fmt-check: ## Check formatting without changing files
	$(CARGO) fmt --all -- --check

clippy: ## Run clippy with warnings as errors
	$(CARGO) clippy --locked --all-targets -- -D warnings

audit: ## Check dependencies against RustSec advisories
	$(CARGO) audit

deny: ## Check advisories, licenses, bans, and sources
	$(CARGO) deny check

check: fmt-check clippy test audit deny ## Run every quality gate (AGENTS.md section 7)

tools: ## Install cargo-audit and cargo-deny at the pinned versions
	$(CARGO) install --locked cargo-audit --version $(CARGO_AUDIT_VERSION)
	$(CARGO) install --locked cargo-deny --version $(CARGO_DENY_VERSION)

snapshot: build-debug ## Regenerate the reviewed help snapshot
	./target/debug/vlab --help > tests/snapshots/help.txt

clean: ## Remove build output
	$(CARGO) clean

clean-data: ## Delete the development data in .data/, keeping the example
	@find "$(DATA_DIR)" -mindepth 1 ! -name settings.example.yaml -exec rm -rf {} + 2>/dev/null || true

# Developer tasks for VLab. Run `make` or `make help` to list them.
#
# `make run` keeps settings and the database in `.data/` inside the project
# (ignored by git) through the VLAB_CONFIG_DIR and VLAB_DATA_DIR overrides
# (ADR-0014). A direct debug build from this checkout selects `.data/`
# automatically (ADR-0031); release and installed binaries default to
# per-user directories.

SHELL := /bin/sh
.DEFAULT_GOAL := help

# rustup installs cargo in ~/.cargo/bin; use it when cargo is not on PATH.
# PATH is exported too, so cargo finds its subcommands (cargo-audit, ...).
export PATH := $(HOME)/.cargo/bin:$(PATH)
CARGO ?= $(shell command -v cargo 2>/dev/null || echo "$(HOME)/.cargo/bin/cargo")
CARGO_AUDIT_VERSION := 0.23.0
CARGO_DENY_VERSION := 0.20.2

DATA_DIR := $(CURDIR)/.data
WORKSPACE_DIR := $(CURDIR)/.workspace
PRESET_FILE := $(CURDIR)/assets/agents.yaml
DEV_ENV := VLAB_CONFIG_DIR="$(DATA_DIR)" VLAB_DATA_DIR="$(DATA_DIR)"

# Arguments for `make run`, for example: make run ARGS="--list-labs"
ARGS ?= --help

.PHONY: help build-debug build-release run load-preset run-schedules run-queue import agent-create-from-file \
	test lint fmt fmt-check clippy audit deny check tools snapshot clean clean-data clean-workspace

help: ## Show this help
	@color=$$(if [ -t 1 ] && [ -z "$${NO_COLOR:-}" ]; then echo yes; else echo no; fi); \
	awk -v color="$$color" ' \
	BEGIN { \
		FS = ":.*## "; \
		groups[1] = "Build and run"; \
		groups[2] = "Data and imports"; \
		groups[3] = "Quality gates"; \
		groups[4] = "Maintenance"; \
	} \
	/^[a-z-]+:.*## / { \
		n++; target[n] = $$1; desc[$$1] = $$2; \
	} \
	function group(name) { \
		if (name == "help" || name == "build-debug" || name == "build-release" || name == "run") return 1; \
		if (name == "load-preset" || name == "run-schedules" || name == "run-queue" || name == "import" || name == "agent-create-from-file") return 2; \
		if (name == "test" || name == "lint" || name == "fmt" || name == "fmt-check" || name == "clippy" || name == "audit" || name == "deny" || name == "check") return 3; \
		if (name == "tools" || name == "snapshot" || name == "clean" || name == "clean-data" || name == "clean-workspace") return 4; \
		return 5; \
	} \
	END { \
		if (color == "yes") { esc = sprintf("%c", 27); bold = esc "[1m"; cyan = esc "[36m"; yellow = esc "[33m"; dim = esc "[2m"; reset = esc "[0m" } \
		else { bold = ""; cyan = ""; yellow = ""; dim = ""; reset = "" } \
		max = 0; \
		for (i = 1; i <= n; i++) if (length(target[i]) > max) max = length(target[i]); \
		printf "%sVLab developer tasks%s\n", bold, reset; \
		printf "%sUsage:%s make <target> [ARGS=\"<vlab arguments>\"]\n\n", bold, reset; \
		for (g = 1; g <= 4; g++) { \
			count = 0; \
			for (i = 1; i <= n; i++) if (group(target[i]) == g) count++; \
			if (count == 0) continue; \
			printf "%s%s%s\n", bold, groups[g], reset; \
			for (i = 1; i <= n; i++) if (group(target[i]) == g) { \
				pad = sprintf("%-" max "s", target[i]); \
				printf "  %s%s%s  %s\n", cyan, pad, reset, desc[target[i]]; \
			} \
			printf "\n"; \
		} \
		printf "%sExamples%s\n", bold, reset; \
		printf "  %smake%s                         show this help\n", cyan, reset; \
		printf "  %smake run%s ARGS=\"--list-labs\"   run a command with .data/ storage\n", cyan, reset; \
		printf "  %smake load-preset%s             load the fictional agent catalog\n", cyan, reset; \
		printf "  %smake run-queue%s               drain up to 8 queued TODO requests\n", cyan, reset; \
		printf "  %smake import%s SRC=notes/security.md INTO=lab/Payments\n", cyan, reset; \
		printf "  %smake agent-create-from-file%s SRC=agents/example.md\n", cyan, reset; \
		printf "  %smake check%s                   run every quality gate\n", cyan, reset; \
	}' $(MAKEFILE_LIST)

build-debug: ## Build the debug binary (target/debug/vlab)
	$(CARGO) build --locked

build-release: ## Build the optimized binary (target/release/vlab)
	$(CARGO) build --locked --release

run: build-debug ## Run vlab with data in .data/ (ARGS="..." for arguments)
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab $(ARGS)

load-preset: build-debug ## Load assets/agents.yaml into the .data/ database
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab --load-preset "$(PRESET_FILE)"

run-schedules: build-debug ## Run due schedules for the current project in .data/
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab --run-schedules

run-queue: build-debug ## Drain up to 8 queued TODO requests in .data/
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab --run-queue

import: build-debug ## Import a markdown document (SRC=file.md INTO=lab/<name>|project/<name>|agent [AGENT=...])
	@test -n "$(SRC)" || { echo "Usage: make import SRC=<file.md> INTO=<lab/name|project/name|agent> [AGENT=<name-or-id>]"; exit 2; }
	@test -n "$(INTO)" || { echo "Usage: make import SRC=<file.md> INTO=<lab/name|project/name|agent> [AGENT=<name-or-id>]"; exit 2; }
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab --import --src "$(SRC)" --into "$(INTO)" $(if $(AGENT),--agent "$(AGENT)",)

agent-create-from-file: build-debug ## Create an agent from a markdown frontmatter definition (SRC=file.md)
	@test -n "$(SRC)" || { echo "Usage: make agent-create-from-file SRC=<file.md>"; exit 2; }
	@mkdir -p "$(DATA_DIR)" && chmod 700 "$(DATA_DIR)"
	@$(DEV_ENV) ./target/debug/vlab --create-agent-from-file "$(SRC)"

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

clean-workspace: ## Delete cloned project checkouts in .workspace/
	rm -rf "$(WORKSPACE_DIR)"

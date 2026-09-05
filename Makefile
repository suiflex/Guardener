# The commands CI runs, runnable by hand in the same order.
#
# There is no build system here to speak of — cargo is the build system. This
# file exists so that "what do I run before I push" has one answer that cannot
# drift from .github/workflows/ci.yml, and so the two dry runs below are not
# something anyone has to reconstruct from the README.

CARGO ?= cargo
BIN := target/release/guardener

# A read-only token. Dry runs still read GitHub — they withhold the writes, not
# the reads — so they need one. `gh auth token` is the easy way to get it.
GUARDENER_TOKEN ?= $(shell gh auth token 2>/dev/null)
export GUARDENER_TOKEN

.DEFAULT_GOAL := help

.PHONY: help
help: ## Show this help
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) \
		| sort \
		| awk -F':.*## ' '{ printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2 }'

.PHONY: verify
verify: fmt lint test ## Everything CI checks, in CI's order

.PHONY: fmt
fmt: ## Check formatting
	$(CARGO) fmt --all -- --check

.PHONY: fix
fix: ## Rewrite formatting in place
	$(CARGO) fmt --all

.PHONY: lint
lint: ## Clippy, warnings treated as errors
	$(CARGO) clippy --all-targets -- -D warnings

.PHONY: test
test: ## Run the tests
	$(CARGO) test

.PHONY: build
build: ## Build the release binary
	$(CARGO) build --release --locked

# Prints what the daily sweep would write, and writes nothing. Reads every
# watched repository, so it is slow and entirely safe.
.PHONY: hygiene
hygiene: build ## Preview the hygiene sweep against the organization
	./$(BIN) hygiene --dry-run

# Adds --fix, still without writing: shows the branch, files and pull request
# each repository would get. The one to read before ever running it for real.
.PHONY: hygiene-fix
hygiene-fix: build ## Preview the hygiene sweep with its fixes
	./$(BIN) hygiene --fix --dry-run

# What hygiene deliberately cannot do. `--fix` only ever adds, so a change to a
# template never reaches a stub already installed; this shows which ones have
# fallen behind. Read-only. Writing needs the script directly, one named file at
# a time — see the header of scripts/sync-stubs.sh for why.
.PHONY: stubs
stubs: ## Show which installed stub workflows have drifted from templates/
	./scripts/sync-stubs.sh

# Runs the gate against a checkout you already have, as the bot would see it.
#   make gate ROOT=../ForgeGuard REPO=suiflex/ForgeGuard PR=66 BASE=origin/main
ROOT ?= .
REPO ?= suiflex/Guardener
PR ?= 1
BASE ?= origin/main

.PHONY: gate
gate: build ## Preview the gate for a checkout (ROOT, REPO, PR, BASE)
	./$(BIN) check \
		--root "$(ROOT)" \
		--repo "$(REPO)" \
		--pr "$(PR)" \
		--head-sha "$$(git -C "$(ROOT)" rev-parse HEAD)" \
		--base "$(BASE)" \
		--dry-run

.PHONY: clean
clean: ## Remove build output
	$(CARGO) clean

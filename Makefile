# ==============================================================================
# clicleaner - Makefile
# Fast CLI utility to scan and clean node_modules and .NET/NuGet bin, obj, packages
# ==============================================================================

# Project detection: supports running from crate root or parent repository root
ifeq ($(wildcard Cargo.toml),)
  ifeq ($(wildcard clicleaner/Cargo.toml),clicleaner/Cargo.toml)
    CRATE_DIR      := clicleaner
    CARGO_MANIFEST := --manifest-path clicleaner/Cargo.toml
  else
    CRATE_DIR      := .
    CARGO_MANIFEST :=
  endif
else
  CRATE_DIR        := .
  CARGO_MANIFEST   :=
endif

# Variables
BINARY_NAME      := clicleaner
CARGO            := cargo
CARGO_FLAGS      ?=
VERSION          ?= $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(CRATE_DIR)/Cargo.toml | head -n 1)
OS               ?= $(shell uname -s | tr '[:upper:]' '[:lower:]')
ARCH             ?= $(shell uname -m)

# Paths
TARGET_DIR       := $(CRATE_DIR)/target
RELEASE_BIN      := $(TARGET_DIR)/release/$(BINARY_NAME)
DEBUG_BIN        := $(TARGET_DIR)/debug/$(BINARY_NAME)
DIST_DIR         := dist
PACKAGE_NAME     := $(BINARY_NAME)-v$(VERSION)-$(OS)-$(ARCH)
PACKAGE_ARCHIVE  := $(DIST_DIR)/$(PACKAGE_NAME).tar.gz
CHECKSUM_FILE    := $(DIST_DIR)/checksums.sha256

# Installation paths
PREFIX           ?= /usr/local
BINDIR           ?= $(PREFIX)/bin
DESTDIR          ?=

# Runtime configuration defaults (override on command line: e.g. make run PATH_TO_SCAN=~/projects)
PATH_TO_SCAN     ?= .
AGE              ?= 30d
ARGS             ?=

# Checksum command discovery (shasum on macOS, sha256sum on Linux)
SHA256_CMD       ?= $(shell command -v shasum >/dev/null 2>&1 && echo "shasum -a 256" || echo "sha256sum")

# ANSI color codes for terminal display
COLOR_RESET      := \033[0m
COLOR_BOLD       := \033[1m
COLOR_CYAN       := \033[36m
COLOR_GREEN      := \033[32m
COLOR_YELLOW     := \033[33m
COLOR_MAGENTA    := \033[35m

.DEFAULT_GOAL    := help

# ==============================================================================
# HELP & DOCUMENTATION
# ==============================================================================

.PHONY: help
help: ## Display this help menu with all available targets and descriptions
	@printf "$(COLOR_BOLD)clicleaner v%s - Available Make Targets$(COLOR_RESET)\n\n" "$(VERSION)"
	@printf "$(COLOR_YELLOW)Usage:$(COLOR_RESET) make $(COLOR_CYAN)<target>$(COLOR_RESET) [VARIABLE=value]\n\n"
	@printf "$(COLOR_BOLD)Variables that can be overridden:$(COLOR_RESET)\n"
	@printf "  $(COLOR_GREEN)PATH_TO_SCAN$(COLOR_RESET)  Target directory to scan (default: $(COLOR_CYAN).$(COLOR_RESET))\n"
	@printf "  $(COLOR_GREEN)AGE$(COLOR_RESET)           Age filter threshold (default: $(COLOR_CYAN)30d$(COLOR_RESET))\n"
	@printf "  $(COLOR_GREEN)ARGS$(COLOR_RESET)          Arbitrary CLI flags passed to the binary (e.g. ARGS=\"--dry-run -t node\")\n"
	@printf "  $(COLOR_GREEN)PREFIX$(COLOR_RESET)        Install prefix directory (default: $(COLOR_CYAN)/usr/local$(COLOR_RESET))\n\n"
	@printf "$(COLOR_BOLD)Build Targets:$(COLOR_RESET)\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / { \
		if ($$1 ~ /^(build|check)/) printf "  $(COLOR_CYAN)%-22s$(COLOR_RESET) %s\n", $$1, $$2 \
	}' $(MAKEFILE_LIST)
	@printf "\n$(COLOR_BOLD)Run Command Examples:$(COLOR_RESET)\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / { \
		if ($$1 ~ /^run/) printf "  $(COLOR_CYAN)%-22s$(COLOR_RESET) %s\n", $$1, $$2 \
	}' $(MAKEFILE_LIST)
	@printf "\n$(COLOR_BOLD)Testing & Code Quality:$(COLOR_RESET)\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / { \
		if ($$1 ~ /^(test|lint|clippy|fmt)/) printf "  $(COLOR_CYAN)%-22s$(COLOR_RESET) %s\n", $$1, $$2 \
	}' $(MAKEFILE_LIST)
	@printf "\n$(COLOR_BOLD)Release Pipeline:$(COLOR_RESET)\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / { \
		if ($$1 ~ /^release/) printf "  $(COLOR_CYAN)%-22s$(COLOR_RESET) %s\n", $$1, $$2 \
	}' $(MAKEFILE_LIST)
	@printf "\n$(COLOR_BOLD)Installation & Maintenance:$(COLOR_RESET)\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / { \
		if ($$1 ~ /^(install|uninstall|clean)/) printf "  $(COLOR_CYAN)%-22s$(COLOR_RESET) %s\n", $$1, $$2 \
	}' $(MAKEFILE_LIST)

# ==============================================================================
# BUILD TARGETS
# ==============================================================================

.PHONY: build
build: build-release ## Build optimized release binary (alias for build-release)

.PHONY: build-release
build-release: ## Compile optimized release binary using cargo
	@printf "$(COLOR_BOLD)Building release binary...$(COLOR_RESET)\n"
	$(CARGO) build --release $(CARGO_MANIFEST) $(CARGO_FLAGS)
	@printf "$(COLOR_GREEN)✔ Release binary ready: $(RELEASE_BIN)$(COLOR_RESET)\n"

.PHONY: build-debug
build-debug: ## Compile unoptimized debug binary with debug symbols
	@printf "$(COLOR_BOLD)Building debug binary...$(COLOR_RESET)\n"
	$(CARGO) build $(CARGO_MANIFEST) $(CARGO_FLAGS)
	@printf "$(COLOR_GREEN)✔ Debug binary ready: $(DEBUG_BIN)$(COLOR_RESET)\n"

.PHONY: build-all
build-all: ## Build all targets including tests, examples, and benchmarks
	$(CARGO) build $(CARGO_MANIFEST) --all-targets $(CARGO_FLAGS)

.PHONY: check
check: ## Quickly check code for syntax and type errors without code generation
	$(CARGO) check $(CARGO_MANIFEST) --all-targets $(CARGO_FLAGS)

# ==============================================================================
# RUN COMMAND EXAMPLES
# ==============================================================================

.PHONY: run
run: ## Run clicleaner with custom flags (e.g. make run ARGS="--dry-run -t node")
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- $(ARGS) $(PATH_TO_SCAN)

.PHONY: run-help
run-help: ## Display the clicleaner CLI usage guide and available flags
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --help

.PHONY: run-version
run-version: ## Display clicleaner version information
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --version

.PHONY: run-dry-run
run-dry-run: ## Example: Dry-run scan on PATH_TO_SCAN without deleting any files
	@printf "$(COLOR_YELLOW)Running dry-run scan on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --dry-run $(PATH_TO_SCAN)

.PHONY: run-node
run-node: ## Example: Scan and preview only Node.js node_modules directories
	@printf "$(COLOR_YELLOW)Scanning only Node.js node_modules on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --target node --dry-run $(PATH_TO_SCAN)

.PHONY: run-nuget
run-nuget: ## Example: Scan and preview only .NET / NuGet (bin, obj, packages)
	@printf "$(COLOR_YELLOW)Scanning only .NET / NuGet targets on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --target nuget --dry-run $(PATH_TO_SCAN)

.PHONY: run-interactive
run-interactive: ## Example: Run interactive checklist prompt to pick which folders to clean
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --interactive $(PATH_TO_SCAN)

.PHONY: run-json
run-json: ## Example: Emit scan results as structured JSON (ideal for scripting/CI)
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --dry-run --json $(PATH_TO_SCAN)

.PHONY: run-older-than
run-older-than: ## Example: Dry-run filter by last modified age (e.g. make run-older-than AGE=14d)
	@printf "$(COLOR_YELLOW)Scanning for directories older than $(AGE) on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --dry-run --older-than $(AGE) $(PATH_TO_SCAN)

.PHONY: run-trash
run-trash: ## Example: Dry-run preview with OS Trash / Recycle Bin enabled
	@printf "$(COLOR_YELLOW)Scanning with OS Trash mode enabled on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --dry-run --trash $(PATH_TO_SCAN)

.PHONY: run-strict
run-strict: ## Example: Scan node_modules requiring sibling package.json or lockfile
	@printf "$(COLOR_YELLOW)Scanning node_modules in strict mode on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --strict --dry-run $(PATH_TO_SCAN)

.PHONY: run-clean
run-clean: ## Example: Automatically delete all matched targets without prompt (--yes)
	@printf "$(COLOR_YELLOW)Cleaning targets immediately on: $(PATH_TO_SCAN)$(COLOR_RESET)\n"
	$(CARGO) run $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --yes $(PATH_TO_SCAN)

# ==============================================================================
# TESTING & CODE QUALITY
# ==============================================================================

.PHONY: test
test: ## Run unittests and integration tests
	@printf "$(COLOR_BOLD)Running test suite...$(COLOR_RESET)\n"
	$(CARGO) test $(CARGO_MANIFEST) $(CARGO_FLAGS)

.PHONY: test-verbose
test-verbose: ## Run tests showing standard output and individual test names
	$(CARGO) test $(CARGO_MANIFEST) $(CARGO_FLAGS) -- --nocapture

.PHONY: lint clippy
lint: clippy ## Run clippy linter across all targets
clippy: ## Run clippy with strict compiler warnings
	@printf "$(COLOR_BOLD)Running cargo clippy...$(COLOR_RESET)\n"
	$(CARGO) clippy $(CARGO_MANIFEST) --all-targets --all-features $(CARGO_FLAGS) -- -D warnings

.PHONY: fmt
fmt: ## Automatically format all Rust source files with rustfmt
	@printf "$(COLOR_BOLD)Formatting codebase with rustfmt...$(COLOR_RESET)\n"
	$(CARGO) fmt --manifest-path $(CRATE_DIR)/Cargo.toml --all

.PHONY: fmt-check
fmt-check: ## Verify source code formatting conforms to rustfmt guidelines
	@printf "$(COLOR_BOLD)Checking code formatting...$(COLOR_RESET)\n"
	$(CARGO) fmt --manifest-path $(CRATE_DIR)/Cargo.toml --all -- --check

# ==============================================================================
# RELEASE PIPELINE
# ==============================================================================

.PHONY: release
release: release-check release-package release-checksums ## Run full release workflow: verify, build, package, and generate checksums
	@printf "\n$(COLOR_GREEN)$(COLOR_BOLD)==================================================$(COLOR_RESET)\n"
	@printf "$(COLOR_GREEN)$(COLOR_BOLD)✔ Release v$(VERSION) packaged successfully!$(COLOR_RESET)\n"
	@printf "  Archive:  $(PACKAGE_ARCHIVE)\n"
	@printf "  Checksum: $(CHECKSUM_FILE)\n"
	@printf "$(COLOR_GREEN)$(COLOR_BOLD)==================================================$(COLOR_RESET)\n"

.PHONY: release-check
release-check: fmt-check clippy test release-publish-dry-run ## Verify code health, run test suite, and check crates.io packaging
	@printf "$(COLOR_GREEN)✔ All release pre-checks passed!$(COLOR_RESET)\n"

.PHONY: release-package
release-package: build-release ## Package release binary, docs, and licenses into dist/ tarball
	@printf "$(COLOR_BOLD)Packaging release archive for $(OS)/$(ARCH)...$(COLOR_RESET)\n"
	@rm -rf $(DIST_DIR)/$(PACKAGE_NAME)
	@mkdir -p $(DIST_DIR)/$(PACKAGE_NAME)
	@cp $(RELEASE_BIN) $(DIST_DIR)/$(PACKAGE_NAME)/$(BINARY_NAME)
	@if command -v strip >/dev/null 2>&1; then \
		printf "  Stripping debug symbols from packaged binary...\n"; \
		strip $(DIST_DIR)/$(PACKAGE_NAME)/$(BINARY_NAME) 2>/dev/null || true; \
	fi
	@cp $(CRATE_DIR)/README.md $(CRATE_DIR)/DESIGN.md $(DIST_DIR)/$(PACKAGE_NAME)/
	@if [ -f $(CRATE_DIR)/LICENSE ]; then cp $(CRATE_DIR)/LICENSE $(DIST_DIR)/$(PACKAGE_NAME)/; fi
	@tar -czf $(PACKAGE_ARCHIVE) -C $(DIST_DIR) $(PACKAGE_NAME)
	@rm -rf $(DIST_DIR)/$(PACKAGE_NAME)
	@printf "$(COLOR_GREEN)✔ Created release archive: $(PACKAGE_ARCHIVE)$(COLOR_RESET)\n"

.PHONY: release-checksums
release-checksums: ## Generate SHA-256 checksums for packaged release artifacts
	@printf "$(COLOR_BOLD)Generating SHA-256 checksums...$(COLOR_RESET)\n"
	@mkdir -p $(DIST_DIR)
	@cd $(DIST_DIR) && $(SHA256_CMD) $(PACKAGE_NAME).tar.gz > checksums.sha256
	@printf "$(COLOR_GREEN)✔ Checksums written to $(CHECKSUM_FILE):$(COLOR_RESET)\n"
	@cat $(CHECKSUM_FILE)

.PHONY: release-publish-dry-run
release-publish-dry-run: ## Verify crates.io publishing prerequisites without uploading
	@printf "$(COLOR_BOLD)Testing cargo package for crates.io...$(COLOR_RESET)\n"
	$(CARGO) publish $(CARGO_MANIFEST) --dry-run --allow-dirty

.PHONY: release-publish
release-publish: release-check ## Publish package to crates.io
	@printf "$(COLOR_BOLD)Publishing clicleaner v$(VERSION) to crates.io...$(COLOR_RESET)\n"
	$(CARGO) publish $(CARGO_MANIFEST)

.PHONY: release-tag
release-tag: ## Create and push an annotated git release tag (e.g. v0.1.0)
	@printf "$(COLOR_BOLD)Tagging git release v$(VERSION)...$(COLOR_RESET)\n"
	git tag -a "v$(VERSION)" -m "Release v$(VERSION)"
	@printf "$(COLOR_GREEN)✔ Created tag v$(VERSION). Push using: git push origin v$(VERSION)$(COLOR_RESET)\n"

# ==============================================================================
# INSTALLATION & CLEANUP
# ==============================================================================

.PHONY: install
install: build-release ## Install release binary to $(PREFIX)/bin (e.g. /usr/local/bin)
	@printf "$(COLOR_BOLD)Installing $(BINARY_NAME) to $(DESTDIR)$(BINDIR)...$(COLOR_RESET)\n"
	install -d "$(DESTDIR)$(BINDIR)"
	install -m 755 "$(RELEASE_BIN)" "$(DESTDIR)$(BINDIR)/$(BINARY_NAME)"
	@printf "$(COLOR_GREEN)✔ Installed $(BINARY_NAME) to $(DESTDIR)$(BINDIR)/$(BINARY_NAME)$(COLOR_RESET)\n"

.PHONY: uninstall
uninstall: ## Remove installed binary from $(PREFIX)/bin
	@printf "$(COLOR_BOLD)Uninstalling $(BINARY_NAME) from $(DESTDIR)$(BINDIR)...$(COLOR_RESET)\n"
	rm -f "$(DESTDIR)$(BINDIR)/$(BINARY_NAME)"
	@printf "$(COLOR_GREEN)✔ Removed $(DESTDIR)$(BINDIR)/$(BINARY_NAME)$(COLOR_RESET)\n"

.PHONY: install-cargo
install-cargo: ## Install binary locally into ~/.cargo/bin using cargo install
	@printf "$(COLOR_BOLD)Installing locally via cargo install...$(COLOR_RESET)\n"
	$(CARGO) install --path $(CRATE_DIR)

.PHONY: clean-dist
clean-dist: ## Remove release artifacts in dist/ directory
	@rm -rf $(DIST_DIR)
	@printf "$(COLOR_GREEN)✔ Cleaned $(DIST_DIR)/ directory$(COLOR_RESET)\n"

.PHONY: clean
clean: clean-dist ## Remove build artifacts in target/ and dist/
	@printf "$(COLOR_BOLD)Cleaning target artifacts...$(COLOR_RESET)\n"
	$(CARGO) clean $(CARGO_MANIFEST)
	@printf "$(COLOR_GREEN)✔ Cleaned $(TARGET_DIR) and $(DIST_DIR)$(COLOR_RESET)\n"

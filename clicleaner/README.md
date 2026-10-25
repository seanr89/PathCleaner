# `clicleaner`

A blazing-fast, context-aware CLI utility written in **Rust** that scans a target directory tree for **Node.js (`node_modules`)** and **.NET / NuGet (`bin`, `obj`, `packages`)** directories, calculates reclaimable disk space in parallel, and removes them safely to free up space on your machine.

For the detailed system architecture, crate breakdown, and detection rules, see [DESIGN.md](./DESIGN.md).

---

## Features

- **Multi-Threaded Traversal & Subtree Pruning**: Uses [`jwalk`](https://crates.io/crates/jwalk) and [`rayon`](https://crates.io/crates/rayon) to walk directories across all CPU cores and immediately stops descending inside matched `node_modules` or build folders, avoiding 95%+ of redundant filesystem syscalls.
- **Context-Aware Safety Checks**:
  - **`node_modules`**: Automatically prunes nested `node_modules` and supports `--strict` mode to require a sibling `package.json` or lockfile.
  - **`.NET / NuGet` (`bin`, `obj`, `packages`)**: Verifies parent/sibling project markers (`.csproj`, `.fsproj`, `.vbproj`, `.sln`, `packages.config`, or `.nupkg` artifacts) so generic `./bin` script folders or JavaScript monorepo `packages/` workspaces are **never** accidentally deleted.
  - **System Path Guard**: Refuses to scan root or operating system directories (`/`, `/System`, `/usr`, `C:\Windows`, etc.).
- **Flexible Cleanup Modes**:
  - **Interactive Multi-Select (Default in TTY)**: Toggle individual directories to delete or keep with a live terminal checklist.
  - **Dry-Run (`--dry-run` / `-d`)**: Preview a formatted table of cleanable directories and total reclaimable disk space without deleting anything.
  - **Automated Batch Mode (`--yes` / `-y`)**: Immediately purge all matched directories.
  - **OS Trash Support (`--trash`)**: Move directories to the macOS Trash / Windows Recycle Bin instead of permanent deletion.
  - **Age Filtering (`--older-than` / `-o`)**: Only target stale artifacts (e.g., `--older-than 30d` or `--older-than 2w`).

---

## Installation & Building

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (`cargo` and `rustc` 1.75+)

### Build from Source

```bash
# Clone or navigate to the project directory
cd clicleaner

# Build optimized release binary
cargo build --release

# (Optional) Install the binary globally to ~/.cargo/bin/clicleaner
cargo install --path .
```

---

## Usage

```bash
clicleaner [OPTIONS] [TARGET_PATH]
```

### Quick Examples

```bash
# 1. Interactive scan of a workspace (scans both Node.js and .NET/NuGet by default)
clicleaner ~/Documents/development

# 2. Dry-run preview to inspect reclaimable disk space without deleting anything
clicleaner ~/Documents/development --dry-run

# 3. Automatically delete all discovered node_modules and NuGet bin/obj/packages folders
clicleaner ~/Documents/development --yes

# 4. Clean ONLY Node.js node_modules folders
clicleaner ~/Documents/development --target node --yes

# 5. Clean ONLY .NET / NuGet (bin, obj, packages) folders and move them to the OS Trash
clicleaner ~/Documents/development --target nuget --trash --yes

# 6. Only clean directories not modified in the last 30 days
clicleaner ~/Documents/development --older-than 30d

# 7. Exclude specific glob patterns and emit JSON for scripting
clicleaner ~/Documents/development --exclude "**/archive/**" --dry-run --json
```

### CLI Options Reference

| Option / Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `[TARGET_PATH]` | — | `.` | Root directory path to scan for cleanable artifacts |
| `--target <TYPES>` | `-t` | `all` | Target ecosystems to scan: `all`, `node`, `nuget` (comma-separated) |
| `--include-obj <BOOL>` | — | `true` | Include .NET `obj/` directories alongside `bin/` and NuGet `packages/` |
| `--strict` | — | `false` | Require a sibling `package.json` or lockfile before flagging `node_modules` |
| `--dry-run` | `-d` | `false` | Scan and display reclaimable space without deleting anything |
| `--interactive` | `-i` | `false` | Force interactive multi-select prompt before deletion |
| `--yes` | `-y` | `false` | Skip confirmation prompts and delete all matched targets immediately |
| `--older-than <DURATION>` | `-o` | None | Only target directories not modified within duration (`30d`, `2w`, `6mo`, `12h`) |
| `--trash` | — | `false` | Move directories to OS Trash / Recycle Bin instead of permanent deletion |
| `--threads <N>` | `-j` | CPU cores | Number of worker threads for parallel scanning and deletion |
| `--exclude <GLOB>` | — | None | Exclude paths matching glob patterns (can be passed multiple times) |
| `--json` | — | `false` | Emit structured JSON output for scripting and CI/CD pipelines |
| `--help` | `-h` | — | Print help information |
| `--version` | `-V` | — | Print version information |

---

## Project Structure

```text
clicleaner/
├── Cargo.toml                  # Package manifest and dependencies
├── DESIGN.md                   # Architectural & technical design document
├── README.md                   # User documentation & usage guide
├── src/
│   ├── lib.rs                  # Library root exporting scanner, cleaner, CLI, and UI modules
│   ├── main.rs                 # CLI entrypoint & orchestration
│   ├── cli.rs                  # Clap argument definitions & duration parser
│   ├── cleaner.rs              # Parallel deletion engine (remove_dir_all / trash)
│   ├── error.rs                # Structured error types
│   ├── ui.rs                   # Terminal table formatting, relative timestamps & prompts
│   └── scanner/
│       ├── mod.rs              # Parallel directory walker (jwalk) & subtree pruning
│       ├── rules.rs            # Context validators for Node.js and .NET/NuGet
│       └── size.rs             # Parallel directory byte-size & mtime computation
└── tests/
    └── integration_tests.rs    # End-to-end CLI & safety integration tests
```

---

## Running Tests

```bash
cargo test
```

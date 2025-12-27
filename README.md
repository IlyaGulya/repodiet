# repodiet

A terminal-based Git repository history analyzer that helps you find and visualize storage bloat. Analyzes your repository's HEAD to show cumulative historical sizes vs current sizes, helping identify files that have grown over time or been deleted but still consume space in Git history.

![Rust](https://img.shields.io/badge/rust-2024-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

<!-- TODO: Add screenshot -->
<!-- ![Screenshot](docs/screenshot.png) -->

## Features

- **Interactive Tree View** - Navigate your repository structure and see cumulative vs current sizes
- **Deleted File Detection** - Find files removed from working tree but still consuming space in Git history
- **Large Blob Detective** - Identify the 50 largest blobs with authorship information
- **Extension Statistics** - View storage breakdown by file type
- **Full-text Search** - Search across all repository paths
- **Incremental Scanning** - SQLite cache for fast subsequent runs
- **Compressed Size Tracking** - Accurate on-disk sizes from Git pack files
- **Multi-language Keyboard** - Works with QWERTY and Russian ЙЦУКЕН layouts

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/IlyaGulya/repodiet.git
cd repodiet

# Build release binary
cargo build --release

# Binary will be at target/release/repodiet
```

### Pre-built Binaries

Download pre-built binaries from the [Releases](https://github.com/IlyaGulya/repodiet/releases) page.

Available platforms:
- Linux (x86_64, musl)
- macOS (x86_64, Apple Silicon)
- Windows (x86_64)

## Usage

```bash
# Analyze current directory
repodiet

# Analyze specific repository
repodiet /path/to/repo
```

### Keyboard Shortcuts

#### Navigation (All Views)

| Key | Action |
|-----|--------|
| `q` | Quit |
| `/` | Enter search mode |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |

#### Tree View

| Key | Action |
|-----|--------|
| `Enter` / `l` / `→` | Enter directory |
| `Backspace` / `h` / `←` | Go back |
| `d` | Toggle deleted-only filter |
| `t` | Switch to Extension view |
| `L` | Switch to Large Blobs view |
| `Esc` | Go back (or quit at root) |

#### Extension View

| Key | Action |
|-----|--------|
| `t` / `Esc` | Return to Tree view |
| `L` | Switch to Large Blobs view |

#### Large Blobs View

| Key | Action |
|-----|--------|
| `Enter` | Navigate to blob location in tree |
| `t` / `Esc` | Return to Tree view |

#### Search Mode

| Key | Action |
|-----|--------|
| *any character* | Add to search query |
| `Backspace` | Delete last character |
| `Enter` | Navigate to selected result |
| `Esc` | Exit search |

## Views

### Tree View (Default)

Shows repository structure with:
- **Cumulative size**: Total space consumed in Git history
- **Current size**: Space used by current version
- **Bloat indicator**: Percentage of historical overhead

Directories are sorted by cumulative size. Deleted files (current size = 0) are highlighted.

### Extension View

Aggregates statistics by file extension:
- Total count of files
- Cumulative size across all versions
- Current size in working tree

### Large Blobs View

Lists the 50 largest blobs with:
- File path
- Size
- Original author
- Commit date

### Search View

Full-text search across all paths in repository history.

## Building

### Prerequisites

- Rust 1.85+ (install via [rustup](https://rustup.rs/))
- Git (for cloning)

Dependencies are automatically handled by Cargo:
- `ratatui` - TUI framework
- `crossterm` - Terminal handling
- `git2` - Git repository access
- `sqlx` - SQLite database
- `tokio` - Async runtime

### Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench

# Run with specific repo
cargo run --release -- /path/to/repo
```

## Testing

```bash
# Run all tests (unit + integration)
cargo test

# Run specific test file
cargo test --test database_tests

# Run with output
cargo test -- --nocapture
```

### Test Coverage

- **16 unit tests** - Model, ViewModel, utilities
- **22 integration tests** - Database, Git scanner, tree loading, E2E

## Benchmarks

Performance benchmarks using [Criterion](https://github.com/bheisler/criterion.rs):

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench tree_bench

# Compare against baseline
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

### Benchmark Groups

| Group | Description | Sizes |
|-------|-------------|-------|
| `tree_add_path` | Tree construction | 1K, 10K, 50K paths |
| `tree_compute_totals` | Size aggregation | 1K, 10K, 50K nodes |
| `db_save_blobs` | Database inserts | 1K, 10K, 50K blobs |
| `db_load_tree` | Tree loading | 1K, 10K, 50K paths |
| `search_add_char` | Incremental search | 1K, 10K, 50K files |
| `scanner_*` | Git scanning | 50-200 commits |

## Architecture

```
src/
├── main.rs              # Entry point, event loop
├── model/               # Data structures
│   ├── tree.rs          # TreeNode - file/directory stats
│   ├── blob.rs          # LargeBlobInfo, ExtensionStats
│   └── search.rs        # SearchResult
├── repository/          # Data layer
│   ├── database.rs      # SQLite operations
│   └── git_scanner.rs   # Git history scanning
├── viewmodel/           # Business logic (MVVM)
│   ├── app_viewmodel.rs # Main coordinator
│   ├── tree_viewmodel.rs
│   ├── search_viewmodel.rs
│   ├── blobs_viewmodel.rs
│   └── extension_viewmodel.rs
├── view/                # TUI rendering
│   ├── tree_view.rs
│   ├── search_view.rs
│   ├── blobs_view.rs
│   └── extension_view.rs
├── input/               # Keyboard handling
│   └── keyboard.rs      # Layout-independent mapping
└── util/
    └── format.rs        # Size/date formatting
```

### Data Flow

1. **Scan**: Walk Git history, extract file paths and blob sizes
2. **Store**: Cache results in SQLite database
3. **Build**: Construct hierarchical TreeNode structure
4. **Display**: Render current view in TUI
5. **Interact**: Handle keyboard input, update state
6. **Repeat**: Re-render on state changes

## How It Works

### Compressed Size Tracking

Unlike `git ls-files`, repodiet reads actual on-disk sizes from Git pack files using `gix-pack`. This gives accurate storage measurements accounting for Git's delta compression.

### Incremental Scanning

The SQLite database caches:
- Scanned commit OIDs
- Blob metadata (size, path, author)
- Current HEAD reference

On subsequent runs, only new commits since the last scan are processed.

### Deleted File Detection

Files are marked as "deleted" when:
- They exist in Git history (cumulative_size > 0)
- They don't exist in current HEAD (current_size = 0)

This helps identify legacy files still consuming space.

## Current Limitations

- **HEAD only** - Currently analyzes only the HEAD commit; cannot switch branches/tags/commits
- **Working tree required** - Does not support bare Git repositories
- **Read-only** - Cannot clean up or remove bloat from the repository

## Roadmap

Planned features for future releases:

- [ ] **Branch/tag/commit selection** - Analyze any ref, not just HEAD
- [ ] **Full repository bloat** - Show total repo size including all branches and unreachable objects
- [ ] **Bare repository support** - Analyze bare Git repositories without working tree
- [ ] **Cleanup commands** - Built-in commands to rewrite history and remove bloat
- [ ] **Export reports** - Generate CSV/JSON/PNG reports for CI integration and visualization
- [ ] **Fuzzy search** - Better search with fuzzy matching

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Run formatter: `cargo fmt`
6. Run linter: `cargo clippy`
7. Submit a pull request

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [git2](https://github.com/rust-lang/git2-rs) - Git bindings
- [gitoxide](https://github.com/Byron/gitoxide) - Pack file handling

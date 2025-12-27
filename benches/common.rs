// Shared benchmark helpers
// Functions here are used across different benchmark files
#![allow(dead_code)]

use repodiet::model::TreeNode;
use repodiet::repository::Database;
use git2::{Repository, Signature};
use std::path::PathBuf;
use tempfile::TempDir;

/// Generate a tree with N paths for benchmarking
pub fn generate_tree(num_paths: usize) -> TreeNode {
    let mut root = TreeNode::new("(root)");
    let dirs = ["src", "lib", "test", "pkg", "mod"];

    for i in 0..num_paths {
        let depth = (i % 5) + 1;
        let mut path_parts: Vec<String> = (0..depth)
            .map(|d| format!("{}_{}", dirs[d], i / 1000))
            .collect();
        path_parts.push(format!("file_{}.rs", i));

        let path_refs: Vec<&str> = path_parts.iter().map(|s| s.as_str()).collect();
        root.add_path_with_sizes(&path_refs, (i * 100) as u64, (i * 50) as u64, 1);
    }
    root.compute_totals();
    root
}

/// Generate a tree with some deleted files for benchmarking deletion detection
pub fn generate_tree_with_deletions(num_paths: usize, deletion_ratio: f64) -> TreeNode {
    let mut root = TreeNode::new("(root)");
    let dirs = ["src", "lib", "test", "pkg", "mod"];

    for i in 0..num_paths {
        let depth = (i % 5) + 1;
        let mut path_parts: Vec<String> = (0..depth)
            .map(|d| format!("{}_{}", dirs[d], i / 1000))
            .collect();
        path_parts.push(format!("file_{}.rs", i));

        let path_refs: Vec<&str> = path_parts.iter().map(|s| s.as_str()).collect();

        // Mark some files as deleted (current_size = 0)
        let current_size = if (i as f64 / num_paths as f64) < deletion_ratio {
            0
        } else {
            (i * 50) as u64
        };

        root.add_path_with_sizes(&path_refs, (i * 100) as u64, current_size, 1);
    }
    root.compute_totals();
    root
}

/// Generate blob data for database benchmarks
pub fn generate_blobs(num_blobs: usize) -> Vec<(String, String, i64, i64)> {
    (0..num_blobs)
        .map(|i| (
            format!("oid_{:08x}", i),
            format!("src/dir_{}/file_{}.rs", i % 100, i),
            (i * 100) as i64,
            (i * 50) as i64,
        ))
        .collect()
}

/// Generate blob metadata for database benchmarks
pub fn generate_blob_metadata(num_blobs: usize) -> Vec<(String, i64, String, String, i64)> {
    (0..num_blobs)
        .map(|i| (
            format!("oid_{:08x}", i),
            (i * 1000) as i64, // size
            format!("src/dir_{}/file_{}.rs", i % 100, i),
            format!("author_{}", i % 10),
            1700000000 + (i as i64), // timestamp
        ))
        .collect()
}

/// Create in-memory database for benchmarks
pub async fn setup_bench_db() -> Database {
    let db = Database::new(":memory:").await.unwrap();
    db.init_schema().await.unwrap();
    db
}

/// Create a temporary git repository for benchmarks
pub fn create_bench_repo() -> (TempDir, PathBuf, Repository) {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path().to_path_buf();
    let repo = Repository::init(&repo_path).unwrap();

    // Configure git user for commits
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Bench User").unwrap();
    config.set_str("user.email", "bench@example.com").unwrap();

    (dir, repo_path, repo)
}

/// Add files and create a commit
pub fn add_commit(repo: &Repository, files: &[(&str, &[u8])], message: &str) -> git2::Oid {
    let sig = Signature::now("Bench User", "bench@example.com").unwrap();
    let mut index = repo.index().unwrap();

    for (path, content) in files {
        let full_path = repo.workdir().unwrap().join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full_path, content).unwrap();
        index.add_path(std::path::Path::new(path)).unwrap();
    }

    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    if let Some(parent) = parent {
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent]).unwrap()
    } else {
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[]).unwrap()
    }
}

/// Generate files for benchmark repository
pub fn generate_files(num_files: usize) -> Vec<(String, Vec<u8>)> {
    (0..num_files)
        .map(|i| {
            let path = format!("src/dir_{}/file_{}.rs", i % 50, i);
            let content = format!("// File {}\nfn func_{}() {{}}\n", i, i).into_bytes();
            (path, content)
        })
        .collect()
}

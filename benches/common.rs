// Shared benchmark helpers
// Functions here are used across different benchmark files
#![allow(dead_code)]

use criterion::async_executor::AsyncExecutor;
use repodiet::model::TreeNode;
use std::sync::Arc;
use repodiet::repository::{BlobMetaRecord, BlobRecord, Database};
use git2::{Repository, Signature};
use std::borrow::Cow;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Async executor for Criterion benchmarks
pub struct TokioExecutor(pub Runtime);

impl AsyncExecutor for TokioExecutor {
    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}

/// Create a new TokioExecutor
pub fn tokio_executor() -> TokioExecutor {
    TokioExecutor(Runtime::new().unwrap())
}

/// Generate path parts for a given index
fn path_parts(i: usize) -> Vec<String> {
    const DIRS: [&str; 5] = ["src", "lib", "test", "pkg", "mod"];
    let depth = (i % 5) + 1;

    let mut parts: Vec<String> = (0..depth)
        .map(|d| format!("{}_{}", DIRS[d], i / 1000))
        .collect();

    parts.push(format!("file_{}.rs", i));
    parts
}

/// Generate a tree with configurable current_size calculation
fn generate_tree_generic(num_paths: usize, current_size: impl Fn(usize) -> u64) -> TreeNode {
    let mut root = TreeNode::new("(root)");
    for i in 0..num_paths {
        let parts = path_parts(i);
        let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        root.add_path_with_sizes(&refs, (i * 100) as u64, current_size(i), 1);
    }
    root.compute_totals();
    root
}

/// Generate a tree with N paths for benchmarking
pub fn generate_tree(num_paths: usize) -> TreeNode {
    generate_tree_generic(num_paths, |i| (i * 50) as u64)
}

/// Generate a tree wrapped in Arc for search benchmarks
pub fn generate_tree_arc(num_paths: usize) -> Arc<TreeNode> {
    Arc::new(generate_tree(num_paths))
}

/// Generate a tree with some deleted files for benchmarking deletion detection
pub fn generate_tree_with_deletions(num_paths: usize, deletion_ratio: f64) -> TreeNode {
    generate_tree_generic(num_paths, |i| {
        if (i as f64 / num_paths as f64) < deletion_ratio {
            0
        } else {
            (i * 50) as u64
        }
    })
}

/// Generate a deterministic 20-byte OID from an index
pub fn make_oid(i: usize) -> [u8; 20] {
    let mut oid = [0u8; 20];
    let bytes = i.to_le_bytes();
    oid[..bytes.len()].copy_from_slice(&bytes);
    oid
}

/// Generate commit OIDs for database benchmarks
pub fn generate_commit_oids(num_commits: usize) -> Vec<[u8; 20]> {
    (0..num_commits).map(make_oid).collect()
}

/// Generate blob data for database benchmarks
pub fn generate_blobs(num_blobs: usize) -> Vec<BlobRecord<'static>> {
    (0..num_blobs)
        .map(|i| BlobRecord {
            oid: make_oid(i),
            path: Cow::Owned(format!("src/dir_{}/file_{}.rs", i % 100, i)),
            cumulative_size: (i * 100) as i64,
            current_size: (i * 50) as i64,
        })
        .collect()
}

/// Generate blob metadata for database benchmarks
pub fn generate_blob_metadata(num_blobs: usize) -> Vec<BlobMetaRecord<'static>> {
    (0..num_blobs)
        .map(|i| BlobMetaRecord {
            oid: make_oid(i),
            size: (i * 1000) as i64,
            path: Cow::Owned(format!("src/dir_{}/file_{}.rs", i % 100, i)),
            author: Cow::Owned(format!("author_{}", i % 10)),
            timestamp: 1700000000 + (i as i64),
        })
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

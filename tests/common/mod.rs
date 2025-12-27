// Shared test fixtures for integration tests
// Functions here are used across different test files
#![allow(dead_code)]

use git2::{Repository, Signature};
use repodiet::repository::Database;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create an in-memory test database
pub async fn create_test_db() -> Database {
    Database::new(":memory:").await.unwrap()
}

/// Create a temporary git repository with initial commit
pub fn create_test_repo() -> (TempDir, PathBuf, Repository) {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path().to_path_buf();
    let repo = Repository::init(&repo_path).unwrap();

    // Configure git user for commits
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test User").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();

    (dir, repo_path, repo)
}

/// Add a file to the repository and create a commit
pub fn add_commit(
    repo: &Repository,
    files: &[(&str, &[u8])],
    message: &str,
) -> git2::Oid {
    let sig = Signature::now("Test User", "test@example.com").unwrap();

    let mut index = repo.index().unwrap();

    for (path, content) in files {
        // Write file to working directory
        let full_path = repo.workdir().unwrap().join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full_path, content).unwrap();

        // Add to index
        index.add_path(std::path::Path::new(path)).unwrap();
    }

    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    // Get parent commit if exists
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    let commit_id = if let Some(parent) = parent {
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[&parent],
        ).unwrap()
    } else {
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[],
        ).unwrap()
    };

    commit_id
}

/// Remove a file from the repository and create a commit
pub fn remove_file_commit(
    repo: &Repository,
    path: &str,
    message: &str,
) -> git2::Oid {
    let sig = Signature::now("Test User", "test@example.com").unwrap();

    // Remove from working directory
    let full_path = repo.workdir().unwrap().join(path);
    if full_path.exists() {
        std::fs::remove_file(&full_path).unwrap();
    }

    // Remove from index
    let mut index = repo.index().unwrap();
    index.remove_path(std::path::Path::new(path)).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parent = repo.head().unwrap().peel_to_commit().unwrap();

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        message,
        &tree,
        &[&parent],
    ).unwrap()
}

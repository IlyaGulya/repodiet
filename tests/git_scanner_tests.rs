// Git Scanner integration tests
// Tests git scanning against real (temporary) git repositories

mod common;

use repodiet::repository::{Database, GitScanner};
use tempfile::TempDir;

/// Create a test database in a temp directory
async fn create_db_in_dir(dir: &TempDir) -> Database {
    let db_path = dir.path().join("test.db");
    let db = Database::new(db_path.to_str().unwrap()).await.unwrap();
    db.init_schema().await.unwrap();
    db
}

#[tokio::test]
async fn test_scan_empty_repo() {
    let (dir, repo_path, _repo) = common::create_test_repo();

    // Create initial commit with empty tree
    let repo = git2::Repository::open(&repo_path).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[]).unwrap();

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    let tree = scanner.scan(&db).await.unwrap();

    // Root should exist but have no children (empty tree)
    assert_eq!(tree.name, "(root)");
    assert!(tree.children.is_empty() || tree.cumulative_size == 0);
}

#[tokio::test]
async fn test_scan_single_commit() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // Add a file
    common::add_commit(
        &repo,
        &[("hello.txt", b"Hello, World!")],
        "Add hello.txt",
    );

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    let tree = scanner.scan(&db).await.unwrap();

    // Should have hello.txt
    assert!(tree.children.contains_key("hello.txt"));
    let hello = tree.children.get("hello.txt").unwrap();
    assert!(hello.cumulative_size > 0);
    assert!(hello.current_size > 0); // File exists in HEAD
}

#[tokio::test]
async fn test_scan_multiple_commits() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // First commit
    common::add_commit(&repo, &[("file.txt", b"version 1")], "v1");

    // Second commit - modify file
    common::add_commit(&repo, &[("file.txt", b"version 2, longer content")], "v2");

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    let tree = scanner.scan(&db).await.unwrap();

    // File should exist
    assert!(tree.children.contains_key("file.txt"));
    let file = tree.children.get("file.txt").unwrap();

    // Should have cumulative size from both versions
    assert!(file.cumulative_size > 0);
    // Current size should reflect HEAD version
    assert!(file.current_size > 0);
    // Blob count should be 2 (two different versions)
    assert_eq!(file.blob_count, 2);
}

#[tokio::test]
async fn test_deleted_file_detection() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // Add a file
    common::add_commit(&repo, &[("to_delete.txt", b"This will be deleted")], "Add file");

    // Delete the file
    common::remove_file_commit(&repo, "to_delete.txt", "Delete file");

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    let tree = scanner.scan(&db).await.unwrap();

    // File should still appear in tree (was in history)
    assert!(tree.children.contains_key("to_delete.txt"));
    let file = tree.children.get("to_delete.txt").unwrap();

    // Cumulative size > 0 (was in history)
    assert!(file.cumulative_size > 0);
    // Current size = 0 (deleted from HEAD)
    assert_eq!(file.current_size, 0);
}

#[tokio::test]
async fn test_incremental_scan() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // First commit
    common::add_commit(&repo, &[("file1.txt", b"content1")], "First");

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    // First scan
    let tree1 = scanner.scan(&db).await.unwrap();
    assert!(tree1.children.contains_key("file1.txt"));

    // Add another commit
    common::add_commit(&repo, &[("file2.txt", b"content2")], "Second");

    // Second scan should be incremental (only new commit)
    let tree2 = scanner.scan(&db).await.unwrap();
    assert!(tree2.children.contains_key("file1.txt"));
    assert!(tree2.children.contains_key("file2.txt"));
}

#[tokio::test]
async fn test_head_caching() {
    let (dir, repo_path, repo) = common::create_test_repo();

    common::add_commit(&repo, &[("file.txt", b"content")], "Initial");

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    // First scan
    scanner.scan(&db).await.unwrap();

    // HEAD OID should be cached
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let cached_head = db.get_metadata("head_oid").await;
    assert_eq!(cached_head.as_deref(), Some(head.id().to_string().as_str()));

    // Second scan with same HEAD should use cache (fast path)
    // This is hard to verify directly, but at least ensure it works
    let tree = scanner.scan(&db).await.unwrap();
    assert!(tree.children.contains_key("file.txt"));
}

#[tokio::test]
async fn test_large_blob_metadata() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // Add a file with known content
    common::add_commit(&repo, &[("large.bin", &[0u8; 1000])], "Add large file");

    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());

    scanner.scan(&db).await.unwrap();

    // Get top blobs
    let blobs = db.get_top_blobs(10).await.unwrap();

    // Should have at least our large file
    assert!(!blobs.is_empty());

    // Find our blob
    let large = blobs.iter().find(|b| b.path == "large.bin");
    assert!(large.is_some());
    let large = large.unwrap();

    // Verify metadata
    assert!(large.size > 0);
    assert_eq!(large.first_author, "Test User");
    assert!(large.first_date > 0);
}

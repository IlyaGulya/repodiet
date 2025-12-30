// End-to-End Pipeline tests
// Full pipeline from git repo to ViewModel

mod common;

use std::sync::Arc;
use repodiet::repository::{Database, GitScanner};
use repodiet::viewmodel::{TreeViewModel, SearchViewModel, BlobsViewModel};
use tempfile::TempDir;

/// Create a test database in a temp directory
async fn create_db_in_dir(dir: &TempDir) -> Database {
    let db_path = dir.path().join("test.db");
    let db = Database::new(db_path.to_str().unwrap()).await.unwrap();
    db.init_schema().await.unwrap();
    db
}

#[tokio::test]
async fn test_full_scan_to_viewmodel() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // Add some files
    common::add_commit(&repo, &[
        ("src/main.rs", b"fn main() {}"),
        ("src/lib.rs", b"pub mod utils;"),
        ("README.md", b"# Project"),
    ], "Initial commit");

    // Scan
    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
    let tree = scanner.scan(&db).await.unwrap();

    // Create TreeViewModel
    let vm = TreeViewModel::new(Arc::new(tree));

    // Verify initial state
    assert!(vm.is_at_root());
    assert_eq!(vm.current_path(), "/");

    // Verify visible children
    let children = vm.visible_children();
    assert!(!children.is_empty());

    // Should have src and README.md
    let names: Vec<_> = children.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"src"));
    assert!(names.contains(&"README.md"));
}

#[tokio::test]
async fn test_search_across_scanned_data() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // Add files with recognizable names
    common::add_commit(&repo, &[
        ("src/main.rs", b"fn main() {}"),
        ("src/lib.rs", b"pub mod utils;"),
        ("src/utils.rs", b"pub fn helper() {}"),
        ("tests/test_main.rs", b"#[test]"),
        ("README.md", b"# Project"),
    ], "Initial commit");

    // Scan
    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
    let tree = scanner.scan(&db).await.unwrap();

    // Create SearchViewModel
    let mut vm = SearchViewModel::new(Arc::new(tree));

    // Search for ".rs" files
    for c in ".rs".chars() {
        vm.add_char(c);
    }

    // Verify results
    let paths: Vec<_> = vm.results().map(|(p, _, _)| p).collect();
    assert!(!paths.is_empty());

    // Should find all .rs files
    assert!(paths.iter().any(|p| p.ends_with("main.rs")));
    assert!(paths.iter().any(|p| p.ends_with("lib.rs")));
    assert!(paths.iter().any(|p| p.ends_with("utils.rs")));
    assert!(paths.iter().any(|p| p.ends_with("test_main.rs")));

    // Should NOT find README.md
    assert!(!paths.iter().any(|p| p.ends_with("README.md")));

    // Test case-insensitive search - clear and search for readme
    vm.clear();
    for c in "readme".chars() {
        vm.add_char(c);
    }
    let paths: Vec<_> = vm.results().map(|(p, _, _)| p).collect();
    assert!(paths.iter().any(|p| p.contains("README")));
}

#[tokio::test]
async fn test_large_blobs_from_scan() {
    let (dir, repo_path, repo) = common::create_test_repo();

    // Add files of varying sizes
    common::add_commit(&repo, &[
        ("small.txt", b"tiny"),
        ("medium.txt", &[b'x'; 100]),
        ("large.bin", &[b'x'; 1000]),
    ], "Add files");

    // Scan
    let db = create_db_in_dir(&dir).await;
    let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
    let tree = scanner.scan(&db).await.unwrap();
    let total_cumulative = tree.cumulative_size;

    // Get large blobs
    let blobs = db.get_top_blobs(10).await.unwrap();

    // Create BlobsViewModel
    let vm = BlobsViewModel::new(blobs, total_cumulative);

    // Verify blobs are present
    let blobs = vm.blobs();
    assert!(!blobs.is_empty());

    // Blobs should be sorted by size (largest first)
    for i in 1..blobs.len() {
        assert!(blobs[i-1].size >= blobs[i].size);
    }

    // Total blob size should be calculated
    assert!(vm.total_blob_size() > 0);
}

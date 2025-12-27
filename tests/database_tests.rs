// Database integration tests
// Tests SQLite operations in isolation using in-memory database

mod common;

use repodiet::repository::{Database, SCHEMA_VERSION};

/// Helper to create test database with initialized schema
async fn setup_db() -> Database {
    let db = common::create_test_db().await;
    db.init_schema().await.unwrap();
    db
}

#[tokio::test]
async fn test_schema_init() {
    let db = common::create_test_db().await;

    // First init should return true (schema was rebuilt/created)
    let rebuilt = db.init_schema().await.unwrap();
    assert!(rebuilt, "First init_schema should return true");

    // Second init should return false (schema exists and version matches)
    let rebuilt = db.init_schema().await.unwrap();
    assert!(!rebuilt, "Second init_schema should return false");

    // Verify schema version is stored
    let version = db.get_metadata("schema_version").await;
    assert_eq!(version.as_deref(), Some(SCHEMA_VERSION));
}

#[tokio::test]
async fn test_schema_version_stored() {
    let db = setup_db().await;

    let version = db.get_metadata("schema_version").await;
    assert_eq!(version.as_deref(), Some(SCHEMA_VERSION));
}

#[tokio::test]
async fn test_metadata_roundtrip() {
    let db = setup_db().await;

    // Set metadata
    db.set_metadata("test_key", "test_value").await.unwrap();

    // Get metadata
    let value = db.get_metadata("test_key").await;
    assert_eq!(value.as_deref(), Some("test_value"));

    // Update metadata
    db.set_metadata("test_key", "updated_value").await.unwrap();
    let value = db.get_metadata("test_key").await;
    assert_eq!(value.as_deref(), Some("updated_value"));

    // Non-existent key returns None
    let value = db.get_metadata("nonexistent").await;
    assert!(value.is_none());
}

#[tokio::test]
async fn test_save_and_load_tree() {
    let db = setup_db().await;

    // Save some blobs (oid, path, cumulative_size, current_size)
    let blobs = vec![
        ("oid1".to_string(), "src/main.rs".to_string(), 1000i64, 500i64),
        ("oid2".to_string(), "src/lib.rs".to_string(), 800i64, 400i64),
        ("oid3".to_string(), "README.md".to_string(), 200i64, 200i64),
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    // Load tree
    let tree = db.load_tree().await.unwrap();

    // Verify structure
    assert_eq!(tree.name, "(root)");
    assert!(tree.children.contains_key("src"));
    assert!(tree.children.contains_key("README.md"));

    let src = tree.children.get("src").unwrap();
    assert!(src.children.contains_key("main.rs"));
    assert!(src.children.contains_key("lib.rs"));

    // Verify sizes
    let main_rs = src.children.get("main.rs").unwrap();
    assert_eq!(main_rs.cumulative_size, 1000);
    assert_eq!(main_rs.current_size, 500);
}

#[tokio::test]
async fn test_blob_conflict_handling() {
    let db = setup_db().await;

    // Save first blob for a path
    let blobs1 = vec![
        ("oid1".to_string(), "src/file.rs".to_string(), 100i64, 50i64),
    ];
    db.save_blobs(&blobs1, None).await.unwrap();

    // Save another blob for the same path (simulating new version)
    let blobs2 = vec![
        ("oid2".to_string(), "src/file.rs".to_string(), 150i64, 75i64),
    ];
    db.save_blobs(&blobs2, None).await.unwrap();

    // Load tree - cumulative should accumulate, current should update
    let tree = db.load_tree().await.unwrap();
    let src = tree.children.get("src").unwrap();
    let file = src.children.get("file.rs").unwrap();

    // Cumulative size should be sum of both
    assert_eq!(file.cumulative_size, 250); // 100 + 150
    // Current size should be sum (both contribute to current)
    assert_eq!(file.current_size, 125); // 50 + 75
    // Blob count should be 2
    assert_eq!(file.blob_count, 2);
}

#[tokio::test]
async fn test_top_blobs_sorted() {
    let db = setup_db().await;

    // Save blob metadata with different sizes
    let metadata = vec![
        ("oid1".to_string(), 100i64, "small.txt".to_string(), "author".to_string(), 1000i64),
        ("oid2".to_string(), 500i64, "medium.txt".to_string(), "author".to_string(), 1001i64),
        ("oid3".to_string(), 1000i64, "large.txt".to_string(), "author".to_string(), 1002i64),
        ("oid4".to_string(), 250i64, "small2.txt".to_string(), "author".to_string(), 1003i64),
    ];
    db.save_blob_metadata(&metadata, None).await.unwrap();

    // Get top 3 blobs
    let top = db.get_top_blobs(3).await.unwrap();

    assert_eq!(top.len(), 3);
    // Should be sorted by size descending
    assert_eq!(top[0].size, 1000);
    assert_eq!(top[0].path, "large.txt");
    assert_eq!(top[1].size, 500);
    assert_eq!(top[1].path, "medium.txt");
    assert_eq!(top[2].size, 250);
    assert_eq!(top[2].path, "small2.txt");
}

#[tokio::test]
async fn test_seen_blobs_tracking() {
    let db = setup_db().await;

    // Initially no seen blobs
    let seen = db.load_seen_blobs().await.unwrap();
    assert!(seen.is_empty());

    // Save blobs - they should be marked as seen
    let blobs = vec![
        ("oid1".to_string(), "file1.txt".to_string(), 100i64, 100i64),
        ("oid2".to_string(), "file2.txt".to_string(), 200i64, 200i64),
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    // Load seen blobs
    let seen = db.load_seen_blobs().await.unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen.contains("oid1"));
    assert!(seen.contains("oid2"));

    // Save same oid again - should not duplicate
    let blobs2 = vec![
        ("oid1".to_string(), "file1.txt".to_string(), 50i64, 50i64),
    ];
    db.save_blobs(&blobs2, None).await.unwrap();

    let seen = db.load_seen_blobs().await.unwrap();
    assert_eq!(seen.len(), 2); // Still 2, not 3
}

#[tokio::test]
async fn test_commit_scanned_tracking() {
    let db = setup_db().await;

    // Initially no commits scanned
    assert!(!db.is_commit_scanned("commit1").await);
    assert!(!db.is_commit_scanned("commit2").await);

    // Mark commits as scanned
    db.mark_commits_scanned(&["commit1".to_string(), "commit2".to_string()])
        .await
        .unwrap();

    // Now they should be marked
    assert!(db.is_commit_scanned("commit1").await);
    assert!(db.is_commit_scanned("commit2").await);
    assert!(!db.is_commit_scanned("commit3").await);

    // Mark same commit again - should not error
    db.mark_commits_scanned(&["commit1".to_string()])
        .await
        .unwrap();
    assert!(db.is_commit_scanned("commit1").await);
}

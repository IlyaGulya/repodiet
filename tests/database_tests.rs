// Database integration tests
// Tests SQLite operations in isolation using in-memory database

mod common;

use repodiet::repository::{BlobMetaRecord, BlobRecord, Database, SCHEMA_VERSION};

/// Helper to create a 20-byte OID from a test identifier
fn test_oid(id: u8) -> [u8; 20] {
    let mut oid = [0u8; 20];
    oid[0] = id;
    oid
}

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

    // Save some blobs
    let blobs = vec![
        BlobRecord::new(test_oid(1), "src/main.rs", 1000, 500),
        BlobRecord::new(test_oid(2), "src/lib.rs", 800, 400),
        BlobRecord::new(test_oid(3), "README.md", 200, 200),
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
        BlobRecord::new(test_oid(1), "src/file.rs", 100, 50),
    ];
    db.save_blobs(&blobs1, None).await.unwrap();

    // Save another blob for the same path (simulating new version)
    let blobs2 = vec![
        BlobRecord::new(test_oid(2), "src/file.rs", 150, 75),
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
        BlobMetaRecord::new(test_oid(1), 100, "small.txt", "author", 1000),
        BlobMetaRecord::new(test_oid(2), 500, "medium.txt", "author", 1001),
        BlobMetaRecord::new(test_oid(3), 1000, "large.txt", "author", 1002),
        BlobMetaRecord::new(test_oid(4), 250, "small2.txt", "author", 1003),
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
        BlobRecord::new(test_oid(1), "file1.txt", 100, 100),
        BlobRecord::new(test_oid(2), "file2.txt", 200, 200),
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    // Load seen blobs
    let seen = db.load_seen_blobs().await.unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen.contains(&test_oid(1)));
    assert!(seen.contains(&test_oid(2)));

    // Save same oid again - should not duplicate
    let blobs2 = vec![
        BlobRecord::new(test_oid(1), "file1.txt", 50, 50),
    ];
    db.save_blobs(&blobs2, None).await.unwrap();

    let seen = db.load_seen_blobs().await.unwrap();
    assert_eq!(seen.len(), 2); // Still 2, not 3
}

#[tokio::test]
async fn test_commit_scanned_tracking() {
    let db = setup_db().await;

    let commit1 = test_oid(1);
    let commit2 = test_oid(2);
    let commit3 = test_oid(3);

    // Initially no commits scanned
    assert!(!db.is_commit_scanned(&commit1).await);
    assert!(!db.is_commit_scanned(&commit2).await);

    // Mark commits as scanned
    db.mark_commits_scanned(&[commit1, commit2])
        .await
        .unwrap();

    // Now they should be marked
    assert!(db.is_commit_scanned(&commit1).await);
    assert!(db.is_commit_scanned(&commit2).await);
    assert!(!db.is_commit_scanned(&commit3).await);

    // Mark same commit again - should not error
    db.mark_commits_scanned(&[commit1])
        .await
        .unwrap();
    assert!(db.is_commit_scanned(&commit1).await);
}

// Tree Loading integration tests
// Verify database ↔ model data integrity

mod common;

use repodiet::repository::Database;
use repodiet::model::ExtensionStats;
use std::collections::HashMap;

/// Helper to create test database with initialized schema
async fn setup_db() -> Database {
    let db = common::create_test_db().await;
    db.init_schema().await.unwrap();
    db
}

#[tokio::test]
async fn test_tree_hierarchy_from_db() {
    let db = setup_db().await;

    // Save blobs with nested paths
    let blobs = vec![
        ("oid1".to_string(), "src/main.rs".to_string(), 100i64, 100i64),
        ("oid2".to_string(), "src/lib.rs".to_string(), 200i64, 200i64),
        ("oid3".to_string(), "src/utils/helpers.rs".to_string(), 50i64, 50i64),
        ("oid4".to_string(), "tests/test.rs".to_string(), 75i64, 75i64),
        ("oid5".to_string(), "README.md".to_string(), 25i64, 25i64),
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    let tree = db.load_tree().await.unwrap();

    // Verify root level
    assert_eq!(tree.children.len(), 3); // src, tests, README.md

    // Verify src structure
    let src = tree.children.get("src").unwrap();
    assert_eq!(src.children.len(), 3); // main.rs, lib.rs, utils

    // Verify nested structure
    let utils = src.children.get("utils").unwrap();
    assert!(utils.children.contains_key("helpers.rs"));

    // Verify tests
    let tests = tree.children.get("tests").unwrap();
    assert!(tests.children.contains_key("test.rs"));
}

#[tokio::test]
async fn test_size_aggregation_on_load() {
    let db = setup_db().await;

    // Save blobs in nested structure
    let blobs = vec![
        ("oid1".to_string(), "src/a.rs".to_string(), 100i64, 50i64),
        ("oid2".to_string(), "src/b.rs".to_string(), 200i64, 100i64),
        ("oid3".to_string(), "src/sub/c.rs".to_string(), 300i64, 150i64),
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    let tree = db.load_tree().await.unwrap();

    // Verify root totals (should be sum of all)
    assert_eq!(tree.cumulative_size, 600); // 100 + 200 + 300
    assert_eq!(tree.current_size, 300);    // 50 + 100 + 150

    // Verify src totals
    let src = tree.children.get("src").unwrap();
    assert_eq!(src.cumulative_size, 600);
    assert_eq!(src.current_size, 300);

    // Verify src/sub totals
    let sub = src.children.get("sub").unwrap();
    assert_eq!(sub.cumulative_size, 300);
    assert_eq!(sub.current_size, 150);
}

#[tokio::test]
async fn test_deleted_files_marked() {
    let db = setup_db().await;

    // Save blobs - some with current_size=0 (deleted)
    let blobs = vec![
        ("oid1".to_string(), "existing.txt".to_string(), 100i64, 100i64),
        ("oid2".to_string(), "deleted.txt".to_string(), 200i64, 0i64),
        ("oid3".to_string(), "src/also_deleted.rs".to_string(), 300i64, 0i64),
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    let tree = db.load_tree().await.unwrap();

    // Verify existing file
    let existing = tree.children.get("existing.txt").unwrap();
    assert!(existing.current_size > 0);

    // Verify deleted file (cumulative > 0, current = 0)
    let deleted = tree.children.get("deleted.txt").unwrap();
    assert_eq!(deleted.cumulative_size, 200);
    assert_eq!(deleted.current_size, 0);

    // contains_deleted_files should be true for root
    assert!(tree.contains_deleted_files());

    // deleted_cumulative_size should sum up deleted content
    assert_eq!(tree.deleted_cumulative_size(), 500); // 200 + 300
}

#[tokio::test]
async fn test_extension_stats_from_tree() {
    let db = setup_db().await;

    // Save blobs with different extensions
    let blobs = vec![
        ("oid1".to_string(), "src/main.rs".to_string(), 100i64, 100i64),
        ("oid2".to_string(), "src/lib.rs".to_string(), 200i64, 200i64),
        ("oid3".to_string(), "assets/logo.png".to_string(), 500i64, 500i64),
        ("oid4".to_string(), "assets/icon.png".to_string(), 300i64, 300i64),
        ("oid5".to_string(), "README.md".to_string(), 50i64, 50i64),
        ("oid6".to_string(), "Makefile".to_string(), 25i64, 25i64), // No extension
    ];
    db.save_blobs(&blobs, None).await.unwrap();

    let tree = db.load_tree().await.unwrap();

    // Compute extension stats
    let mut stats: HashMap<String, ExtensionStats> = HashMap::new();
    collect_extension_stats(&tree, &mut stats);

    // Verify .rs stats
    let rs = stats.get(".rs").unwrap();
    assert_eq!(rs.file_count, 2);
    assert_eq!(rs.cumulative_size, 300); // 100 + 200

    // Verify .png stats
    let png = stats.get(".png").unwrap();
    assert_eq!(png.file_count, 2);
    assert_eq!(png.cumulative_size, 800); // 500 + 300

    // Verify .md stats
    let md = stats.get(".md").unwrap();
    assert_eq!(md.file_count, 1);
    assert_eq!(md.cumulative_size, 50);
}

// Helper function to collect extension stats from tree
fn collect_extension_stats(node: &repodiet::model::TreeNode, stats: &mut HashMap<String, ExtensionStats>) {
    if node.children.is_empty() {
        // Leaf node - file
        let ext = if let Some(dot_pos) = node.name.rfind('.') {
            node.name[dot_pos..].to_string()
        } else {
            "(no ext)".to_string()
        };

        let entry = stats.entry(ext).or_insert(ExtensionStats::default());
        entry.file_count += 1;
        entry.cumulative_size += node.cumulative_size;
        entry.current_size += node.current_size;
    } else {
        // Directory - recurse
        for child in node.children.values() {
            collect_extension_stats(child, stats);
        }
    }
}

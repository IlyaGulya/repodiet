use anyhow::{Context, Result};
use indicatif::ProgressBar;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, Row, Sqlite};
use std::str::FromStr;

use crate::model::{LargeBlobInfo, TreeNode};

use super::SCHEMA_VERSION;

/// Database abstraction for SQLite operations
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(db_path: &str) -> Result<Self> {
        // Configure connection options with PRAGMAs applied to every connection
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}?mode=rwc", db_path))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .pragma("temp_store", "MEMORY")
            .pragma("cache_size", "-64000"); // 64MB cache

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("Failed to connect to database")?;

        Ok(Self { pool })
    }

    /// Initialize database schema, returns true if schema was rebuilt
    pub async fn init_schema(&self) -> Result<bool> {
        // Create metadata table first (needed to check version)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"
        ).execute(&self.pool).await?;

        // Check schema version
        let stored_version: Option<String> = sqlx::query("SELECT value FROM metadata WHERE key = 'schema_version'")
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get("value"));

        let needs_rebuild = stored_version.as_deref() != Some(SCHEMA_VERSION);

        if needs_rebuild {
            if stored_version.is_some() {
                eprintln!("Schema version changed ({} -> {}), rebuilding index...",
                    stored_version.unwrap_or_default(), SCHEMA_VERSION);
            }
            // Drop and recreate all tables (including old normalized tables if they exist)
            sqlx::query("DROP TABLE IF EXISTS path_stats").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS path_lookup").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS paths").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS seen_blobs").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS scanned_commits").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS blobs").execute(&self.pool).await?;
            sqlx::query("DELETE FROM metadata").execute(&self.pool).await?;
        }

        // Create tables - simple schema without normalization for performance
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS paths (
                path TEXT PRIMARY KEY,
                cumulative_size INTEGER NOT NULL,
                current_size INTEGER NOT NULL,
                blob_count INTEGER NOT NULL
            )"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS seen_blobs (
                oid BLOB PRIMARY KEY
            )"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS scanned_commits (
                oid BLOB PRIMARY KEY
            )"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS blobs (
                oid BLOB PRIMARY KEY,
                size INTEGER NOT NULL,
                path TEXT NOT NULL,
                first_author TEXT NOT NULL,
                first_date INTEGER NOT NULL
            )"
        ).execute(&self.pool).await?;

        // Store current schema version
        if needs_rebuild {
            sqlx::query("INSERT OR REPLACE INTO metadata (key, value) VALUES ('schema_version', ?)")
                .bind(SCHEMA_VERSION)
                .execute(&self.pool)
                .await?;
        }

        Ok(needs_rebuild)
    }

    /// Get metadata value by key
    pub async fn get_metadata(&self, key: &str) -> Option<String> {
        sqlx::query("SELECT value FROM metadata WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .map(|row| row.get("value"))
    }

    /// Set metadata value
    pub async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO metadata (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Load tree from database
    pub async fn load_tree(&self) -> Result<TreeNode> {
        let rows = sqlx::query("SELECT path, cumulative_size, current_size, blob_count FROM paths")
            .fetch_all(&self.pool)
            .await?;

        let mut root = TreeNode::new("(root)");

        for row in rows {
            let path: String = row.get("path");
            let cumulative: i64 = row.get("cumulative_size");
            let current: i64 = row.get("current_size");
            let count: i64 = row.get("blob_count");

            let parts: Vec<&str> = path.split('/').collect();
            root.add_path_with_sizes(&parts, cumulative as u64, current as u64, count as u64);
        }

        root.compute_totals();
        Ok(root)
    }

    /// Get top N largest blobs
    pub async fn get_top_blobs(&self, limit: usize) -> Result<Vec<LargeBlobInfo>> {
        let rows = sqlx::query(
            "SELECT oid, size, path, first_author, first_date FROM blobs ORDER BY size DESC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| {
            LargeBlobInfo {
                oid: row.get("oid"),
                size: row.get::<i64, _>("size") as u64,
                path: row.get("path"),
                first_author: row.get("first_author"),
                first_date: row.get("first_date"),
            }
        }).collect())
    }

    /// Check if a commit has been scanned (used by tests)
    #[allow(dead_code)]
    pub async fn is_commit_scanned(&self, oid: &[u8; 20]) -> bool {
        sqlx::query("SELECT 1 FROM scanned_commits WHERE oid = ?")
            .bind(oid.as_slice())
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    /// Load all scanned commit OIDs into a HashSet for fast lookup
    /// Returns raw 20-byte SHA-1 hashes
    pub async fn load_scanned_commit_oids(&self) -> rustc_hash::FxHashSet<[u8; 20]> {
        let rows: Vec<Vec<u8>> = sqlx::query_scalar("SELECT oid FROM scanned_commits")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.into_iter()
            .filter_map(|v| v.try_into().ok())
            .collect()
    }

    /// Load all previously seen blob OIDs
    /// Returns raw 20-byte SHA-1 hashes
    pub async fn load_seen_blobs(&self) -> Result<rustc_hash::FxHashSet<[u8; 20]>> {
        let rows = sqlx::query("SELECT oid FROM seen_blobs")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter()
            .filter_map(|row| {
                let v: Vec<u8> = row.get("oid");
                v.try_into().ok()
            })
            .collect())
    }

    /// Save batch of new blobs using multi-row INSERT for speed
    /// OIDs are raw 20-byte SHA-1 hashes (stored as BLOB)
    pub async fn save_blobs(&self, blobs: &[([u8; 20], String, i64, i64)], progress: Option<&ProgressBar>) -> Result<()> {
        const BATCH_SIZE: usize = 5000;

        // Use a single transaction for all batches
        let mut tx = self.pool.begin().await?;

        for chunk in blobs.chunks(BATCH_SIZE) {
            // Multi-row INSERT for seen_blobs
            if !chunk.is_empty() {
                let placeholders: Vec<&str> = vec!["(?)"; chunk.len()];
                let sql = format!(
                    "INSERT OR IGNORE INTO seen_blobs (oid) VALUES {}",
                    placeholders.join(", ")
                );
                let mut query = sqlx::query(&sql);
                for (oid, _, _, _) in chunk {
                    query = query.bind(oid.as_slice());
                }
                query.execute(&mut *tx).await?;
            }

            // paths table needs individual upserts due to complex ON CONFLICT logic
            for (_, path, cumulative, current) in chunk {
                sqlx::query(
                    "INSERT INTO paths (path, cumulative_size, current_size, blob_count) VALUES (?, ?, ?, 1)
                     ON CONFLICT(path) DO UPDATE SET
                        cumulative_size = cumulative_size + excluded.cumulative_size,
                        current_size = current_size + excluded.current_size,
                        blob_count = blob_count + 1"
                )
                .bind(path)
                .bind(cumulative)
                .bind(current)
                .execute(&mut *tx)
                .await?;
            }

            if let Some(pb) = progress {
                pb.inc(chunk.len() as u64);
            }
        }

        tx.commit().await?;
        Ok(())
    }

    /// Save blob metadata for large blob detection using multi-row INSERT
    /// OIDs are raw 20-byte SHA-1 hashes (stored as BLOB)
    pub async fn save_blob_metadata(&self, metadata: &[([u8; 20], i64, String, String, i64)], progress: Option<&ProgressBar>) -> Result<()> {
        const BATCH_SIZE: usize = 5000;

        // Use a single transaction for all batches
        let mut tx = self.pool.begin().await?;

        for chunk in metadata.chunks(BATCH_SIZE) {
            if chunk.is_empty() {
                continue;
            }

            // Build multi-row INSERT: VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), ...
            let placeholders: Vec<&str> = vec!["(?, ?, ?, ?, ?)"; chunk.len()];
            let sql = format!(
                "INSERT OR IGNORE INTO blobs (oid, size, path, first_author, first_date) VALUES {}",
                placeholders.join(", ")
            );

            let mut query = sqlx::query(&sql);
            for (oid, size, path, author, date) in chunk {
                query = query.bind(oid.as_slice()).bind(size).bind(path).bind(author).bind(date);
            }
            query.execute(&mut *tx).await?;

            if let Some(pb) = progress {
                pb.inc(chunk.len() as u64);
            }
        }

        tx.commit().await?;
        Ok(())
    }

    /// Mark commits as scanned using multi-row INSERT
    /// OIDs are raw 20-byte SHA-1 hashes (stored as BLOB)
    pub async fn mark_commits_scanned(&self, oids: &[[u8; 20]]) -> Result<()> {
        const BATCH_SIZE: usize = 5000;

        // Use a single transaction for all batches
        let mut tx = self.pool.begin().await?;

        for chunk in oids.chunks(BATCH_SIZE) {
            if chunk.is_empty() {
                continue;
            }

            let placeholders: Vec<&str> = vec!["(?)"; chunk.len()];
            let sql = format!(
                "INSERT OR IGNORE INTO scanned_commits (oid) VALUES {}",
                placeholders.join(", ")
            );

            let mut query = sqlx::query(&sql);
            for oid in chunk {
                query = query.bind(oid.as_slice());
            }
            query.execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

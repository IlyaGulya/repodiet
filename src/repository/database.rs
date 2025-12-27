use anyhow::{Context, Result};
use indicatif::ProgressBar;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Row, Sqlite};

use crate::model::{LargeBlobInfo, TreeNode};

use super::SCHEMA_VERSION;

/// Database abstraction for SQLite operations
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}?mode=rwc", db_path))
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
            // Drop and recreate all tables
            sqlx::query("DROP TABLE IF EXISTS paths").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS seen_blobs").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS scanned_commits").execute(&self.pool).await?;
            sqlx::query("DROP TABLE IF EXISTS blobs").execute(&self.pool).await?;
            sqlx::query("DELETE FROM metadata").execute(&self.pool).await?;
        }

        // Create tables
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
                oid TEXT PRIMARY KEY
            )"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS scanned_commits (
                oid TEXT PRIMARY KEY
            )"
        ).execute(&self.pool).await?;

        // New table for blob metadata (for large blob detection)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS blobs (
                oid TEXT PRIMARY KEY,
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

    /// Check if a commit has been scanned
    pub async fn is_commit_scanned(&self, oid: &str) -> bool {
        sqlx::query("SELECT 1 FROM scanned_commits WHERE oid = ?")
            .bind(oid)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    /// Load all previously seen blob OIDs
    pub async fn load_seen_blobs(&self) -> Result<std::collections::HashSet<String>> {
        let rows = sqlx::query("SELECT oid FROM seen_blobs")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|row| row.get::<String, _>("oid")).collect())
    }

    /// Save batch of new blobs
    pub async fn save_blobs(&self, blobs: &[(String, String, i64, i64)], progress: Option<&ProgressBar>) -> Result<()> {
        for chunk in blobs.chunks(1000) {
            let mut tx = self.pool.begin().await?;
            for (oid, path, cumulative, current) in chunk {
                sqlx::query("INSERT OR IGNORE INTO seen_blobs (oid) VALUES (?)")
                    .bind(oid)
                    .execute(&mut *tx)
                    .await?;

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
            tx.commit().await?;
            if let Some(pb) = progress {
                pb.inc(chunk.len() as u64);
            }
        }
        Ok(())
    }

    /// Save blob metadata for large blob detection
    pub async fn save_blob_metadata(&self, metadata: &[(String, i64, String, String, i64)], progress: Option<&ProgressBar>) -> Result<()> {
        for chunk in metadata.chunks(1000) {
            let mut tx = self.pool.begin().await?;
            for (oid, size, path, author, date) in chunk {
                sqlx::query(
                    "INSERT OR IGNORE INTO blobs (oid, size, path, first_author, first_date) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(oid)
                .bind(size)
                .bind(path)
                .bind(author)
                .bind(date)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            if let Some(pb) = progress {
                pb.inc(chunk.len() as u64);
            }
        }
        Ok(())
    }

    /// Mark commits as scanned
    pub async fn mark_commits_scanned(&self, oids: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for oid in oids {
            sqlx::query("INSERT OR IGNORE INTO scanned_commits (oid) VALUES (?)")
                .bind(oid)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

use anyhow::{Context, Result};
use indicatif::ProgressBar;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, QueryBuilder, Row, Sqlite, Transaction};
use std::borrow::Cow;
use std::str::FromStr;

use crate::model::{LargeBlobInfo, TreeNode};

use super::SCHEMA_VERSION;

/// A blob record for database storage (zero-copy friendly)
#[derive(Debug, Clone)]
pub struct BlobRecord<'a> {
    pub oid: [u8; 20],
    pub path: Cow<'a, str>,
    pub cumulative_size: i64,
    pub current_size: i64,
}

impl<'a> BlobRecord<'a> {
    pub fn new(oid: [u8; 20], path: impl Into<Cow<'a, str>>, cumulative_size: i64, current_size: i64) -> Self {
        Self { oid, path: path.into(), cumulative_size, current_size }
    }
}

/// Blob metadata record for database storage (zero-copy friendly)
#[derive(Debug, Clone)]
pub struct BlobMetaRecord<'a> {
    pub oid: [u8; 20],
    pub size: i64,
    pub path: Cow<'a, str>,
    pub author: Cow<'a, str>,
    pub timestamp: i64,
}

impl<'a> BlobMetaRecord<'a> {
    pub fn new(
        oid: [u8; 20],
        size: i64,
        path: impl Into<Cow<'a, str>>,
        author: impl Into<Cow<'a, str>>,
        timestamp: i64,
    ) -> Self {
        Self { oid, size, path: path.into(), author: author.into(), timestamp }
    }
}

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
    #[allow(dead_code)]
    pub async fn save_blobs(&self, blobs: &[BlobRecord<'_>], progress: Option<&ProgressBar>) -> Result<()> {
        self.save_blobs_with_callback(blobs, |n| {
            if let Some(pb) = progress {
                pb.inc(n as u64);
            }
        }).await
    }

    /// Save blob metadata for large blob detection using multi-row INSERT
    #[allow(dead_code)]
    pub async fn save_blob_metadata(&self, metadata: &[BlobMetaRecord<'_>], progress: Option<&ProgressBar>) -> Result<()> {
        self.save_blob_metadata_with_callback(metadata, |n| {
            if let Some(pb) = progress {
                pb.inc(n as u64);
            }
        }).await
    }

    /// Save batch of new blobs with a callback for progress
    pub async fn save_blobs_with_callback<F>(
        &self,
        blobs: &[BlobRecord<'_>],
        mut on_progress: F,
    ) -> Result<()>
    where
        F: FnMut(usize),
    {
        let mut tx = self.pool.begin().await?;
        self.save_blobs_in_tx(&mut tx, blobs, &mut on_progress)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Save blob metadata with a callback for progress
    pub async fn save_blob_metadata_with_callback<F>(
        &self,
        metadata: &[BlobMetaRecord<'_>],
        mut on_progress: F,
    ) -> Result<()>
    where
        F: FnMut(usize),
    {
        let mut tx = self.pool.begin().await?;
        self.save_blob_metadata_in_tx(&mut tx, metadata, &mut on_progress)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Mark commits as scanned using multi-row INSERT
    /// OIDs are raw 20-byte SHA-1 hashes (stored as BLOB)
    pub async fn mark_commits_scanned(&self, oids: &[[u8; 20]]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        self.mark_commits_scanned_in_tx(&mut tx, oids).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Apply blobs + metadata + scanned commits in ONE transaction.
    pub async fn apply_scan_with_callback<F1, F2>(
        &self,
        blobs: &[BlobRecord<'_>],
        metadata: &[BlobMetaRecord<'_>],
        scanned_commits: &[[u8; 20]],
        mut on_blobs_progress: F1,
        mut on_meta_progress: F2,
    ) -> Result<()>
    where
        F1: FnMut(usize),
        F2: FnMut(usize),
    {
        let mut tx = self.pool.begin().await?;

        // Persist rows
        self.save_blobs_in_tx(&mut tx, blobs, &mut on_blobs_progress)
            .await?;
        self.save_blob_metadata_in_tx(&mut tx, metadata, &mut on_meta_progress)
            .await?;

        // Advance state
        self.mark_commits_scanned_in_tx(&mut tx, scanned_commits)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn save_blobs_in_tx<F>(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        blobs: &[BlobRecord<'_>],
        on_progress: &mut F,
    ) -> Result<()>
    where
        F: FnMut(usize),
    {
        const BATCH_SIZE: usize = 5000;

        for chunk in blobs.chunks(BATCH_SIZE) {
            // Multi-row INSERT for seen_blobs using QueryBuilder
            if !chunk.is_empty() {
                let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
                    "INSERT OR IGNORE INTO seen_blobs (oid) "
                );
                qb.push_values(chunk, |mut row, record| {
                    row.push_bind(record.oid.as_slice());
                });
                qb.build().execute(&mut **tx).await?;
            }

            // Multi-row upsert for paths using QueryBuilder
            let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
                "INSERT INTO paths (path, cumulative_size, current_size, blob_count) "
            );
            qb.push_values(chunk, |mut row, record| {
                row.push_bind(record.path.as_ref())
                    .push_bind(record.cumulative_size)
                    .push_bind(record.current_size)
                    .push_bind(1_i64);
            });
            qb.push(
                " ON CONFLICT(path) DO UPDATE SET \
                    cumulative_size = cumulative_size + excluded.cumulative_size, \
                    current_size = current_size + excluded.current_size, \
                    blob_count = blob_count + excluded.blob_count"
            );
            qb.build().execute(&mut **tx).await?;

            on_progress(chunk.len());
        }

        Ok(())
    }

    async fn save_blob_metadata_in_tx<F>(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        metadata: &[BlobMetaRecord<'_>],
        on_progress: &mut F,
    ) -> Result<()>
    where
        F: FnMut(usize),
    {
        const BATCH_SIZE: usize = 5000;

        for chunk in metadata.chunks(BATCH_SIZE) {
            if chunk.is_empty() {
                continue;
            }

            let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
                "INSERT OR IGNORE INTO blobs (oid, size, path, first_author, first_date) "
            );
            qb.push_values(chunk, |mut row, record| {
                row.push_bind(record.oid.as_slice())
                    .push_bind(record.size)
                    .push_bind(record.path.as_ref())
                    .push_bind(record.author.as_ref())
                    .push_bind(record.timestamp);
            });
            qb.build().execute(&mut **tx).await?;

            on_progress(chunk.len());
        }

        Ok(())
    }

    async fn mark_commits_scanned_in_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        commits: &[[u8; 20]],
    ) -> Result<()> {
        const BATCH_SIZE: usize = 5000;

        for chunk in commits.chunks(BATCH_SIZE) {
            if chunk.is_empty() {
                continue;
            }

            let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
                "INSERT OR IGNORE INTO scanned_commits (oid) "
            );
            qb.push_values(chunk, |mut row, oid| {
                row.push_bind(oid.as_slice());
            });
            qb.build().execute(&mut **tx).await?;
        }

        Ok(())
    }
}

//! Database implementation of ScanStore

use anyhow::Result;
use gix::ObjectId;
use rustc_hash::FxHashSet;

use crate::model::TreeNode;
use crate::repository::{BlobMetaRecord, BlobRecord, Database};

use super::interner::PathInterner;
use super::progress::ProgressReporter;
use super::store::ScanStore;
use super::types::ScanDelta;

impl ScanStore for Database {
    async fn get_head_oid(&self) -> Option<String> {
        self.get_metadata("head_oid").await
    }

    async fn set_head_oid(&self, oid_hex: &str) -> Result<()> {
        self.set_metadata("head_oid", oid_hex).await
    }

    async fn load_scanned_commits(&self) -> FxHashSet<[u8; 20]> {
        self.load_scanned_commit_oids().await
    }

    async fn load_seen_blobs(&self) -> Result<FxHashSet<ObjectId>> {
        let bytes_set = Database::load_seen_blobs(self).await?;
        Ok(bytes_set
            .into_iter()
            .filter_map(|b| ObjectId::try_from(b.as_slice()).ok())
            .collect())
    }

    async fn save_delta_rows(
        &self,
        delta: &ScanDelta,
        interner: &PathInterner,
        progress: &dyn ProgressReporter,
    ) -> Result<()> {
        if delta.blobs.is_empty() {
            return Ok(());
        }

        // Convert BlobRow to database records
        let blobs_for_db: Vec<BlobRecord<'_>> = delta
            .blobs
            .iter()
            .map(|row| BlobRecord::new(
                row.oid.as_bytes().try_into().unwrap(),
                interner.get_str(row.path_id),
                row.cumulative_size,
                row.current_size,
            ))
            .collect();

        // Save blobs with progress
        let pb = progress.start("Indexing", blobs_for_db.len() as u64);
        self.save_blobs_with_callback(&blobs_for_db, |n| pb.inc(n as u64))
            .await?;
        pb.finish();

        // Convert BlobMetaRow to database records
        if !delta.metadata.is_empty() {
            let metadata_for_db: Vec<BlobMetaRecord<'_>> = delta
                .metadata
                .iter()
                .map(|row| BlobMetaRecord::new(
                    row.oid.as_bytes().try_into().unwrap(),
                    row.size,
                    interner.get_str(row.path_id),
                    row.author.clone(),
                    row.timestamp,
                ))
                .collect();

            let pb = progress.start("Indexing metadata", metadata_for_db.len() as u64);
            self.save_blob_metadata_with_callback(&metadata_for_db, |n| pb.inc(n as u64))
                .await?;
            pb.finish();
        }

        Ok(())
    }

    async fn mark_scanned_commits(&self, commits: &[ObjectId]) -> Result<()> {
        if commits.is_empty() {
            return Ok(());
        }

        let commit_oids: Vec<[u8; 20]> = commits
            .iter()
            .map(|o| o.as_bytes().try_into().unwrap())
            .collect();
        Database::mark_commits_scanned(self, &commit_oids).await
    }

    async fn load_tree(&self) -> Result<TreeNode> {
        Database::load_tree(self).await
    }

    async fn apply_scan(
        &self,
        delta: &ScanDelta,
        commits: &[ObjectId],
        interner: &PathInterner,
        progress: &dyn ProgressReporter,
    ) -> Result<()> {
        // Convert BlobRow to database records
        let blobs_for_db: Vec<BlobRecord<'_>> = delta
            .blobs
            .iter()
            .map(|row| {
                BlobRecord::new(
                    row.oid.as_bytes().try_into().unwrap(),
                    interner.get_str(row.path_id),
                    row.cumulative_size,
                    row.current_size,
                )
            })
            .collect();

        // Convert BlobMetaRow to database records
        let metadata_for_db: Vec<BlobMetaRecord<'_>> = delta
            .metadata
            .iter()
            .map(|row| {
                BlobMetaRecord::new(
                    row.oid.as_bytes().try_into().unwrap(),
                    row.size,
                    interner.get_str(row.path_id),
                    row.author.clone(),
                    row.timestamp,
                )
            })
            .collect();

        let commit_oids: Vec<[u8; 20]> = commits
            .iter()
            .map(|o| o.as_bytes().try_into().unwrap())
            .collect();

        let pb_blobs = progress.start("Indexing", blobs_for_db.len() as u64);
        let pb_meta = progress.start("Indexing metadata", metadata_for_db.len() as u64);

        self.apply_scan_with_callback(
            &blobs_for_db,
            &metadata_for_db,
            &commit_oids,
            |n| pb_blobs.inc(n as u64),
            |n| pb_meta.inc(n as u64),
        )
        .await?;

        pb_blobs.finish();
        pb_meta.finish();

        Ok(())
    }
}

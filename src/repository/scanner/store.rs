//! Scan store trait for persistence abstraction
//!
//! Decouples scanning logic from database implementation details.

use anyhow::Result;
use gix::ObjectId;
use rustc_hash::FxHashSet;

use crate::model::TreeNode;

use super::interner::PathInterner;
use super::progress::ProgressReporter;
use super::types::ScanDelta;

/// Persistence layer for scan data
///
/// This trait abstracts the database operations needed by the scanner,
/// allowing the scanning logic to be tested without a real database.
#[allow(async_fn_in_trait)]
pub trait ScanStore {
    /// Get the cached HEAD OID, if any
    async fn get_head_oid(&self) -> Option<String>;

    /// Set the cached HEAD OID
    async fn set_head_oid(&self, oid_hex: &str) -> Result<()>;

    /// Load all scanned commit OIDs into a set for fast lookup
    async fn load_scanned_commits(&self) -> FxHashSet<[u8; 20]>;

    /// Load all previously seen blob OIDs
    async fn load_seen_blobs(&self) -> Result<FxHashSet<ObjectId>>;

    /// Save blob and metadata rows from a scan delta
    ///
    /// This only persists the data rows, not the scanning state.
    /// Call `mark_scanned_commits` separately to update state.
    async fn save_delta_rows(
        &self,
        delta: &ScanDelta,
        interner: &PathInterner,
        progress: &dyn ProgressReporter,
    ) -> Result<()>;

    /// Mark commits as scanned
    ///
    /// This advances the scanning state, separate from row persistence.
    async fn mark_scanned_commits(&self, commits: &[ObjectId]) -> Result<()>;

    /// Load the tree from the database
    async fn load_tree(&self) -> Result<TreeNode>;

    /// Apply a scan result atomically if the store supports it.
    ///
    /// Default implementation is NOT atomic: it saves rows then marks commits.
    /// Database-backed stores should override this to do it in one transaction.
    async fn apply_scan(
        &self,
        delta: &ScanDelta,
        commits: &[ObjectId],
        interner: &PathInterner,
        progress: &dyn ProgressReporter,
    ) -> Result<()> {
        self.save_delta_rows(delta, interner, progress).await?;
        self.mark_scanned_commits(commits).await?;
        Ok(())
    }
}

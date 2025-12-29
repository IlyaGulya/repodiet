//! Domain types for git scanning
//!
//! These types form the data contract between scanning layers.

use gix::ObjectId;

/// Interned path identifier to avoid String allocations
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct PathId(pub u32);

impl PathId {
    #[allow(dead_code)]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// Information about a commit being scanned
#[derive(Debug, Clone)]
pub struct CommitInfo {
    #[allow(dead_code)]
    pub oid: ObjectId,
    pub tree: ObjectId,
    pub author: String,
    pub timestamp: i64,
}

/// A blob record to be saved to the database
#[derive(Debug, Clone)]
pub struct BlobRow {
    pub oid: ObjectId,
    pub path_id: PathId,
    pub cumulative_size: i64,
    pub current_size: i64,
}

/// Metadata about a blob (first author, first commit date)
#[derive(Debug, Clone)]
pub struct BlobMetaRow {
    pub oid: ObjectId,
    pub size: i64,
    pub path_id: PathId,
    pub author: String,
    pub timestamp: i64,
}

/// Snapshot of HEAD tree for determining "current" files
#[derive(Debug, Default)]
pub struct HeadSnapshot {
    /// HEAD commit OID as hex string (kept for debugging/display)
    #[allow(dead_code)]
    pub head_oid_hex: String,
    /// Maps path_id -> (blob_oid, compressed_size)
    pub blobs_by_path: rustc_hash::FxHashMap<PathId, (ObjectId, i64)>,
}

/// The result of scanning commits - blob/metadata rows to persist
///
/// Note: scanned commits are tracked separately by the orchestrator,
/// allowing explicit control over when commits are marked as scanned.
#[derive(Debug, Default)]
pub struct ScanDelta {
    pub blobs: Vec<BlobRow>,
    pub metadata: Vec<BlobMetaRow>,
}

impl ScanDelta {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty() && self.metadata.is_empty()
    }
}

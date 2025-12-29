//! Tree scanning context
//!
//! Encapsulates the recursive tree scanning algorithm with all necessary state.

use gix::prelude::{Find, FindExt};
use gix::ObjectId;
use rustc_hash::FxHashSet;

use super::interner::PathInterner;
use super::pack::PackSizeIndex;
use super::types::{BlobMetaRow, BlobRow, CommitInfo, HeadSnapshot, PathId, ScanDelta};

/// Buffer pool for reusing decode buffers across recursion
#[derive(Default)]
pub struct BufferPool {
    buffers: Vec<Vec<u8>>,
}

impl BufferPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a buffer from the pool (or allocate a new one)
    pub fn take(&mut self) -> Vec<u8> {
        self.buffers
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(8 * 1024))
    }

    /// Return a buffer to the pool
    pub fn give(&mut self, mut buf: Vec<u8>) {
        buf.clear();
        self.buffers.push(buf);
    }
}

/// Delta builder accumulates scan results
#[derive(Default)]
pub struct DeltaBuilder {
    blobs: Vec<BlobRow>,
    metadata: Vec<BlobMetaRow>,
}

impl DeltaBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a blob found during scanning
    pub fn record_blob(
        &mut self,
        oid: ObjectId,
        path_id: PathId,
        cumulative_size: i64,
        current_size: i64,
        commit: &CommitInfo,
        is_new_blob: bool,
    ) {
        if is_new_blob {
            self.blobs.push(BlobRow {
                oid,
                path_id,
                cumulative_size,
                current_size,
            });
            self.metadata.push(BlobMetaRow {
                oid,
                size: cumulative_size,
                path_id,
                author: commit.author.clone(),
                timestamp: commit.timestamp,
            });
        } else if current_size > 0 {
            // Existing blob at current path - only record current_size contribution
            self.blobs.push(BlobRow {
                oid,
                path_id,
                cumulative_size: 0,
                current_size,
            });
        }
    }

    /// Build the final ScanDelta from accumulated results
    pub fn build(self) -> ScanDelta {
        ScanDelta {
            blobs: self.blobs,
            metadata: self.metadata,
        }
    }

    pub fn blob_count(&self) -> usize {
        self.blobs.len()
    }
}

/// Context for tree scanning - replaces the 14-parameter recursive function
pub struct TreeScanCtx<'a, S: Find> {
    odb: &'a S,
    pack: &'a PackSizeIndex,
    head: &'a HeadSnapshot,

    pub interner: &'a mut PathInterner,
    buf_pool: BufferPool,

    seen_trees: FxHashSet<(ObjectId, PathId)>,
    seen_blobs: FxHashSet<ObjectId>,
    seen_path_blobs: FxHashSet<(PathId, ObjectId)>,

    out: DeltaBuilder,
}

impl<'a, S: Find> TreeScanCtx<'a, S> {
    pub fn new(
        odb: &'a S,
        pack: &'a PackSizeIndex,
        head: &'a HeadSnapshot,
        interner: &'a mut PathInterner,
        initial_seen_blobs: FxHashSet<ObjectId>,
    ) -> Self {
        Self {
            odb,
            pack,
            head,
            interner,
            buf_pool: BufferPool::new(),
            seen_trees: FxHashSet::default(),
            seen_blobs: initial_seen_blobs,
            seen_path_blobs: FxHashSet::default(),
            out: DeltaBuilder::new(),
        }
    }

    /// Scan a single commit's tree
    pub fn scan_commit(&mut self, commit: &CommitInfo) {
        let mut path_buf = Vec::with_capacity(256);
        self.scan_tree(commit.tree, &mut path_buf, commit);
    }

    /// Recursive tree scanning
    fn scan_tree(&mut self, tree_oid: ObjectId, path: &mut Vec<u8>, commit: &CommitInfo) {
        // Check if we've seen this (tree_oid, path) combination
        let path_id = self.interner.intern(path);
        if !self.seen_trees.insert((tree_oid, path_id)) {
            return;
        }

        // Get a buffer from the pool
        let mut buf = self.buf_pool.take();

        let tree = match self.odb.find_tree(&tree_oid, &mut buf) {
            Ok(t) => t,
            Err(_) => {
                self.buf_pool.give(buf);
                return;
            }
        };

        let base_len = path.len();

        for entry in tree.entries.iter() {
            // Build path
            if !path.is_empty() {
                path.push(b'/');
            }
            path.extend_from_slice(entry.filename.as_ref());

            let oid = entry.oid.to_owned();

            if entry.mode.is_blob() {
                self.handle_blob(oid, path, commit);
            } else if entry.mode.is_tree() {
                self.scan_tree(oid, path, commit);
            }

            // Restore path
            path.truncate(base_len);
        }

        self.buf_pool.give(buf);
    }

    /// Handle a blob entry
    fn handle_blob(&mut self, oid: ObjectId, path: &[u8], commit: &CommitInfo) {
        let path_id = self.interner.intern(path);

        // Check if we've seen this (path, oid) combination
        if !self.seen_path_blobs.insert((path_id, oid)) {
            return;
        }

        let is_new_blob = self.seen_blobs.insert(oid);
        let size = self.pack.size_of(oid);

        // Check if this blob is at this path in HEAD
        let current_size = self
            .head
            .blobs_by_path
            .get(&path_id)
            .filter(|(head_oid, _)| *head_oid == oid)
            .map(|(_, s)| *s)
            .unwrap_or(0);

        self.out
            .record_blob(oid, path_id, size, current_size, commit, is_new_blob);
    }

    /// Finish scanning and return the delta
    pub fn finish(self) -> ScanDelta {
        self.out.build()
    }

    /// Number of trees visited
    pub fn trees_visited(&self) -> usize {
        self.seen_trees.len()
    }

    /// Number of blobs found
    pub fn blobs_found(&self) -> usize {
        self.out.blob_count()
    }
}

//! Pack file size index
//!
//! Encapsulates compressed size lookups from pack files and loose objects.

use anyhow::Result;
use gix::ObjectId;
use gix_hash::Kind as HashKind;
use gix_pack::{data, index};
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

/// Index of compressed (on-disk) sizes for git objects
pub struct PackSizeIndex {
    /// Sizes from pack files (oid -> size)
    packed: FxHashMap<ObjectId, u64>,
    /// Path to objects directory for loose object lookups
    objects_dir: PathBuf,
}

impl PackSizeIndex {
    /// Load pack sizes from all pack files in the repository
    ///
    /// Uses gix's Repository to properly resolve the git directory,
    /// which handles bare repos, worktrees, and repos where .git is a file.
    pub fn load(repo: &gix::Repository) -> Self {
        let objects_dir = repo.objects.store_ref().path().to_path_buf();
        let packed = load_all_compressed_sizes(&objects_dir);
        Self {
            packed,
            objects_dir,
        }
    }

    /// Get the compressed size for an object
    pub fn size_of(&self, oid: ObjectId) -> i64 {
        // First check pack files
        if let Some(&size) = self.packed.get(&oid) {
            return size as i64;
        }
        // Fall back to loose object
        if let Some(size) = get_loose_object_size(&self.objects_dir, oid) {
            return size as i64;
        }
        0
    }

    /// Number of objects in pack index
    pub fn len(&self) -> usize {
        self.packed.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.packed.is_empty()
    }
}

/// Load compressed (on-disk) sizes for all objects in a pack file
fn load_pack_compressed_sizes(
    idx_path: &Path,
    pack_path: &Path,
) -> Result<FxHashMap<ObjectId, u64>> {
    let hash_kind = HashKind::Sha1;

    let idx = index::File::at(idx_path, hash_kind)?;
    let pack = data::File::at(pack_path, hash_kind)?;

    let mut entries: Vec<_> = idx.iter().collect();
    entries.sort_by_key(|e| e.pack_offset);

    let pack_end = pack.pack_end() as u64;
    let mut sizes = FxHashMap::default();
    sizes.reserve(entries.len());

    for (i, entry) in entries.iter().enumerate() {
        let entry_end = entries
            .get(i + 1)
            .map(|next| next.pack_offset)
            .unwrap_or(pack_end);

        let entry_size = entry_end - entry.pack_offset;
        sizes.insert(entry.oid, entry_size);
    }

    Ok(sizes)
}

/// Load compressed sizes from all pack files in objects/pack/
fn load_all_compressed_sizes(objects_dir: &Path) -> FxHashMap<ObjectId, u64> {
    let mut all_sizes = FxHashMap::default();
    let pack_dir = objects_dir.join("pack");

    if let Ok(entries) = std::fs::read_dir(&pack_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "idx") {
                let pack_path = path.with_extension("pack");
                if pack_path.exists() {
                    match load_pack_compressed_sizes(&path, &pack_path) {
                        Ok(sizes) => {
                            all_sizes.extend(sizes);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load pack {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }

    all_sizes
}

/// Get compressed size for a loose object by reading file size
fn get_loose_object_size(objects_dir: &Path, oid: ObjectId) -> Option<u64> {
    let hex = oid.to_hex().to_string();
    let path = objects_dir.join(&hex[..2]).join(&hex[2..]);
    std::fs::metadata(&path).ok().map(|m| m.len())
}

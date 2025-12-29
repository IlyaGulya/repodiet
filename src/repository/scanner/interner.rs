//! Path interning for zero-allocation tree traversal
//!
//! Maps path bytes to u32 IDs to avoid String allocations in hot paths.

use gix::bstr::BString;
use rustc_hash::FxHashMap;

use super::types::PathId;

/// Path interner to avoid allocating String for every path during tree traversal.
/// Maps paths to PathId for compact storage in HashSets.
#[derive(Default)]
pub struct PathInterner {
    map: FxHashMap<BString, u32>,
    vec: Vec<BString>,
}

impl PathInterner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a path and return its ID
    pub fn intern(&mut self, bytes: &[u8]) -> PathId {
        // Check if already interned
        if let Some(&id) = self.map.get(bytes) {
            return PathId(id);
        }
        let id = self.vec.len() as u32;
        let owned = BString::from(bytes);
        self.map.insert(owned.clone(), id);
        self.vec.push(owned);
        PathId(id)
    }

    /// Get the string representation of a path ID
    pub fn get_str(&self, id: PathId) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(self.vec[id.0 as usize].as_ref())
    }

    /// Get the raw bytes of a path ID (avoids UTF-8 conversion)
    #[allow(dead_code)]
    pub fn get_bytes(&self, id: PathId) -> &[u8] {
        self.vec[id.0 as usize].as_ref()
    }

    /// Number of interned paths
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_returns_same_id() {
        let mut interner = PathInterner::new();
        let id1 = interner.intern(b"foo/bar");
        let id2 = interner.intern(b"foo/bar");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_different_paths_different_ids() {
        let mut interner = PathInterner::new();
        let id1 = interner.intern(b"foo/bar");
        let id2 = interner.intern(b"foo/baz");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_get_str_roundtrip() {
        let mut interner = PathInterner::new();
        let id = interner.intern(b"src/main.rs");
        assert_eq!(interner.get_str(id), "src/main.rs");
    }
}

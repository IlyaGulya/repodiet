use std::collections::HashMap;

/// A node in our directory tree representing file/directory statistics
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub cumulative_size: u64,
    pub current_size: u64,
    pub blob_count: u64,
    pub children: HashMap<String, TreeNode>,
    /// Precomputed: whether this node or any descendant has deleted files
    pub has_deleted_descendants: bool,
    /// Precomputed: cumulative size of only deleted content in this subtree
    pub deleted_size: u64,
}

impl TreeNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            cumulative_size: 0,
            current_size: 0,
            blob_count: 0,
            children: HashMap::new(),
            has_deleted_descendants: false,
            deleted_size: 0,
        }
    }

    pub fn add_path_with_sizes(&mut self, path_parts: &[&str], cumulative: u64, current: u64, count: u64) {
        if path_parts.is_empty() {
            return;
        }

        let child_name = path_parts[0];
        let child = self
            .children
            .entry(child_name.to_string())
            .or_insert_with(|| TreeNode::new(child_name));

        if path_parts.len() == 1 {
            // Leaf node - add sizes here only, compute_totals will roll up
            child.cumulative_size += cumulative;
            child.current_size += current;
            child.blob_count += count;
        } else {
            child.add_path_with_sizes(&path_parts[1..], cumulative, current, count);
        }
    }

    pub fn compute_totals(&mut self) {
        if self.children.is_empty() {
            // Leaf node: compute deleted metrics directly
            let is_deleted = self.current_size == 0 && self.cumulative_size > 0;
            self.has_deleted_descendants = is_deleted;
            self.deleted_size = if is_deleted { self.cumulative_size } else { 0 };
        } else {
            // Directory: roll up from children
            for child in self.children.values_mut() {
                child.compute_totals();
                self.cumulative_size += child.cumulative_size;
                self.current_size += child.current_size;
                self.blob_count += child.blob_count;
                self.deleted_size += child.deleted_size;
                self.has_deleted_descendants |= child.has_deleted_descendants;
            }
        }
    }

    /// Check if this node or any of its descendants contains deleted files
    /// (files with current_size == 0 but cumulative_size > 0)
    #[inline]
    pub fn contains_deleted_files(&self) -> bool {
        self.has_deleted_descendants
    }

    /// Get the cumulative size of only deleted content
    /// (files where current_size == 0 but cumulative_size > 0)
    #[inline]
    pub fn deleted_cumulative_size(&self) -> u64 {
        self.deleted_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tree() -> TreeNode {
        let mut root = TreeNode::new("(root)");

        root.add_path_with_sizes(&["src", "main.rs"], 1000, 500, 1);
        root.add_path_with_sizes(&["src", "lib.rs"], 800, 400, 1);
        root.add_path_with_sizes(&["src", "utils", "helper.rs"], 300, 300, 1);
        root.add_path_with_sizes(&["assets", "logo.png"], 5000, 0, 1); // Deleted file
        root.add_path_with_sizes(&["assets", "icon.png"], 2000, 2000, 1);
        root.add_path_with_sizes(&["README.md"], 100, 100, 1);
        root.add_path_with_sizes(&["Cargo.toml"], 200, 200, 1);

        root.compute_totals();
        root
    }

    #[test]
    fn test_tree_node_totals() {
        let tree = create_test_tree();

        assert_eq!(tree.cumulative_size, 1000 + 800 + 300 + 5000 + 2000 + 100 + 200);
        assert_eq!(tree.current_size, 500 + 400 + 300 + 0 + 2000 + 100 + 200);
    }

    #[test]
    fn test_tree_node_children() {
        let tree = create_test_tree();

        assert_eq!(tree.children.len(), 4);
        assert!(tree.children.contains_key("src"));
        assert!(tree.children.contains_key("assets"));
        assert!(tree.children.contains_key("README.md"));
        assert!(tree.children.contains_key("Cargo.toml"));
    }

    #[test]
    fn test_contains_deleted_files() {
        let tree = create_test_tree();

        let assets = tree.children.get("assets").unwrap();
        assert!(assets.contains_deleted_files());

        let src = tree.children.get("src").unwrap();
        assert!(!src.contains_deleted_files());
    }

    #[test]
    fn test_deleted_cumulative_size() {
        let tree = create_test_tree();

        let assets = tree.children.get("assets").unwrap();
        assert_eq!(assets.cumulative_size, 7000);
        assert_eq!(assets.deleted_cumulative_size(), 5000);
    }

    #[test]
    fn test_bloat_calculation() {
        let tree = create_test_tree();

        let assets = tree.children.get("assets").unwrap();
        let logo = assets.children.get("logo.png").unwrap();
        assert_eq!(logo.cumulative_size, 5000);
        assert_eq!(logo.current_size, 0);

        let src = tree.children.get("src").unwrap();
        let main = src.children.get("main.rs").unwrap();
        assert_eq!(main.cumulative_size, 1000);
        assert_eq!(main.current_size, 500);
    }
}

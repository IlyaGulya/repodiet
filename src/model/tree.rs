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

    /// Visits all leaf nodes, calling `f` with the full path and node.
    /// Uses a reusable path buffer - only allocates once per leaf when caller clones.
    pub fn visit_leaves(&self, mut f: impl FnMut(&str, &TreeNode)) {
        // Stack stores (node, base_len) where base_len is path length before this node
        let mut stack: Vec<(&TreeNode, usize)> = Vec::new();
        let mut path = String::new();

        // Seed with root's children
        for child in self.children.values() {
            stack.push((child, 0));
        }

        while let Some((node, base_len)) = stack.pop() {
            // Truncate path back to before this node's name
            path.truncate(base_len);
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(&node.name);

            if node.children.is_empty() {
                f(&path, node);
            } else {
                let current_len = path.len();
                for child in node.children.values() {
                    stack.push((child, current_len));
                }
            }
        }
    }

    /// Visits all leaf nodes without path allocation.
    /// Use when you only need node data, not paths.
    pub fn visit_leaf_nodes(&self, mut f: impl FnMut(&TreeNode)) {
        let mut stack: Vec<&TreeNode> = self.children.values().collect();
        while let Some(node) = stack.pop() {
            if node.children.is_empty() {
                f(node);
            } else {
                stack.extend(node.children.values());
            }
        }
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

    #[test]
    fn test_visit_leaves() {
        let tree = create_test_tree();

        let mut paths = Vec::new();
        tree.visit_leaves(|path, _| paths.push(path.to_string()));

        // Should have 7 leaf nodes (files)
        assert_eq!(paths.len(), 7);

        // All should have paths
        assert!(paths.iter().any(|p| p.ends_with("main.rs")));
        assert!(paths.iter().any(|p| p.ends_with("lib.rs")));
        assert!(paths.iter().any(|p| p.ends_with("helper.rs")));
        assert!(paths.iter().any(|p| p.ends_with("logo.png")));
        assert!(paths.iter().any(|p| p.ends_with("icon.png")));
        assert!(paths.iter().any(|p| p == "README.md"));
        assert!(paths.iter().any(|p| p == "Cargo.toml"));

        // Paths should include full path
        assert!(paths.iter().any(|p| p == "src/main.rs" || p == "src/lib.rs"));
    }

    #[test]
    fn test_visit_leaf_nodes() {
        let tree = create_test_tree();

        let mut total_size = 0u64;
        let mut count = 0usize;
        tree.visit_leaf_nodes(|node| {
            total_size += node.cumulative_size;
            count += 1;
        });

        // Should have 7 leaf nodes (files)
        assert_eq!(count, 7);

        // Verify we can access node data
        assert_eq!(total_size, tree.cumulative_size);
    }
}

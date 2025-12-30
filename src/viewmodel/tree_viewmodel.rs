use std::cell::RefCell;
use std::sync::Arc;

use crate::model::TreeNode;

use super::selection::Selectable;

/// View representation of a tree node for rendering
#[derive(Debug, Clone)]
pub struct TreeNodeView {
    pub name: String,
    pub display_size: u64,
    pub current_size: u64,
    pub has_children: bool,
}

struct ChildrenCache {
    children: Vec<TreeNodeView>,
    dirty: bool,
}

/// ViewModel for tree navigation
pub struct TreeViewModel {
    root: Arc<TreeNode>,
    path_stack: Vec<String>,
    selected_index: usize,
    show_deleted_only: bool,
    total_cumulative: u64,
    cache: RefCell<ChildrenCache>,
}

impl TreeViewModel {
    pub fn new(root: Arc<TreeNode>) -> Self {
        let total_cumulative = root.cumulative_size;
        Self {
            root,
            path_stack: Vec::new(),
            selected_index: 0,
            show_deleted_only: false,
            total_cumulative,
            cache: RefCell::new(ChildrenCache {
                children: Vec::new(),
                dirty: true,
            }),
        }
    }

    /// Get the total deleted size (for deleted-only mode header)
    pub fn total_deleted(&self) -> u64 {
        self.root.deleted_cumulative_size()
    }

    /// Check if we're at the root level
    pub fn is_at_root(&self) -> bool {
        self.path_stack.is_empty()
    }

    /// Check if deleted-only mode is active
    pub fn is_deleted_only(&self) -> bool {
        self.show_deleted_only
    }

    /// Get the current path as a string
    pub fn current_path(&self) -> String {
        if self.path_stack.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", self.path_stack.join("/"))
        }
    }

    /// Get the current node in the path
    pub fn current_node(&self) -> &TreeNode {
        let mut node: &TreeNode = &self.root;
        for name in &self.path_stack {
            if let Some(child) = node.children.get(name) {
                node = child;
            }
        }
        node
    }

    /// Invalidate the children cache and reset selection
    fn invalidate(&mut self) {
        self.cache.borrow_mut().dirty = true;
        self.selected_index = 0;
    }

    /// Ensure the children cache is populated
    fn ensure_children(&self) {
        let mut cache = self.cache.borrow_mut();
        if !cache.dirty {
            return;
        }

        let current = self.current_node();
        let mut children: Vec<_> = current
            .children
            .values()
            .filter(|node| !self.show_deleted_only || node.contains_deleted_files())
            .map(|node| {
                let display_size = if self.show_deleted_only {
                    node.deleted_cumulative_size()
                } else {
                    node.cumulative_size
                };
                TreeNodeView {
                    name: node.name.clone(),
                    display_size,
                    current_size: node.current_size,
                    has_children: !node.children.is_empty(),
                }
            })
            .collect();

        children.sort_by(|a, b| b.display_size.cmp(&a.display_size));
        cache.children = children;
        cache.dirty = false;
    }

    /// Get visible children based on current filters
    pub fn visible_children(&self) -> std::cell::Ref<'_, Vec<TreeNodeView>> {
        self.ensure_children();
        std::cell::Ref::map(self.cache.borrow(), |c| &c.children)
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get total for percentage calculations
    pub fn total_for_percent(&self) -> u64 {
        if self.show_deleted_only {
            self.total_deleted()
        } else {
            self.total_cumulative
        }
    }

    pub fn enter_selected(&mut self) {
        let child_name = {
            let children = self.visible_children();
            children
                .get(self.selected_index)
                .filter(|c| c.has_children)
                .map(|c| c.name.clone())
        };
        if let Some(name) = child_name {
            self.path_stack.push(name);
            self.invalidate();
        }
    }

    /// Go back one level, returns false if already at root
    pub fn go_back(&mut self) -> bool {
        if self.path_stack.is_empty() {
            false
        } else {
            self.path_stack.pop();
            self.invalidate();
            true
        }
    }

    pub fn toggle_deleted_only(&mut self) {
        self.show_deleted_only = !self.show_deleted_only;
        self.invalidate();
    }

    /// Navigate to a specific path (used by search results)
    pub fn navigate_to_path(&mut self, path: &str) {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() > 1 {
            self.path_stack = parts[..parts.len() - 1]
                .iter()
                .map(|s| s.to_string())
                .collect();
            self.invalidate();
        }
    }
}

impl Selectable for TreeViewModel {
    fn len(&self) -> usize {
        self.ensure_children();
        self.cache.borrow().children.len()
    }

    fn selected(&self) -> usize {
        self.selected_index
    }

    fn set_selected(&mut self, index: usize) {
        self.selected_index = index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tree() -> Arc<TreeNode> {
        let mut root = TreeNode::new("(root)");
        root.add_path_with_sizes(&["src", "main.rs"], 1000, 500, 1);
        root.add_path_with_sizes(&["src", "lib.rs"], 800, 400, 1);
        root.add_path_with_sizes(&["assets", "logo.png"], 5000, 0, 1);
        root.add_path_with_sizes(&["assets", "icon.png"], 2000, 2000, 1);
        root.add_path_with_sizes(&["README.md"], 100, 100, 1);
        root.compute_totals();
        Arc::new(root)
    }

    #[test]
    fn test_navigation() {
        let tree = create_test_tree();
        let mut vm = TreeViewModel::new(tree);

        assert!(vm.is_at_root());
        assert_eq!(vm.current_path(), "/");

        // Move to first child (sorted by size, so assets first)
        assert!(!vm.visible_children().is_empty());

        vm.enter_selected();
        assert!(!vm.is_at_root());

        assert!(vm.go_back());
        assert!(vm.is_at_root());
    }

    #[test]
    fn test_deleted_filter() {
        let tree = create_test_tree();
        let mut vm = TreeViewModel::new(tree);

        vm.toggle_deleted_only();
        assert!(vm.is_deleted_only());

        let children = vm.visible_children();
        // Only assets should be visible (contains deleted logo.png)
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "assets");
    }
}

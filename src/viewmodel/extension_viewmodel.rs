use std::collections::HashMap;
use crate::model::{ExtensionStats, TreeNode};

use super::selection::Selectable;

/// Computed stats for display
#[derive(Debug, Clone)]
pub struct ExtensionStatsView {
    pub extension: String,
    pub cumulative_size: u64,
    pub current_size: u64,
    pub file_count: u64,
}

/// ViewModel for extension breakdown view
pub struct ExtensionViewModel {
    stats: Vec<ExtensionStatsView>,
    selected_index: usize,
    total_cumulative: u64,
    total_current: u64,
    total_files: u64,
}

impl ExtensionViewModel {
    pub fn new(root: &TreeNode) -> Self {
        let stats = Self::compute_stats(root);
        let total_cumulative = stats.iter().map(|s| s.cumulative_size).sum();
        let total_current = stats.iter().map(|s| s.current_size).sum();
        let total_files = stats.iter().map(|s| s.file_count).sum();

        Self {
            stats,
            selected_index: 0,
            total_cumulative,
            total_current,
            total_files,
        }
    }

    fn compute_stats(root: &TreeNode) -> Vec<ExtensionStatsView> {
        let mut stats: HashMap<String, ExtensionStats> = HashMap::new();

        root.visit_leaf_nodes(|node| {
            let ext = node
                .name
                .rsplit_once('.')
                .map(|(_, e)| e)
                .filter(|e| !e.is_empty() && e.len() <= 10 && !e.contains('/'))
                .map(|e| format!(".{}", e.to_lowercase()))
                .unwrap_or_else(|| "(no ext)".to_string());

            let entry = stats.entry(ext).or_default();
            entry.cumulative_size += node.cumulative_size;
            entry.current_size += node.current_size;
            entry.file_count += node.blob_count;
        });

        let mut result: Vec<_> = stats
            .into_iter()
            .map(|(ext, s)| ExtensionStatsView {
                extension: ext,
                cumulative_size: s.cumulative_size,
                current_size: s.current_size,
                file_count: s.file_count,
            })
            .collect();

        result.sort_by(|a, b| b.cumulative_size.cmp(&a.cumulative_size));
        result
    }

    pub fn stats(&self) -> &[ExtensionStatsView] {
        &self.stats
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn total_cumulative(&self) -> u64 {
        self.total_cumulative
    }

    pub fn total_current(&self) -> u64 {
        self.total_current
    }

    pub fn total_files(&self) -> u64 {
        self.total_files
    }
}

impl Selectable for ExtensionViewModel {
    fn len(&self) -> usize {
        self.stats.len()
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

    fn create_test_tree() -> TreeNode {
        let mut root = TreeNode::new("(root)");
        root.add_path_with_sizes(&["src", "main.rs"], 1000, 500, 1);
        root.add_path_with_sizes(&["src", "lib.rs"], 800, 400, 1);
        root.add_path_with_sizes(&["assets", "logo.png"], 5000, 0, 1);
        root.add_path_with_sizes(&["assets", "icon.png"], 2000, 2000, 1);
        root.compute_totals();
        root
    }

    #[test]
    fn test_extension_stats() {
        let tree = create_test_tree();
        let vm = ExtensionViewModel::new(&tree);

        let stats = vm.stats();
        assert!(!stats.is_empty());

        // Should have .rs and .png
        let ext_names: Vec<_> = stats.iter().map(|s| s.extension.as_str()).collect();
        assert!(ext_names.contains(&".rs"));
        assert!(ext_names.contains(&".png"));
    }
}

use std::sync::Arc;

use crate::model::TreeNode;

use super::selection::Selectable;

/// Precomputed entry for fast searching
struct SearchEntry {
    path: String,
    path_lower: String,
    cumulative_size: u64,
    current_size: u64,
}

/// ViewModel for search functionality
pub struct SearchViewModel {
    query: String,
    result_indices: Vec<usize>,
    selected_index: usize,
    entries: Vec<SearchEntry>,
    total_cumulative: u64,
}

impl SearchViewModel {
    pub fn new(root: Arc<TreeNode>) -> Self {
        let total_cumulative = root.cumulative_size;
        let mut entries = Vec::new();

        root.visit_leaves(|path, node| {
            entries.push(SearchEntry {
                path_lower: path.to_lowercase(),
                path: path.to_string(),
                cumulative_size: node.cumulative_size,
                current_size: node.current_size,
            });
        });

        Self {
            query: String::new(),
            result_indices: Vec::new(),
            selected_index: 0,
            entries,
            total_cumulative,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn results(&self) -> impl Iterator<Item = (&str, u64, u64)> + '_ {
        self.result_indices.iter().map(|&i| {
            let e = &self.entries[i];
            (e.path.as_str(), e.cumulative_size, e.current_size)
        })
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn total_cumulative(&self) -> u64 {
        self.total_cumulative
    }

    pub fn add_char(&mut self, c: char) {
        self.query.push(c);
        self.update_results();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.update_results();
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.result_indices.clear();
        self.selected_index = 0;
    }

    fn update_results(&mut self) {
        if self.query.is_empty() {
            self.result_indices.clear();
            self.selected_index = 0;
            return;
        }

        let query_lower = self.query.to_lowercase();

        let mut matched: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.path_lower.contains(&query_lower))
            .map(|(i, e)| (i, e.cumulative_size))
            .collect();

        matched.sort_by(|a, b| b.1.cmp(&a.1));
        matched.truncate(100);

        self.result_indices = matched.into_iter().map(|(i, _)| i).collect();
        self.selected_index = 0;
    }

    /// Get selected result's path
    pub fn selected_path(&self) -> Option<&str> {
        self.result_indices
            .get(self.selected_index)
            .map(|&i| self.entries[i].path.as_str())
    }
}

impl Selectable for SearchViewModel {
    fn len(&self) -> usize {
        self.result_indices.len() // O(1)
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
        root.add_path_with_sizes(&["README.md"], 100, 100, 1);
        root.compute_totals();
        Arc::new(root)
    }

    #[test]
    fn test_search() {
        let tree = create_test_tree();
        let mut vm = SearchViewModel::new(tree);

        vm.add_char('.');
        vm.add_char('r');
        vm.add_char('s');

        assert_eq!(vm.results().count(), 2);
    }

    #[test]
    fn test_search_case_insensitive() {
        let tree = create_test_tree();
        let mut vm = SearchViewModel::new(tree);

        for c in "README".chars() {
            vm.add_char(c);
        }

        assert_eq!(vm.results().count(), 1);
    }

    #[test]
    fn test_empty_search() {
        let tree = create_test_tree();
        let vm = SearchViewModel::new(tree);

        assert_eq!(vm.results().count(), 0);
    }
}

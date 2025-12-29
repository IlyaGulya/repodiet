use crate::model::{SearchResult, TreeNode};

use super::selection;

/// ViewModel for search functionality
pub struct SearchViewModel {
    query: String,
    results: Vec<SearchResult>,
    selected_index: usize,
    root: TreeNode,
    total_cumulative: u64,
}

impl SearchViewModel {
    pub fn new(root: TreeNode) -> Self {
        let total_cumulative = root.cumulative_size;
        Self {
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
            root,
            total_cumulative,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn results(&self) -> &[SearchResult] {
        &self.results
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
        self.results.clear();
        self.selected_index = 0;
    }

    fn update_results(&mut self) {
        if self.query.is_empty() {
            self.results.clear();
            return;
        }

        let query_lower = self.query.to_lowercase();
        let mut results = Vec::new();

        fn search_recursive(
            node: &TreeNode,
            path: &str,
            query: &str,
            results: &mut Vec<SearchResult>,
        ) {
            let full_path = if path.is_empty() {
                node.name.clone()
            } else {
                format!("{}/{}", path, node.name)
            };

            if node.children.is_empty() {
                if full_path.to_lowercase().contains(query) {
                    results.push(SearchResult {
                        path: full_path.clone(),
                        cumulative_size: node.cumulative_size,
                        current_size: node.current_size,
                    });
                }
            }

            for child in node.children.values() {
                search_recursive(child, &full_path, query, results);
            }
        }

        for child in self.root.children.values() {
            search_recursive(child, "", &query_lower, &mut results);
        }

        results.sort_by(|a, b| b.cumulative_size.cmp(&a.cumulative_size));
        results.truncate(100);

        self.results = results;
        self.selected_index = 0;
    }

    pub fn move_up(&mut self) {
        selection::move_up(&mut self.selected_index, self.results.len());
    }

    pub fn move_down(&mut self) {
        selection::move_down(&mut self.selected_index, self.results.len());
    }

    /// Get selected result's path
    pub fn selected_path(&self) -> Option<&str> {
        self.results.get(self.selected_index).map(|r| r.path.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tree() -> TreeNode {
        let mut root = TreeNode::new("(root)");
        root.add_path_with_sizes(&["src", "main.rs"], 1000, 500, 1);
        root.add_path_with_sizes(&["src", "lib.rs"], 800, 400, 1);
        root.add_path_with_sizes(&["README.md"], 100, 100, 1);
        root.compute_totals();
        root
    }

    #[test]
    fn test_search() {
        let tree = create_test_tree();
        let mut vm = SearchViewModel::new(tree);

        vm.add_char('.');
        vm.add_char('r');
        vm.add_char('s');

        assert_eq!(vm.results().len(), 2);
    }

    #[test]
    fn test_search_case_insensitive() {
        let tree = create_test_tree();
        let mut vm = SearchViewModel::new(tree);

        for c in "README".chars() {
            vm.add_char(c);
        }

        assert_eq!(vm.results().len(), 1);
    }

    #[test]
    fn test_empty_search() {
        let tree = create_test_tree();
        let vm = SearchViewModel::new(tree);

        assert!(vm.results().is_empty());
    }
}

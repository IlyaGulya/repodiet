use crate::model::{LargeBlobInfo, TreeNode};
use crate::input::Intent;
use super::{TreeViewModel, ExtensionViewModel, SearchViewModel, BlobsViewModel};

/// Current view mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Tree,
    ByExtension,
    LargeBlobs,
    Search,
}

/// Action to take after handling an intent
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Redraw,
    Quit,
}

/// Main application ViewModel coordinating all view-specific ViewModels
pub struct AppViewModel {
    view_mode: ViewMode,
    pub tree_vm: TreeViewModel,
    pub extension_vm: ExtensionViewModel,
    pub search_vm: SearchViewModel,
    pub blobs_vm: BlobsViewModel,
}

impl AppViewModel {
    pub fn new(root: TreeNode, large_blobs: Vec<LargeBlobInfo>) -> Self {
        let total_cumulative = root.cumulative_size;
        let extension_vm = ExtensionViewModel::new(&root);
        let search_vm = SearchViewModel::new(root.clone());
        let tree_vm = TreeViewModel::new(root);
        let blobs_vm = BlobsViewModel::new(large_blobs, total_cumulative);

        Self {
            view_mode: ViewMode::Tree,
            tree_vm,
            extension_vm,
            search_vm,
            blobs_vm,
        }
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn is_search_mode(&self) -> bool {
        self.view_mode == ViewMode::Search
    }

    fn move_up_current(&mut self) {
        match self.view_mode {
            ViewMode::Tree => self.tree_vm.move_up(),
            ViewMode::ByExtension => self.extension_vm.move_up(),
            ViewMode::LargeBlobs => self.blobs_vm.move_up(),
            ViewMode::Search => self.search_vm.move_up(),
        }
    }

    fn move_down_current(&mut self) {
        match self.view_mode {
            ViewMode::Tree => self.tree_vm.move_down(),
            ViewMode::ByExtension => self.extension_vm.move_down(),
            ViewMode::LargeBlobs => self.blobs_vm.move_down(),
            ViewMode::Search => self.search_vm.move_down(),
        }
    }

    fn enter_current(&mut self) {
        match self.view_mode {
            ViewMode::Tree => self.tree_vm.enter_selected(),
            ViewMode::LargeBlobs => {
                if let Some(path) = self.blobs_vm.selected_path() {
                    self.tree_vm.navigate_to_path(path);
                    self.view_mode = ViewMode::Tree;
                }
            }
            ViewMode::Search => {
                if let Some(path) = self.search_vm.selected_path() {
                    self.tree_vm.navigate_to_path(path);
                    self.search_vm.clear();
                    self.view_mode = ViewMode::Tree;
                }
            }
            ViewMode::ByExtension => {}
        }
    }

    /// Handle a user intent and return the action to take
    pub fn handle_intent(&mut self, intent: Intent) -> Action {
        match intent {
            Intent::Quit => Action::Quit,

            Intent::ShowTree => {
                if self.view_mode == ViewMode::Search {
                    self.search_vm.clear();
                }
                self.view_mode = ViewMode::Tree;
                Action::Redraw
            }

            Intent::ShowExtensions => {
                self.view_mode = ViewMode::ByExtension;
                Action::Redraw
            }

            Intent::ShowLargeBlobs => {
                self.view_mode = ViewMode::LargeBlobs;
                Action::Redraw
            }

            Intent::EnterSearch => {
                self.search_vm.clear();
                self.view_mode = ViewMode::Search;
                Action::Redraw
            }

            Intent::MoveUp => {
                self.move_up_current();
                Action::Redraw
            }

            Intent::MoveDown => {
                self.move_down_current();
                Action::Redraw
            }

            Intent::Enter => {
                self.enter_current();
                Action::Redraw
            }

            Intent::Back => {
                if self.view_mode == ViewMode::Tree {
                    self.tree_vm.go_back();
                }
                Action::Redraw
            }

            Intent::ToggleDeletedOnly => {
                if self.view_mode == ViewMode::Tree {
                    self.tree_vm.toggle_deleted_only();
                }
                Action::Redraw
            }

            Intent::SearchChar(c) => {
                if self.view_mode == ViewMode::Search {
                    self.search_vm.add_char(c);
                }
                Action::Redraw
            }

            Intent::SearchBackspace => {
                if self.view_mode == ViewMode::Search {
                    self.search_vm.backspace();
                }
                Action::Redraw
            }
        }
    }

    /// Get the ViewMode as input::ViewMode for key mapping
    pub fn input_view_mode(&self) -> crate::input::ViewMode {
        match self.view_mode {
            ViewMode::Tree => crate::input::ViewMode::Tree,
            ViewMode::ByExtension => crate::input::ViewMode::ByExtension,
            ViewMode::LargeBlobs => crate::input::ViewMode::LargeBlobs,
            ViewMode::Search => crate::input::ViewMode::Tree, // Search handles keys specially
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tree() -> TreeNode {
        let mut root = TreeNode::new("(root)");
        root.add_path_with_sizes(&["src", "main.rs"], 1000, 500, 1);
        root.add_path_with_sizes(&["assets", "logo.png"], 5000, 0, 1);
        root.compute_totals();
        root
    }

    #[test]
    fn test_mode_switching() {
        let tree = create_test_tree();
        let mut vm = AppViewModel::new(tree, vec![]);

        assert_eq!(vm.view_mode(), ViewMode::Tree);

        vm.handle_intent(Intent::ShowExtensions);
        assert_eq!(vm.view_mode(), ViewMode::ByExtension);

        vm.handle_intent(Intent::ShowTree);
        assert_eq!(vm.view_mode(), ViewMode::Tree);

        vm.handle_intent(Intent::EnterSearch);
        assert_eq!(vm.view_mode(), ViewMode::Search);
    }

    #[test]
    fn test_quit_action() {
        let tree = create_test_tree();
        let mut vm = AppViewModel::new(tree, vec![]);

        let action = vm.handle_intent(Intent::Quit);
        assert_eq!(action, Action::Quit);
    }
}

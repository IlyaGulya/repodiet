/// Trait for navigable list views with selection
pub trait Selectable {
    /// Returns the number of items in the list
    fn len(&self) -> usize;

    /// Returns the currently selected index
    fn selected(&self) -> usize;

    /// Sets the selected index directly
    fn set_selected(&mut self, index: usize);

    /// Returns true if the list is empty
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Move selection up with wraparound
    fn move_up(&mut self) {
        let len = self.len();
        if len == 0 {
            self.set_selected(0);
            return;
        }
        let selected = self.selected();
        self.set_selected(if selected == 0 { len - 1 } else { selected - 1 });
    }

    /// Move selection down with wraparound
    fn move_down(&mut self) {
        let len = self.len();
        if len == 0 {
            self.set_selected(0);
            return;
        }
        let selected = self.selected();
        self.set_selected(if selected + 1 >= len { 0 } else { selected + 1 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test struct implementing Selectable
    struct TestList {
        items: Vec<i32>,
        selected_index: usize,
    }

    impl Selectable for TestList {
        fn len(&self) -> usize {
            self.items.len()
        }

        fn selected(&self) -> usize {
            self.selected_index
        }

        fn set_selected(&mut self, index: usize) {
            self.selected_index = index;
        }
    }

    #[test]
    fn test_move_up_wraparound() {
        let mut list = TestList { items: vec![1, 2, 3], selected_index: 0 };
        list.move_up();
        assert_eq!(list.selected(), 2);
    }

    #[test]
    fn test_move_up_normal() {
        let mut list = TestList { items: vec![1, 2, 3], selected_index: 2 };
        list.move_up();
        assert_eq!(list.selected(), 1);
    }

    #[test]
    fn test_move_down_wraparound() {
        let mut list = TestList { items: vec![1, 2, 3], selected_index: 2 };
        list.move_down();
        assert_eq!(list.selected(), 0);
    }

    #[test]
    fn test_move_down_normal() {
        let mut list = TestList { items: vec![1, 2, 3], selected_index: 0 };
        list.move_down();
        assert_eq!(list.selected(), 1);
    }

    #[test]
    fn test_empty_list() {
        let mut list = TestList { items: vec![], selected_index: 5 };
        list.move_up();
        assert_eq!(list.selected(), 0);

        list.selected_index = 5;
        list.move_down();
        assert_eq!(list.selected(), 0);
    }

    #[test]
    fn test_is_empty() {
        let empty_list = TestList { items: vec![], selected_index: 0 };
        let non_empty_list = TestList { items: vec![1], selected_index: 0 };
        assert!(empty_list.is_empty());
        assert!(!non_empty_list.is_empty());
    }
}

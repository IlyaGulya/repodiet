use crate::model::LargeBlobInfo;

use super::selection;

/// ViewModel for large blobs view
pub struct BlobsViewModel {
    blobs: Vec<LargeBlobInfo>,
    selected_index: usize,
    total_cumulative: u64,
}

impl BlobsViewModel {
    pub fn new(blobs: Vec<LargeBlobInfo>, total_cumulative: u64) -> Self {
        Self {
            blobs,
            selected_index: 0,
            total_cumulative,
        }
    }

    pub fn blobs(&self) -> &[LargeBlobInfo] {
        &self.blobs
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn total_cumulative(&self) -> u64 {
        self.total_cumulative
    }

    pub fn total_blob_size(&self) -> u64 {
        self.blobs.iter().map(|b| b.size).sum()
    }

    pub fn move_up(&mut self) {
        selection::move_up(&mut self.selected_index, self.blobs.len());
    }

    pub fn move_down(&mut self) {
        selection::move_down(&mut self.selected_index, self.blobs.len());
    }

    /// Get selected blob's path
    pub fn selected_path(&self) -> Option<&str> {
        self.blobs.get(self.selected_index).map(|b| b.path.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation() {
        let blobs = vec![
            LargeBlobInfo {
                oid: "abc".into(),
                size: 1000,
                path: "a.png".to_string(),
                first_author: "alice".to_string(),
                first_date: 0,
            },
            LargeBlobInfo {
                oid: "def".into(),
                size: 500,
                path: "b.png".to_string(),
                first_author: "bob".to_string(),
                first_date: 0,
            },
        ];

        let mut vm = BlobsViewModel::new(blobs, 10000);

        assert_eq!(vm.selected_index(), 0);

        vm.move_down();
        assert_eq!(vm.selected_index(), 1);

        vm.move_down();
        assert_eq!(vm.selected_index(), 0); // Wrap

        vm.move_up();
        assert_eq!(vm.selected_index(), 1); // Wrap back
    }
}

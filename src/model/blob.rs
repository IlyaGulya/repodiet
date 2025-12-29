/// Large blob information for display in the large blob detective view
#[derive(Debug, Clone)]
pub struct LargeBlobInfo {
    pub oid: Vec<u8>,
    pub size: u64,
    pub path: String,
    pub first_author: String,
    pub first_date: i64,
}

/// Statistics aggregated by file extension
#[derive(Debug, Clone, Default)]
pub struct ExtensionStats {
    pub cumulative_size: u64,
    pub current_size: u64,
    pub file_count: u64,
}

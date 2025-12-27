/// A search result entry containing path and size information
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub cumulative_size: u64,
    pub current_size: u64,
}

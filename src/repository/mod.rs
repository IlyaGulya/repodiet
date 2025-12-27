mod database;
mod git_scanner;

pub use database::Database;
pub use git_scanner::GitScanner;

// Re-export the schema version for callers who need it
pub const SCHEMA_VERSION: &str = "4";

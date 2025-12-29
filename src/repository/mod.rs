mod database;
mod scanner;

pub use database::{BlobMetaRecord, BlobRecord, Database};
pub use scanner::GitScanner;

// Re-export the schema version for callers who need it
pub const SCHEMA_VERSION: &str = "8";

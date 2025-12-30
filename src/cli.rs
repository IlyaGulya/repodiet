use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "repodiet", about = "Analyze git repository growth")]
pub struct Cli {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub repo_path: PathBuf,

    /// Enable profiling mode (skips TUI, prints timing)
    #[arg(long)]
    pub profile: bool,
}

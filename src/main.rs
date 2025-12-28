mod model;
mod repository;
mod viewmodel;
mod view;
mod input;
mod util;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;

use input::map_key_to_intent;
use repository::{Database, GitScanner};
use util::format_size;
use viewmodel::{Action, AppViewModel, ViewMode};
use view::{render_tree, render_extension, render_search, render_blobs};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Check for --profile flag
    let profile_mode = args.iter().any(|a| a == "--profile");

    // Get repo path (first non-flag argument, or ".")
    let repo_path = args.iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .unwrap_or(".")
        .to_string();

    // Get cache directory - use temp dir for profile mode to get fresh scan
    let db_path = if profile_mode {
        let temp_dir = std::env::temp_dir().join("repodiet-profile");
        fs::create_dir_all(&temp_dir)?;
        temp_dir.join("profile.db")
    } else {
        let cache_dir = dirs::cache_dir()
            .context("Could not determine cache directory")?
            .join("repodiet");
        fs::create_dir_all(&cache_dir)?;

        // Generate unique index filename based on repo's absolute path
        let abs_repo_path = fs::canonicalize(&repo_path)
            .with_context(|| format!("Could not resolve path: {}", repo_path))?;
        let repo_name = abs_repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo");
        let mut hasher = DefaultHasher::new();
        abs_repo_path.hash(&mut hasher);
        let hash = hasher.finish();
        cache_dir.join(format!("{}_{:016x}.db", repo_name, hash))
    };

    // Delete existing DB in profile mode for fresh scan
    if profile_mode && db_path.exists() {
        fs::remove_file(&db_path)?;
    }

    eprintln!("Using index: {}", db_path.display());

    // Connect to database
    let db_path_str = db_path.to_str().context("Invalid path encoding")?;
    let db = Database::new(db_path_str).await?;
    db.init_schema().await?;

    // Scan repository
    let scanner = if profile_mode {
        GitScanner::profiling(&repo_path)
    } else {
        GitScanner::new(&repo_path)
    };
    let root = scanner.scan(&db).await?;

    eprintln!("Total cumulative: {}, Current: {}",
        format_size(root.cumulative_size),
        format_size(root.current_size));

    // In profile mode, exit after scanning
    if profile_mode {
        eprintln!("Profile mode: exiting after scan");
        return Ok(());
    }

    // Load large blobs
    let large_blobs = db.get_top_blobs(50).await?;
    eprintln!("Loaded {} large blobs for detective view", large_blobs.len());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create ViewModel
    let mut app = AppViewModel::new(root, large_blobs);

    // Main event loop
    loop {
        // Render
        terminal.draw(|f| {
            let area = f.area();
            match app.view_mode() {
                ViewMode::Tree => render_tree(f, &app.tree_vm, area),
                ViewMode::ByExtension => render_extension(f, &app.extension_vm, area),
                ViewMode::Search => render_search(f, &app.search_vm, area),
                ViewMode::LargeBlobs => render_blobs(f, &app.blobs_vm, area),
            }
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                let is_at_root = app.tree_vm.is_at_root();
                let is_search = app.is_search_mode();
                let input_mode = app.input_view_mode();

                if let Some(intent) = map_key_to_intent(&key, input_mode, is_search, is_at_root) {
                    match app.handle_intent(intent) {
                        Action::Quit => break,
                        Action::Redraw => {}
                    }
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}

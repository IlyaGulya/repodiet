use anyhow::{Context, Result};
use git2::{ObjectType, Oid, Repository};
use gix_hash::Kind as HashKind;
use gix_pack::{data, index};
use indicatif::{ProgressBar, ProgressStyle};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::model::TreeNode;
use super::Database;

/// Git repository scanner for extracting history statistics
pub struct GitScanner {
    repo_path: PathBuf,
    verbose: bool,
    profile: bool,
}

impl GitScanner {
    pub fn new(repo_path: &str) -> Self {
        Self {
            repo_path: PathBuf::from(repo_path),
            verbose: true,
            profile: false,
        }
    }

    /// Create a quiet scanner (no logging output, used by benchmarks)
    #[allow(dead_code)]
    pub fn quiet(repo_path: &str) -> Self {
        Self {
            repo_path: PathBuf::from(repo_path),
            verbose: false,
            profile: false,
        }
    }

    /// Create a profiling scanner (detailed timing output)
    pub fn profiling(repo_path: &str) -> Self {
        Self {
            repo_path: PathBuf::from(repo_path),
            verbose: true,
            profile: true,
        }
    }

    /// Scan repository and return tree, using database for caching
    pub async fn scan(&self, db: &Database) -> Result<TreeNode> {
        let total_start = Instant::now();

        if self.verbose {
            eprintln!("Opening repository at: {}", self.repo_path.display());
        }
        let phase_start = Instant::now();
        let repo = Repository::open(&self.repo_path)
            .context("Failed to open git repository")?;

        // Get current HEAD
        let head = repo.head()?.peel_to_commit()?;
        let head_oid = head.id().to_string();
        if self.profile {
            eprintln!("[PROFILE] Open repo + get HEAD: {:?}", phase_start.elapsed());
        }

        // Check if we have a cached index
        let cached_head = db.get_metadata("head_oid").await;

        if cached_head.as_ref() == Some(&head_oid) {
            if self.verbose {
                eprintln!("Index is up to date (HEAD: {}), loading from cache...", &head_oid[..8]);
            }
            return db.load_tree().await;
        }

        // Load compressed (on-disk) sizes from pack files
        let git_dir = self.repo_path.join(".git");
        if self.verbose {
            eprintln!("Loading compressed sizes from pack files...");
        }
        let phase_start = Instant::now();
        let compressed_sizes = load_all_compressed_sizes(&git_dir);
        if self.profile {
            eprintln!("[PROFILE] Load pack sizes ({} objects): {:?}", compressed_sizes.len(), phase_start.elapsed());
        } else if self.verbose {
            eprintln!("Loaded compressed sizes for {} objects", compressed_sizes.len());
        }

        // Get current blobs for "is_current" flag - track by PATH and OID
        if self.verbose {
            eprintln!("Scanning current HEAD for working tree...");
        }
        let phase_start = Instant::now();
        let head_tree = head.tree()?;
        let mut current_path_blobs: FxHashMap<String, (Oid, i64)> = FxHashMap::default();
        head_tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(ObjectType::Blob) {
                let path = if dir.is_empty() {
                    entry.name().unwrap_or("").to_string()
                } else {
                    format!("{}{}", dir, entry.name().unwrap_or(""))
                };
                let oid = entry.id();
                let size = get_compressed_size_by_oid(oid, &compressed_sizes, &git_dir);
                current_path_blobs.insert(path, (oid, size));
            }
            git2::TreeWalkResult::Ok
        })?;
        if self.profile {
            eprintln!("[PROFILE] Scan HEAD tree ({} files): {:?}", current_path_blobs.len(), phase_start.elapsed());
        } else if self.verbose {
            eprintln!("Found {} files in current HEAD", current_path_blobs.len());
        }

        // Collect commits
        if self.verbose {
            eprintln!("Collecting commits...");
        }
        let phase_start = Instant::now();
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)?;
        let all_commits: Vec<Oid> = revwalk.filter_map(|r| r.ok()).collect();
        if self.profile {
            eprintln!("[PROFILE] Revwalk ({} commits): {:?}", all_commits.len(), phase_start.elapsed());
        } else if self.verbose {
            eprintln!("Found {} total commits", all_commits.len());
        }

        // Filter to unscanned commits (bulk load for speed)
        let phase_start = Instant::now();
        let scanned_commits = db.load_scanned_commit_oids().await;
        let mut commits_to_scan = Vec::new();
        for oid in &all_commits {
            if !scanned_commits.contains(&oid.to_string()) {
                commits_to_scan.push(*oid);
            }
        }
        if self.profile {
            eprintln!("[PROFILE] Filter unscanned ({} of {} need scanning, {} cached): {:?}",
                commits_to_scan.len(), all_commits.len(), scanned_commits.len(), phase_start.elapsed());
        } else if self.verbose {
            eprintln!("{} commits need scanning", commits_to_scan.len());
        }

        if commits_to_scan.is_empty() {
            db.set_metadata("head_oid", &head_oid).await?;
            return db.load_tree().await;
        }

        // Scan new commits
        let pb = ProgressBar::new(commits_to_scan.len() as u64);
        if self.verbose && !self.profile {
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} Scanning: [{bar:50.cyan/blue}] {pos}/{len} ({per_sec})")
                    .unwrap()
                    .progress_chars("=>-"),
            );
        } else {
            pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        }

        // Load already-seen blobs into memory for fast lookup
        let phase_start = Instant::now();
        let seen_blob_strings = db.load_seen_blobs().await?;
        // Convert to Oid for faster lookups (avoids string allocations in hot loop)
        let mut seen_blobs: FxHashSet<Oid> = seen_blob_strings
            .iter()
            .filter_map(|s| Oid::from_str(s).ok())
            .collect();
        if self.profile {
            eprintln!("[PROFILE] Load seen blobs ({} blobs): {:?}", seen_blobs.len(), phase_start.elapsed());
        } else if self.verbose {
            eprintln!("Loaded {} previously seen blobs", seen_blobs.len());
        }

        let mut seen_trees: FxHashSet<(Oid, String)> = FxHashSet::default();
        let mut new_blobs: Vec<(Oid, String, i64, i64)> = Vec::new();
        let mut new_blob_metadata: Vec<(Oid, i64, String, String, i64)> = Vec::new();
        let mut seen_path_blobs: FxHashSet<(String, Oid)> = FxHashSet::default();

        let phase_start = Instant::now();
        for oid in &commits_to_scan {
            pb.inc(1);

            let commit = match repo.find_commit(*oid) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let tree = match commit.tree() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let commit_author = commit.author().name().unwrap_or("unknown").to_string();
            let commit_date = commit.time().seconds();

            scan_tree_recursive(
                &repo,
                tree.id(),
                "",
                &mut seen_trees,
                &mut seen_blobs,
                &mut seen_path_blobs,
                &current_path_blobs,
                &mut new_blobs,
                &mut new_blob_metadata,
                &compressed_sizes,
                &git_dir,
                &commit_author,
                commit_date,
            );
        }

        pb.finish_and_clear();
        if self.profile {
            eprintln!("[PROFILE] Scan {} commits (found {} new blobs, {} trees visited): {:?}",
                commits_to_scan.len(), new_blobs.len(), seen_trees.len(), phase_start.elapsed());
        } else if self.verbose {
            eprintln!("Found {} new blobs to index", new_blobs.len());
        }

        // Save to database
        if !new_blobs.is_empty() {
            if self.verbose {
                eprintln!("Updating database...");
            }
            let pb2 = ProgressBar::new(new_blobs.len() as u64);
            if self.verbose && !self.profile {
                pb2.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} Indexing: [{bar:50.cyan/blue}] {pos}/{len}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
            } else {
                pb2.set_draw_target(indicatif::ProgressDrawTarget::hidden());
            }

            // Convert Oid to String for DB storage
            let blobs_for_db: Vec<(String, String, i64, i64)> = new_blobs
                .iter()
                .map(|(oid, path, cum, cur)| (oid.to_string(), path.clone(), *cum, *cur))
                .collect();
            let phase_start = Instant::now();
            db.save_blobs(&blobs_for_db, Some(&pb2)).await?;
            pb2.finish_and_clear();
            if self.profile {
                eprintln!("[PROFILE] Save blobs to DB: {:?}", phase_start.elapsed());
            }

            if !new_blob_metadata.is_empty() {
                let pb3 = ProgressBar::new(new_blob_metadata.len() as u64);
                if self.verbose && !self.profile {
                    pb3.set_style(
                        ProgressStyle::default_bar()
                            .template("{spinner:.green} Indexing metadata: [{bar:50.cyan/blue}] {pos}/{len}")
                            .unwrap()
                            .progress_chars("=>-"),
                    );
                } else {
                    pb3.set_draw_target(indicatif::ProgressDrawTarget::hidden());
                }
                // Convert Oid to String for DB storage
                let metadata_for_db: Vec<(String, i64, String, String, i64)> = new_blob_metadata
                    .iter()
                    .map(|(oid, size, path, author, date)| (oid.to_string(), *size, path.clone(), author.clone(), *date))
                    .collect();
                let phase_start = Instant::now();
                db.save_blob_metadata(&metadata_for_db, Some(&pb3)).await?;
                pb3.finish_and_clear();
                if self.profile {
                    eprintln!("[PROFILE] Save blob metadata to DB: {:?}", phase_start.elapsed());
                }
            }

            // Mark commits as scanned
            let phase_start = Instant::now();
            let commit_oids: Vec<String> = commits_to_scan.iter().map(|o| o.to_string()).collect();
            db.mark_commits_scanned(&commit_oids).await?;
            if self.profile {
                eprintln!("[PROFILE] Mark commits scanned: {:?}", phase_start.elapsed());
            }
        }

        db.set_metadata("head_oid", &head_oid).await?;

        if self.profile {
            eprintln!("[PROFILE] TOTAL scanning time: {:?}", total_start.elapsed());
        }

        if self.verbose {
            eprintln!("Loading tree from database...");
        }
        let phase_start = Instant::now();
        let tree = db.load_tree().await;
        if self.profile {
            eprintln!("[PROFILE] Load tree from DB: {:?}", phase_start.elapsed());
        }
        tree
    }
}

fn scan_tree_recursive(
    repo: &Repository,
    tree_oid: Oid,
    path: &str,
    seen_trees: &mut FxHashSet<(Oid, String)>,
    seen_blobs: &mut FxHashSet<Oid>,
    seen_path_blobs: &mut FxHashSet<(String, Oid)>,
    current_path_blobs: &FxHashMap<String, (Oid, i64)>,
    new_blobs: &mut Vec<(Oid, String, i64, i64)>,
    new_blob_metadata: &mut Vec<(Oid, i64, String, String, i64)>,
    compressed_sizes: &FxHashMap<Oid, u64>,
    git_dir: &Path,
    commit_author: &str,
    commit_date: i64,
) {
    let key = (tree_oid, path.to_string());
    if !seen_trees.insert(key) {
        return;
    }

    let tree = match repo.find_tree(tree_oid) {
        Ok(t) => t,
        Err(_) => return,
    };

    for entry in tree.iter() {
        let entry_name = entry.name().unwrap_or("");
        let entry_path = if path.is_empty() {
            entry_name.to_string()
        } else {
            format!("{}/{}", path, entry_name)
        };

        match entry.kind() {
            Some(ObjectType::Blob) => {
                let blob_oid = entry.id();
                let path_blob_key = (entry_path.clone(), blob_oid);

                if !seen_path_blobs.insert(path_blob_key) {
                    continue;
                }

                let is_new_blob = seen_blobs.insert(blob_oid);
                let size = get_compressed_size_by_oid(blob_oid, compressed_sizes, git_dir);

                let current_size = current_path_blobs
                    .get(&entry_path)
                    .filter(|(head_oid, _)| *head_oid == blob_oid)
                    .map(|(_, s)| *s)
                    .unwrap_or(0);

                if is_new_blob {
                    new_blobs.push((blob_oid, entry_path.clone(), size, current_size));
                    new_blob_metadata.push((blob_oid, size, entry_path, commit_author.to_string(), commit_date));
                } else if current_size > 0 {
                    new_blobs.push((blob_oid, entry_path, 0, current_size));
                }
            }
            Some(ObjectType::Tree) => {
                scan_tree_recursive(
                    repo,
                    entry.id(),
                    &entry_path,
                    seen_trees,
                    seen_blobs,
                    seen_path_blobs,
                    current_path_blobs,
                    new_blobs,
                    new_blob_metadata,
                    compressed_sizes,
                    git_dir,
                    commit_author,
                    commit_date,
                );
            }
            _ => {}
        }
    }
}

/// Load compressed (on-disk) sizes for all objects in a pack file
fn load_pack_compressed_sizes(
    idx_path: &Path,
    pack_path: &Path,
) -> Result<FxHashMap<Oid, u64>> {
    let hash_kind = HashKind::Sha1;

    let idx = index::File::at(idx_path, hash_kind)?;
    let pack = data::File::at(pack_path, hash_kind)?;

    let mut entries: Vec<_> = idx.iter().collect();
    entries.sort_by_key(|e| e.pack_offset);

    let pack_end = pack.pack_end() as u64;
    let mut sizes = FxHashMap::default();
    sizes.reserve(entries.len());

    for (i, entry) in entries.iter().enumerate() {
        let entry_end = entries.get(i + 1)
            .map(|next| next.pack_offset)
            .unwrap_or(pack_end);

        let entry_size = entry_end - entry.pack_offset;
        // Convert gix ObjectId to git2 Oid (both are 20-byte SHA1)
        if let Ok(oid) = Oid::from_bytes(entry.oid.as_bytes()) {
            sizes.insert(oid, entry_size);
        }
    }

    Ok(sizes)
}

/// Load compressed sizes from all pack files in .git/objects/pack/
fn load_all_compressed_sizes(git_dir: &Path) -> FxHashMap<Oid, u64> {
    let mut all_sizes = FxHashMap::default();
    let pack_dir = git_dir.join("objects/pack");

    if let Ok(entries) = std::fs::read_dir(&pack_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "idx") {
                let pack_path = path.with_extension("pack");
                if pack_path.exists() {
                    match load_pack_compressed_sizes(&path, &pack_path) {
                        Ok(sizes) => {
                            all_sizes.extend(sizes);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load pack {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }

    all_sizes
}

/// Get compressed size for a loose object by reading file size
fn get_loose_object_size_by_oid(git_dir: &Path, oid: Oid) -> Option<u64> {
    let hex = oid.to_string();
    let path = git_dir
        .join("objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    std::fs::metadata(&path).ok().map(|m| m.len())
}

/// Get compressed (on-disk) size for a blob using Oid directly
fn get_compressed_size_by_oid(
    oid: Oid,
    compressed_sizes: &FxHashMap<Oid, u64>,
    git_dir: &Path,
) -> i64 {
    if let Some(&size) = compressed_sizes.get(&oid) {
        return size as i64;
    }
    if let Some(size) = get_loose_object_size_by_oid(git_dir, oid) {
        return size as i64;
    }
    0
}

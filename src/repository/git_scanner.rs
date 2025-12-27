use anyhow::{Context, Result};
use git2::{ObjectType, Oid, Repository};
use gix_hash::Kind as HashKind;
use gix_pack::{data, index};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::model::TreeNode;
use super::Database;

/// Git repository scanner for extracting history statistics
pub struct GitScanner {
    repo_path: PathBuf,
    verbose: bool,
}

impl GitScanner {
    pub fn new(repo_path: &str) -> Self {
        Self {
            repo_path: PathBuf::from(repo_path),
            verbose: true,
        }
    }

    /// Create a quiet scanner (no logging output, used by benchmarks)
    #[allow(dead_code)]
    pub fn quiet(repo_path: &str) -> Self {
        Self {
            repo_path: PathBuf::from(repo_path),
            verbose: false,
        }
    }

    /// Scan repository and return tree, using database for caching
    pub async fn scan(&self, db: &Database) -> Result<TreeNode> {
        if self.verbose {
            eprintln!("Opening repository at: {}", self.repo_path.display());
        }
        let repo = Repository::open(&self.repo_path)
            .context("Failed to open git repository")?;

        // Get current HEAD
        let head = repo.head()?.peel_to_commit()?;
        let head_oid = head.id().to_string();

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
        let compressed_sizes = load_all_compressed_sizes(&git_dir);
        if self.verbose {
            eprintln!("Loaded compressed sizes for {} objects", compressed_sizes.len());
        }

        // Get current blobs for "is_current" flag - track by PATH and OID
        if self.verbose {
            eprintln!("Scanning current HEAD for working tree...");
        }
        let head_tree = head.tree()?;
        let mut current_path_blobs: HashMap<String, (String, i64)> = HashMap::new();
        head_tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(ObjectType::Blob) {
                let path = if dir.is_empty() {
                    entry.name().unwrap_or("").to_string()
                } else {
                    format!("{}{}", dir, entry.name().unwrap_or(""))
                };
                let oid_hex = entry.id().to_string();
                let size = get_compressed_size(&oid_hex, &compressed_sizes, &git_dir);
                current_path_blobs.insert(path, (oid_hex, size));
            }
            git2::TreeWalkResult::Ok
        })?;
        if self.verbose {
            eprintln!("Found {} files in current HEAD", current_path_blobs.len());
        }

        // Collect commits
        if self.verbose {
            eprintln!("Collecting commits...");
        }
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)?;
        let all_commits: Vec<Oid> = revwalk.filter_map(|r| r.ok()).collect();
        if self.verbose {
            eprintln!("Found {} total commits", all_commits.len());
        }

        // Filter to unscanned commits
        let mut commits_to_scan = Vec::new();
        for oid in &all_commits {
            if !db.is_commit_scanned(&oid.to_string()).await {
                commits_to_scan.push(*oid);
            }
        }
        if self.verbose {
            eprintln!("{} commits need scanning", commits_to_scan.len());
        }

        if commits_to_scan.is_empty() {
            db.set_metadata("head_oid", &head_oid).await?;
            return db.load_tree().await;
        }

        // Scan new commits
        let pb = ProgressBar::new(commits_to_scan.len() as u64);
        if self.verbose {
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
        let mut seen_blobs = db.load_seen_blobs().await?;
        if self.verbose {
            eprintln!("Loaded {} previously seen blobs", seen_blobs.len());
        }

        let mut seen_trees: HashSet<(String, String)> = HashSet::new();
        let mut new_blobs: Vec<(String, String, i64, i64)> = Vec::new();
        let mut new_blob_metadata: Vec<(String, i64, String, String, i64)> = Vec::new();
        let mut seen_path_blobs: HashSet<(String, String)> = HashSet::new();

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
        if self.verbose {
            eprintln!("Found {} new blobs to index", new_blobs.len());
        }

        // Save to database
        if !new_blobs.is_empty() {
            if self.verbose {
                eprintln!("Updating database...");
            }
            let pb2 = ProgressBar::new(new_blobs.len() as u64);
            if self.verbose {
                pb2.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} Indexing: [{bar:50.cyan/blue}] {pos}/{len}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
            } else {
                pb2.set_draw_target(indicatif::ProgressDrawTarget::hidden());
            }

            db.save_blobs(&new_blobs, Some(&pb2)).await?;
            pb2.finish_and_clear();

            if !new_blob_metadata.is_empty() {
                let pb3 = ProgressBar::new(new_blob_metadata.len() as u64);
                if self.verbose {
                    pb3.set_style(
                        ProgressStyle::default_bar()
                            .template("{spinner:.green} Indexing metadata: [{bar:50.cyan/blue}] {pos}/{len}")
                            .unwrap()
                            .progress_chars("=>-"),
                    );
                } else {
                    pb3.set_draw_target(indicatif::ProgressDrawTarget::hidden());
                }
                db.save_blob_metadata(&new_blob_metadata, Some(&pb3)).await?;
                pb3.finish_and_clear();
            }

            // Mark commits as scanned
            let commit_oids: Vec<String> = commits_to_scan.iter().map(|o| o.to_string()).collect();
            db.mark_commits_scanned(&commit_oids).await?;
        }

        db.set_metadata("head_oid", &head_oid).await?;

        if self.verbose {
            eprintln!("Loading tree from database...");
        }
        db.load_tree().await
    }
}

fn scan_tree_recursive(
    repo: &Repository,
    tree_oid: Oid,
    path: &str,
    seen_trees: &mut HashSet<(String, String)>,
    seen_blobs: &mut HashSet<String>,
    seen_path_blobs: &mut HashSet<(String, String)>,
    current_path_blobs: &HashMap<String, (String, i64)>,
    new_blobs: &mut Vec<(String, String, i64, i64)>,
    new_blob_metadata: &mut Vec<(String, i64, String, String, i64)>,
    compressed_sizes: &HashMap<String, u64>,
    git_dir: &Path,
    commit_author: &str,
    commit_date: i64,
) {
    let key = (tree_oid.to_string(), path.to_string());
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
                let blob_oid = entry.id().to_string();
                let path_blob_key = (entry_path.clone(), blob_oid.clone());

                if !seen_path_blobs.insert(path_blob_key) {
                    continue;
                }

                let is_new_blob = seen_blobs.insert(blob_oid.clone());
                let size = get_compressed_size(&blob_oid, compressed_sizes, git_dir);

                let current_size = current_path_blobs
                    .get(&entry_path)
                    .filter(|(head_oid, _)| head_oid == &blob_oid)
                    .map(|(_, s)| *s)
                    .unwrap_or(0);

                if is_new_blob {
                    new_blobs.push((blob_oid.clone(), entry_path.clone(), size, current_size));
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
) -> Result<HashMap<String, u64>> {
    let hash_kind = HashKind::Sha1;

    let idx = index::File::at(idx_path, hash_kind)?;
    let pack = data::File::at(pack_path, hash_kind)?;

    let mut entries: Vec<_> = idx.iter().collect();
    entries.sort_by_key(|e| e.pack_offset);

    let pack_end = pack.pack_end() as u64;
    let mut sizes = HashMap::new();

    for (i, entry) in entries.iter().enumerate() {
        let entry_end = entries.get(i + 1)
            .map(|next| next.pack_offset)
            .unwrap_or(pack_end);

        let entry_size = entry_end - entry.pack_offset;
        let oid_hex = entry.oid.to_hex().to_string();
        sizes.insert(oid_hex, entry_size);
    }

    Ok(sizes)
}

/// Load compressed sizes from all pack files in .git/objects/pack/
fn load_all_compressed_sizes(git_dir: &Path) -> HashMap<String, u64> {
    let mut all_sizes = HashMap::new();
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
fn get_loose_object_size(git_dir: &Path, oid_hex: &str) -> Option<u64> {
    if oid_hex.len() < 3 {
        return None;
    }
    let path = git_dir
        .join("objects")
        .join(&oid_hex[..2])
        .join(&oid_hex[2..]);
    std::fs::metadata(&path).ok().map(|m| m.len())
}

/// Get compressed (on-disk) size for a blob
fn get_compressed_size(
    oid_hex: &str,
    compressed_sizes: &HashMap<String, u64>,
    git_dir: &Path,
) -> i64 {
    if let Some(&size) = compressed_sizes.get(oid_hex) {
        return size as i64;
    }
    if let Some(size) = get_loose_object_size(git_dir, oid_hex) {
        return size as i64;
    }
    0
}

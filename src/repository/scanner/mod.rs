//! Git repository scanner
//!
//! Scans git history to build a tree of blob sizes.
//!
//! # Architecture
//!
//! The scanner is organized into layers:
//!
//! - **types**: Domain types (PathId, BlobRow, ScanDelta, etc.)
//! - **interner**: Path interning for zero-allocation traversal
//! - **pack**: Pack file size index
//! - **tree**: Tree scanning context (replaces the 14-param recursive function)
//! - **progress**: Progress reporting abstraction
//! - **store**: Persistence layer trait
//! - **db_store**: Database implementation of ScanStore
//! - **scanner**: Main scanner orchestrator

mod db_store;
mod interner;
mod pack;
mod progress;
mod store;
mod tree;
mod types;

pub use interner::PathInterner;
pub use pack::PackSizeIndex;
pub use progress::{NoopProgress, ProgressHandle, ProgressReporter, VerboseProgress};
pub use store::ScanStore;
pub use tree::TreeScanCtx;
pub use types::{CommitInfo, HeadSnapshot, ScanDelta};

use anyhow::{Context, Result};
use gix::prelude::FindExt;
use gix::ObjectId;
use rustc_hash::FxHashSet;
use std::path::PathBuf;
use std::time::Instant;

use crate::model::TreeNode;

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

    /// Scan repository and return tree, using store for caching
    pub async fn scan(&self, store: &impl ScanStore) -> Result<TreeNode> {
        let total_start = Instant::now();
        let progress: Box<dyn ProgressReporter> = if self.profile {
            Box::new(NoopProgress)
        } else {
            Box::new(VerboseProgress::new(self.verbose))
        };

        // Phase 1: Open repository
        self.log("Opening repository...");
        let phase_start = Instant::now();
        let repo = gix::open(&self.repo_path).context("Failed to open git repository")?;

        let head_commit = repo.head_commit().context("Failed to get HEAD commit")?;
        let head_oid = head_commit.id();
        let head_hex = head_oid.to_hex().to_string();
        self.profile_phase("Open repo + get HEAD", phase_start);

        // Phase 2: Check cache
        if store.get_head_oid().await.as_deref() == Some(&head_hex) {
            self.log(&format!(
                "Index is up to date (HEAD: {}), loading from cache...",
                &head_hex[..8]
            ));
            return store.load_tree().await;
        }

        // Phase 3: Load pack sizes
        self.log("Loading compressed sizes from pack files...");
        let phase_start = Instant::now();
        let pack = PackSizeIndex::load(&repo);
        self.profile_phase(&format!("Load pack sizes ({} objects)", pack.len()), phase_start);

        // Phase 4: Build HEAD snapshot
        self.log("Scanning current HEAD for working tree...");
        let phase_start = Instant::now();
        let mut interner = PathInterner::new();
        let head_tree = head_commit.tree().context("Failed to get HEAD tree")?;
        let head_snapshot = self.build_head_snapshot(&head_tree, &head_hex, &pack, &mut interner)?;
        self.profile_phase(
            &format!("Scan HEAD tree ({} files)", head_snapshot.blobs_by_path.len()),
            phase_start,
        );

        // Phase 5: Collect commits via revwalk
        self.log("Collecting commits...");
        let phase_start = Instant::now();
        let all_commits = self.collect_commits(&repo, head_oid.into())?;
        self.profile_phase(&format!("Revwalk ({} commits)", all_commits.len()), phase_start);

        // Phase 6: Filter to unscanned commits
        let phase_start = Instant::now();
        let scanned_commits = store.load_scanned_commits().await;
        let commits_to_scan = self.plan_commits(&all_commits, &scanned_commits);
        self.profile_phase(
            &format!(
                "Filter unscanned ({} of {} need scanning, {} cached)",
                commits_to_scan.len(),
                all_commits.len(),
                scanned_commits.len()
            ),
            phase_start,
        );

        if commits_to_scan.is_empty() {
            store.set_head_oid(&head_hex).await?;
            return store.load_tree().await;
        }

        self.log(&format!("{} commits need scanning", commits_to_scan.len()));

        // Phase 7: Load seen blobs
        let phase_start = Instant::now();
        let seen_blobs = store.load_seen_blobs().await?;
        self.profile_phase(&format!("Load seen blobs ({} blobs)", seen_blobs.len()), phase_start);

        // Phase 8: Scan commits
        let phase_start = Instant::now();
        let delta = self.scan_commits(
            &repo,
            &pack,
            &head_snapshot,
            &mut interner,
            seen_blobs,
            &commits_to_scan,
            progress.as_ref(),
        )?;
        self.profile_phase(
            &format!(
                "Scan {} commits (found {} new blobs)",
                commits_to_scan.len(),
                delta.blobs.len()
            ),
            phase_start,
        );

        // Phase 9: Apply scan atomically (rows + scanned commits)
        let phase_start = Instant::now();
        store
            .apply_scan(&delta, &commits_to_scan, &interner, progress.as_ref())
            .await?;
        self.profile_phase(
            &format!("Apply scan ({} commits)", commits_to_scan.len()),
            phase_start,
        );

        store.set_head_oid(&head_hex).await?;

        if self.profile {
            eprintln!("[PROFILE] TOTAL scanning time: {:?}", total_start.elapsed());
        }

        // Phase 11: Load tree
        self.log("Loading tree from database...");
        let phase_start = Instant::now();
        let tree = store.load_tree().await;
        self.profile_phase("Load tree from DB", phase_start);
        tree
    }

    /// Build a snapshot of HEAD tree
    fn build_head_snapshot(
        &self,
        head_tree: &gix::Tree<'_>,
        head_hex: &str,
        pack: &PackSizeIndex,
        interner: &mut PathInterner,
    ) -> Result<HeadSnapshot> {
        let mut recorder = gix::traverse::tree::Recorder::default();
        head_tree.traverse().breadthfirst(&mut recorder)?;

        let mut blobs_by_path = rustc_hash::FxHashMap::default();
        for entry in recorder.records {
            if entry.mode.is_blob() {
                let path_id = interner.intern(entry.filepath.as_ref());
                let size = pack.size_of(entry.oid);
                blobs_by_path.insert(path_id, (entry.oid, size));
            }
        }

        Ok(HeadSnapshot {
            head_oid_hex: head_hex.to_string(),
            blobs_by_path,
        })
    }

    /// Collect all commits via revwalk (oldest first)
    fn collect_commits(&self, repo: &gix::Repository, head: ObjectId) -> Result<Vec<ObjectId>> {
        let mut commits: Vec<ObjectId> = Vec::new();
        let walk = repo.rev_walk([head]);
        for commit_info in walk.all()? {
            let commit_info = commit_info?;
            commits.push(commit_info.id);
        }
        // Reverse to process oldest first
        commits.reverse();
        Ok(commits)
    }

    /// Filter commits to those not yet scanned
    fn plan_commits(
        &self,
        all_commits: &[ObjectId],
        scanned: &FxHashSet<[u8; 20]>,
    ) -> Vec<ObjectId> {
        all_commits
            .iter()
            .filter(|oid| !scanned.contains(oid.as_bytes()))
            .copied()
            .collect()
    }

    /// Scan commits and return delta
    fn scan_commits(
        &self,
        repo: &gix::Repository,
        pack: &PackSizeIndex,
        head: &HeadSnapshot,
        interner: &mut PathInterner,
        seen_blobs: FxHashSet<ObjectId>,
        commits: &[ObjectId],
        progress: &dyn ProgressReporter,
    ) -> Result<ScanDelta> {
        let odb = repo.objects.clone();
        let mut ctx = TreeScanCtx::new(&odb, pack, head, interner, seen_blobs);

        let pb = progress.start("Scanning", commits.len() as u64);

        for oid in commits {
            pb.inc(1);

            let mut commit_buf = Vec::new();
            let commit = match odb.find_commit(oid, &mut commit_buf) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let tree_id = commit.tree();
            let author_sig = match commit.author() {
                Ok(sig) => sig,
                Err(_) => continue,
            };

            let commit_info = CommitInfo {
                oid: *oid,
                tree: tree_id,
                author: author_sig.name.to_string(),
                timestamp: author_sig.seconds(),
            };

            ctx.scan_commit(&commit_info);
        }

        pb.finish();

        if self.profile {
            eprintln!(
                "[PROFILE] Trees visited: {}, blobs found: {}",
                ctx.trees_visited(),
                ctx.blobs_found()
            );
        }

        Ok(ctx.finish())
    }

    fn log(&self, msg: &str) {
        if self.verbose {
            eprintln!("{}", msg);
        }
    }

    fn profile_phase(&self, name: &str, start: Instant) {
        if self.profile {
            eprintln!("[PROFILE] {}: {:?}", name, start.elapsed());
        }
    }
}

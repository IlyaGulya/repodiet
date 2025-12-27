// Git scanner benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use criterion::async_executor::AsyncExecutor;
use repodiet::repository::{Database, GitScanner};
use tokio::runtime::Runtime;
use tempfile::TempDir;

mod common;

struct TokioExecutor(Runtime);

impl AsyncExecutor for TokioExecutor {
    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}

/// Create a database in a temp directory
async fn create_db_in_dir(dir: &TempDir) -> Database {
    let db_path = dir.path().join("bench.db");
    let db = Database::new(db_path.to_str().unwrap()).await.unwrap();
    db.init_schema().await.unwrap();
    db
}

fn bench_scan_small_repo(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanner_small_repo");
    group.sample_size(10); // Fewer samples for slower benchmarks

    // Create repo with 50 commits, ~200 files (realistic small project)
    let (dir, repo_path, repo) = common::create_bench_repo();

    // Generate initial files
    let files: Vec<_> = (0..200)
        .map(|i| {
            let dir_num = i / 20;
            let path = format!("src/dir_{}/file_{}.rs", dir_num, i);
            let content = format!("// File {}\nfn func_{}() {{}}\n", i, i);
            (path, content.into_bytes())
        })
        .collect();

    // Create initial commit
    let file_refs: Vec<_> = files.iter()
        .map(|(p, c)| (p.as_str(), c.as_slice()))
        .collect();
    common::add_commit(&repo, &file_refs, "Initial commit");

    // Create 49 more commits with modifications
    for commit_num in 1..50 {
        let modified_files: Vec<_> = (0..10)
            .map(|i| {
                let file_idx = (commit_num * 10 + i) % 200;
                let dir_num = file_idx / 20;
                let path = format!("src/dir_{}/file_{}.rs", dir_num, file_idx);
                let content = format!("// File {} version {}\nfn func_{}() {{ /* v{} */ }}\n",
                    file_idx, commit_num, file_idx, commit_num);
                (path, content.into_bytes())
            })
            .collect();

        let file_refs: Vec<_> = modified_files.iter()
            .map(|(p, c)| (p.as_str(), c.as_slice()))
            .collect();
        common::add_commit(&repo, &file_refs, &format!("Commit {}", commit_num));
    }

    group.bench_function("50_commits_200_files", |b| {
        b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| async {
            // Create fresh database for each iteration
            let db = create_db_in_dir(&dir).await;
            let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
            black_box(scanner.scan(&db).await.unwrap())
        });
    });

    group.finish();
}

fn bench_scan_medium_repo(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanner_medium_repo");
    group.sample_size(10); // Fewer samples for slower benchmarks

    // Create repo with 200 commits, ~500 files (realistic medium project)
    let (dir, repo_path, repo) = common::create_bench_repo();

    // Generate initial files
    let files: Vec<_> = (0..500)
        .map(|i| {
            let dir_num = i / 25;
            let path = format!("src/dir_{}/file_{}.rs", dir_num, i);
            let content = format!("// File {}\nfn func_{}() {{}}\n", i, i);
            (path, content.into_bytes())
        })
        .collect();

    // Create initial commit
    let file_refs: Vec<_> = files.iter()
        .map(|(p, c)| (p.as_str(), c.as_slice()))
        .collect();
    common::add_commit(&repo, &file_refs, "Initial commit");

    // Create 199 more commits with modifications
    for commit_num in 1..200 {
        let modified_files: Vec<_> = (0..10)
            .map(|i| {
                let file_idx = (commit_num * 10 + i) % 500;
                let dir_num = file_idx / 25;
                let path = format!("src/dir_{}/file_{}.rs", dir_num, file_idx);
                let content = format!("// File {} version {}\nfn func_{}() {{ /* v{} */ }}\n",
                    file_idx, commit_num, file_idx, commit_num);
                (path, content.into_bytes())
            })
            .collect();

        let file_refs: Vec<_> = modified_files.iter()
            .map(|(p, c)| (p.as_str(), c.as_slice()))
            .collect();
        common::add_commit(&repo, &file_refs, &format!("Commit {}", commit_num));
    }

    group.bench_function("200_commits_500_files", |b| {
        b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| async {
            // Create fresh database for each iteration
            let db = create_db_in_dir(&dir).await;
            let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
            black_box(scanner.scan(&db).await.unwrap())
        });
    });

    group.finish();
}

fn bench_incremental_scan(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("scanner_incremental");
    group.sample_size(10);

    // Create repo with 100 commits, 200 files (realistic incremental scenario)
    let (dir, repo_path, repo) = common::create_bench_repo();

    // Generate initial files
    let files: Vec<_> = (0..200)
        .map(|i| {
            let dir_num = i / 20;
            let path = format!("src/dir_{}/file_{}.rs", dir_num, i);
            let content = format!("// File {}\n", i);
            (path, content.into_bytes())
        })
        .collect();

    // Create initial commit
    let file_refs: Vec<_> = files.iter()
        .map(|(p, c)| (p.as_str(), c.as_slice()))
        .collect();
    common::add_commit(&repo, &file_refs, "Initial commit");

    // Create 99 more commits with modifications
    for commit_num in 1..100 {
        let modified_files: Vec<_> = (0..5)
            .map(|i| {
                let file_idx = (commit_num * 5 + i) % 200;
                let dir_num = file_idx / 20;
                let path = format!("src/dir_{}/file_{}.rs", dir_num, file_idx);
                let content = format!("// v{}\n", commit_num);
                (path, content.into_bytes())
            })
            .collect();

        let file_refs: Vec<_> = modified_files.iter()
            .map(|(p, c)| (p.as_str(), c.as_slice()))
            .collect();
        common::add_commit(&repo, &file_refs, &format!("Commit {}", commit_num));
    }

    // Do initial full scan
    let db = rt.block_on(async {
        let db = create_db_in_dir(&dir).await;
        let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
        scanner.scan(&db).await.unwrap();
        db
    });

    // Add 10 new commits (simulates daily development)
    for i in 100..110 {
        let path = format!("src/new_file_{}.rs", i);
        let content = format!("// New file {}\n", i);
        common::add_commit(&repo, &[(&path, content.as_bytes())], &format!("New commit {}", i));
    }

    // Benchmark incremental scan (should be much faster than full scan)
    group.bench_function("10_new_commits_after_100", |b| {
        b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| async {
            let scanner = GitScanner::quiet(repo_path.to_str().unwrap());
            black_box(scanner.scan(&db).await.unwrap())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scan_small_repo,
    bench_scan_medium_repo,
    bench_incremental_scan
);
criterion_main!(benches);

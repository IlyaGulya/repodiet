// Database operation benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use criterion::async_executor::AsyncExecutor;
use tokio::runtime::Runtime;

mod common;

struct TokioExecutor(Runtime);

impl AsyncExecutor for TokioExecutor {
    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}

fn bench_save_blobs(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_save_blobs");
    for size in [1_000, 10_000, 50_000] {
        let blobs = common::generate_blobs(size);

        group.bench_with_input(
            BenchmarkId::new("blobs", size),
            &blobs,
            |b, blobs| {
                b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| async {
                    let db = common::setup_bench_db().await;
                    db.save_blobs(blobs, None).await.unwrap();
                    black_box(db)
                });
            },
        );
    }
    group.finish();
}

fn bench_load_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_load_tree");
    for size in [1_000, 10_000, 50_000] {
        let blobs = common::generate_blobs(size);

        group.bench_with_input(
            BenchmarkId::new("paths", size),
            &blobs,
            |b, blobs| {
                b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| {
                    let blobs = blobs.clone();
                    async move {
                        let db = common::setup_bench_db().await;
                        db.save_blobs(&blobs, None).await.unwrap();
                        black_box(db.load_tree().await.unwrap())
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_get_top_blobs(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_get_top_blobs");
    // Test with realistic blob metadata count
    for limit in [50, 100, 500] {
        let metadata = common::generate_blob_metadata(50_000);

        group.bench_with_input(
            BenchmarkId::new("limit", limit),
            &limit,
            |b, &limit| {
                b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| {
                    let metadata = metadata.clone();
                    async move {
                        let db = common::setup_bench_db().await;
                        db.save_blob_metadata(&metadata, None).await.unwrap();
                        black_box(db.get_top_blobs(limit).await.unwrap())
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_is_commit_scanned(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_is_commit_scanned");
    let commits: Vec<String> = (0..1000).map(|i| format!("commit_{:08x}", i)).collect();

    // Benchmark lookup of existing commit
    group.bench_function("lookup_existing", |b| {
        let commits = commits.clone();
        b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| {
            let commits = commits.clone();
            async move {
                let db = common::setup_bench_db().await;
                db.mark_commits_scanned(&commits).await.unwrap();
                black_box(db.is_commit_scanned("commit_00000100").await)
            }
        });
    });

    // Benchmark lookup of non-existing commit
    group.bench_function("lookup_missing", |b| {
        let commits = commits.clone();
        b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| {
            let commits = commits.clone();
            async move {
                let db = common::setup_bench_db().await;
                db.mark_commits_scanned(&commits).await.unwrap();
                black_box(db.is_commit_scanned("nonexistent").await)
            }
        });
    });

    group.finish();
}

fn bench_save_blob_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_save_blob_metadata");
    // Real repos can have many large blobs
    for size in [1_000, 10_000, 50_000] {
        let metadata = common::generate_blob_metadata(size);

        group.bench_with_input(
            BenchmarkId::new("entries", size),
            &metadata,
            |b, metadata| {
                b.to_async(TokioExecutor(Runtime::new().unwrap())).iter(|| async {
                    let db = common::setup_bench_db().await;
                    db.save_blob_metadata(metadata, None).await.unwrap();
                    black_box(db)
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_save_blobs,
    bench_load_tree,
    bench_get_top_blobs,
    bench_is_commit_scanned,
    bench_save_blob_metadata
);
criterion_main!(benches);

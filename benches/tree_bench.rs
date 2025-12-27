// Tree building benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use repodiet::model::TreeNode;

mod common;

fn bench_add_path_with_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_add_path");
    for size in [1_000, 10_000, 50_000] {
        group.bench_with_input(
            BenchmarkId::new("paths", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut root = TreeNode::new("(root)");
                    for i in 0..size {
                        let dir = format!("dir_{}", i / 100);
                        let file = format!("file_{}.rs", i);
                        root.add_path_with_sizes(
                            &["src", &dir, &file],
                            100, 50, 1
                        );
                    }
                    black_box(root)
                });
            },
        );
    }
    group.finish();
}

fn bench_compute_totals(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_compute_totals");
    for size in [1_000, 10_000, 50_000] {
        // Pre-generate tree without computing totals
        let mut tree = TreeNode::new("(root)");
        for i in 0..size {
            let dir = format!("dir_{}", i / 100);
            let file = format!("file_{}.rs", i);
            tree.add_path_with_sizes(&["src", &dir, &file], 100, 50, 1);
        }

        group.bench_with_input(
            BenchmarkId::new("nodes", size),
            &tree,
            |b, tree| {
                b.iter(|| {
                    let mut clone = tree.clone();
                    clone.compute_totals();
                    black_box(clone)
                });
            },
        );
    }
    group.finish();
}

fn bench_contains_deleted_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_contains_deleted");
    for size in [1_000, 10_000, 50_000] {
        // Generate tree with NO deleted files (worst case - must check entire tree)
        // This benchmarks full traversal since any() won't short-circuit
        let tree = common::generate_tree_with_deletions(size, 0.0);

        group.bench_with_input(
            BenchmarkId::new("nodes", size),
            &tree,
            |b, tree| {
                b.iter(|| {
                    // black_box the input to prevent compiler from predicting result
                    black_box(black_box(tree).contains_deleted_files())
                });
            },
        );
    }
    group.finish();
}

fn bench_deleted_cumulative_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_deleted_size");
    for size in [1_000, 10_000, 50_000] {
        // Generate tree with 30% deleted files
        let tree = common::generate_tree_with_deletions(size, 0.3);

        group.bench_with_input(
            BenchmarkId::new("nodes", size),
            &tree,
            |b, tree| {
                b.iter(|| {
                    black_box(tree.deleted_cumulative_size())
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_add_path_with_sizes,
    bench_compute_totals,
    bench_contains_deleted_files,
    bench_deleted_cumulative_size
);
criterion_main!(benches);

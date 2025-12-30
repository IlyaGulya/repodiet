// Search performance benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use repodiet::viewmodel::SearchViewModel;

mod common;

fn bench_search_add_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_add_char");
    for size in [1_000, 10_000, 50_000] {
        let tree = common::generate_tree_arc(size);

        group.bench_with_input(
            BenchmarkId::new("files", size),
            &tree,
            |b, tree| {
                b.iter(|| {
                    let mut vm = SearchViewModel::new(tree.clone());
                    // Simulate typing ".rs" incrementally
                    vm.add_char('.');
                    vm.add_char('r');
                    vm.add_char('s');
                    black_box(vm.results().count())
                });
            },
        );
    }
    group.finish();
}

fn bench_search_full_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_full_query");
    for size in [1_000, 10_000, 50_000] {
        let tree = common::generate_tree_arc(size);

        group.bench_with_input(
            BenchmarkId::new("files", size),
            &tree,
            |b, tree| {
                b.iter(|| {
                    let mut vm = SearchViewModel::new(tree.clone());
                    // Type a longer query
                    for c in "file_500".chars() {
                        vm.add_char(c);
                    }
                    black_box(vm.results().count())
                });
            },
        );
    }
    group.finish();
}

fn bench_search_results_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_results_sort");
    // Test with realistic result counts
    for result_target in [100, 1000, 5000] {
        // Generate tree with many matching files
        let tree = common::generate_tree_arc(result_target * 10);

        group.bench_with_input(
            BenchmarkId::new("results", result_target),
            &tree,
            |b, tree| {
                b.iter(|| {
                    let mut vm = SearchViewModel::new(tree.clone());
                    // Search for common extension to get many results
                    vm.add_char('.');
                    vm.add_char('r');
                    vm.add_char('s');
                    black_box(vm.results().count())
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_search_add_char,
    bench_search_full_query,
    bench_search_results_sort
);
criterion_main!(benches);

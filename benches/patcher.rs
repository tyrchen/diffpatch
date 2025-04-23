use divan::{Bencher, black_box};
use patcher::{DiffAlgorithm, Differ, PatchAlgorithm, Patcher, PatcherAlgorithm};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// Define the fixture pairs to benchmark (same as differ.rs)
const FIXTURE_PAIRS: &[(&str, &str)] = &[
    ("simple_before.rs", "simple_after.rs"),
    ("complex_before.rs", "complex_after.rs"),
];

// Helper to load fixture files relative to crate root
pub(crate) fn load_fixture(name: &str) -> String {
    let path = format!("fixtures/code/{}", name);
    std::fs::read_to_string(path).unwrap()
}

// Benchmark applying the patch forward
#[divan::bench(args = [0, 1], name = "naive_forward")]
fn naive_patcher_apply_forward(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original_content = load_fixture(pair.0);
    let new_content = load_fixture(pair.1);

    // Pre-generate the patch outside the benchmark loop
    let patch = Differ::new(&original_content, &new_content).generate();
    let patcher = Patcher::new_with_algorithm(patch, PatcherAlgorithm::Naive);

    bencher
        .with_inputs(|| original_content.clone()) // Clone original content for each run
        .bench_values(|content| {
            // Black box the patcher and content to prevent optimizations
            black_box(patcher.apply(black_box(&content), false))
        });
}

// Benchmark applying the patch in reverse
#[divan::bench(args = [0, 1], name = "naive_reverse")]
fn naive_patcher_apply_reverse(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original_content = load_fixture(pair.0);
    let new_content = load_fixture(pair.1);

    // Pre-generate the patch outside the benchmark loop
    let patch = Differ::new(&original_content, &new_content).generate();
    let patcher = Patcher::new_with_algorithm(patch, PatcherAlgorithm::Naive);

    bencher
        .with_inputs(|| new_content.clone()) // Clone new content for each run
        .bench_values(|content| {
            // Black box the patcher and content to prevent optimizations
            black_box(patcher.apply(black_box(&content), true))
        });
}

#[divan::bench(args = [0, 1], name = "similar_forward")]
fn similar_patcher_apply_forward(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original_content = load_fixture(pair.0);
    let new_content = load_fixture(pair.1);

    let patch = Differ::new(&original_content, &new_content).generate();
    let patcher = Patcher::new_with_algorithm(patch, PatcherAlgorithm::Similar);

    bencher
        .with_inputs(|| original_content.clone())
        .bench_values(|content| black_box(patcher.apply(black_box(&content), false)));
}

#[divan::bench(args = [0, 1], name = "similar_reverse")]
fn similar_patcher_apply_reverse(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original_content = load_fixture(pair.0);
    let new_content = load_fixture(pair.1);

    let patch = Differ::new(&original_content, &new_content).generate();
    let patcher = Patcher::new_with_algorithm(patch, PatcherAlgorithm::Similar);

    bencher
        .with_inputs(|| new_content.clone())
        .bench_values(|content| black_box(patcher.apply(black_box(&content), true)));
}

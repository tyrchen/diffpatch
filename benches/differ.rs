use diffpatch::{
    differ::{DiffAlgorithmType, Differ},
    DiffAlgorithm,
};
use divan::{black_box, Bencher};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// Define the fixture pairs to benchmark
const FIXTURE_PAIRS: &[(&str, &str)] = &[
    ("simple_before.rs", "simple_after.rs"),
    ("complex_before.rs", "complex_after.rs"),
];

pub(crate) fn load_fixture(name: &str) -> String {
    let path = format!("fixtures/code/{}", name);
    std::fs::read_to_string(path).unwrap()
}

// Myers algorithm benchmarks
#[divan::bench(args = [0, 1], name = "myers")]
fn myers_algorithm(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original = load_fixture(pair.0);
    let modified = load_fixture(pair.1);

    bencher
        .with_inputs(|| (original.clone(), modified.clone()))
        .bench_refs(|(original, modified)| {
            let differ = Differ::new_with_algorithm(
                black_box(original),
                black_box(modified),
                DiffAlgorithmType::Myers,
            );
            black_box(differ.generate())
        });
}

// Naive algorithm benchmarks
#[divan::bench(args = [0, 1], name = "naive")]
fn naive_algorithm(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original = load_fixture(pair.0);
    let modified = load_fixture(pair.1);

    bencher
        .with_inputs(|| (original.clone(), modified.clone()))
        .bench_refs(|(original, modified)| {
            let differ = Differ::new_with_algorithm(
                black_box(original),
                black_box(modified),
                DiffAlgorithmType::Naive,
            );
            black_box(differ.generate())
        });
}

// XDiff algorithm benchmarks
#[divan::bench(args = [0, 1], name = "xdiff")]
fn xdiff_algorithm(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original = load_fixture(pair.0);
    let modified = load_fixture(pair.1);

    bencher
        .with_inputs(|| (original.clone(), modified.clone()))
        .bench_refs(|(original, modified)| {
            let differ = Differ::new_with_algorithm(
                black_box(original),
                black_box(modified),
                DiffAlgorithmType::XDiff,
            );
            black_box(differ.generate())
        });
}

// Similar algorithm benchmarks
#[divan::bench(args = [0, 1], name = "similar")]
fn similar_algorithm(bencher: Bencher, index: usize) {
    let pair = FIXTURE_PAIRS[index];
    let original = load_fixture(pair.0);
    let modified = load_fixture(pair.1);

    bencher
        .with_inputs(|| (original.clone(), modified.clone()))
        .bench_refs(|(original, modified)| {
            let differ = Differ::new_with_algorithm(
                black_box(original),
                black_box(modified),
                DiffAlgorithmType::Similar,
            );
            black_box(differ.generate())
        });
}

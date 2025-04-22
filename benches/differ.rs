use diffpatch::differ::{DiffAlgorithmType, Differ};
use divan::{black_box, Bencher};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

const TEXT_SIZES: &[usize] = &[
    1_000,   // Medium texts
    10_000,  // Large texts
    100_000, // Very large texts
];

fn generate_texts(size: usize, change_percentage: f64) -> (String, String) {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 \n\t";
    let mut rng = fastrand::Rng::with_seed(42);

    // Generate original text
    let original: String = (0..size)
        .map(|_| {
            let idx = rng.usize(0..alphabet.len());
            alphabet.chars().nth(idx).unwrap()
        })
        .collect();

    // Generate modified text by changing some characters
    let change_count = (size as f64 * change_percentage).round() as usize;
    let original_chars: Vec<char> = original.chars().collect();
    let mut modified_chars = original_chars.clone();

    for _ in 0..change_count {
        let pos = rng.usize(0..modified_chars.len());
        let idx = rng.usize(0..alphabet.len());
        let new_char = alphabet.chars().nth(idx).unwrap();
        modified_chars[pos] = new_char;
    }

    let modified: String = modified_chars.into_iter().collect();

    (original, modified)
}

// Myers algorithm benchmarks
#[divan::bench(args = TEXT_SIZES)]
fn myers_algorithm(bencher: Bencher, size: usize) {
    let (original, modified) = generate_texts(size, 0.25);

    bencher.bench(|| {
        let differ = Differ::new(
            black_box(&original),
            black_box(&modified),
            DiffAlgorithmType::Myers,
        );
        black_box(differ.generate())
    });
}

// Naive algorithm benchmarks
#[divan::bench(args = TEXT_SIZES)]
fn naive_algorithm(bencher: Bencher, size: usize) {
    let (original, modified) = generate_texts(size, 0.25);

    bencher.bench(|| {
        let differ = Differ::new(
            black_box(&original),
            black_box(&modified),
            DiffAlgorithmType::Naive,
        );
        black_box(differ.generate())
    });
}

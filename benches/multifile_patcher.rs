use diffpatch::{
    differ::{DiffAlgorithmType, Differ},
    MultifilePatch,
};
use divan::{black_box, Bencher};
use std::collections::HashMap;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

const FILE_COUNTS: &[usize] = &[1, 5, 10, 50, 100];
const TEXT_SIZE: usize = 1000;
const CHANGE_PERCENTAGE: f64 = 0.05;

// For benchmark purposes, we need to simulate the file patching process without actual filesystem
fn simulate_multifile_patch(
    files: &HashMap<String, String>,
    patches: &Vec<diffpatch::Patch>,
) -> Vec<diffpatch::PatchedFile> {
    let mut result = Vec::new();

    for patch in patches {
        if let Some(content) = files.get(&patch.old_file) {
            let patcher = diffpatch::Patcher::new(patch.clone());
            if let Ok(new_content) = patcher.apply(content, false) {
                result.push(diffpatch::PatchedFile {
                    path: patch.new_file.clone(),
                    content: new_content,
                });
            }
        }
    }

    result
}

fn generate_multi_file_data(file_count: usize) -> (HashMap<String, String>, MultifilePatch) {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 \n\t";
    let mut rng = fastrand::Rng::with_seed(42);

    let mut original_files = HashMap::new();
    let mut patches = Vec::new();

    for i in 0..file_count {
        let filename = format!("file_{}.txt", i);

        // Generate original text
        let original: String = (0..TEXT_SIZE)
            .map(|_| {
                let idx = rng.usize(0..alphabet.len());
                alphabet.chars().nth(idx).unwrap()
            })
            .collect();

        // Generate modified text by changing some characters
        let change_count = (TEXT_SIZE as f64 * CHANGE_PERCENTAGE).round() as usize;
        let original_chars: Vec<char> = original.chars().collect();
        let mut modified_chars = original_chars.clone();

        for _ in 0..change_count {
            let pos = rng.usize(0..modified_chars.len());
            let idx = rng.usize(0..alphabet.len());
            let new_char = alphabet.chars().nth(idx).unwrap();
            modified_chars[pos] = new_char;
        }

        let modified: String = modified_chars.into_iter().collect();

        // Create patch
        let differ = Differ::new(&original, &modified, DiffAlgorithmType::Myers);
        let mut patch = differ.generate();

        // Set file paths
        patch.old_file = filename.clone();
        patch.new_file = filename.clone();

        patches.push(patch);

        // Add original file to the map
        original_files.insert(filename, original);
    }

    (original_files, MultifilePatch { patches })
}

// Benchmark for multifile patching with varying number of files
#[divan::bench(args = FILE_COUNTS)]
fn multi_patch_application(bencher: Bencher, file_count: usize) {
    let (files, patch) = generate_multi_file_data(file_count);

    bencher.bench(|| {
        black_box(simulate_multifile_patch(
            black_box(&files),
            black_box(&patch.patches),
        ))
    });
}

// Benchmark for creating multipatch from individual diffs
#[divan::bench(args = FILE_COUNTS)]
fn multi_patch_creation(bencher: Bencher, file_count: usize) {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 \n\t";
    let mut rng = fastrand::Rng::with_seed(42);

    // Pre-generate all file content pairs
    let mut file_pairs = Vec::with_capacity(file_count);

    for i in 0..file_count {
        let filename = format!("file_{}.txt", i);

        // Generate original text
        let original: String = (0..TEXT_SIZE)
            .map(|_| {
                let idx = rng.usize(0..alphabet.len());
                alphabet.chars().nth(idx).unwrap()
            })
            .collect();

        // Generate modified text
        let change_count = (TEXT_SIZE as f64 * CHANGE_PERCENTAGE).round() as usize;
        let original_chars: Vec<char> = original.chars().collect();
        let mut modified_chars = original_chars.clone();

        for _ in 0..change_count {
            let pos = rng.usize(0..modified_chars.len());
            let idx = rng.usize(0..alphabet.len());
            let new_char = alphabet.chars().nth(idx).unwrap();
            modified_chars[pos] = new_char;
        }

        let modified: String = modified_chars.into_iter().collect();

        file_pairs.push((filename, original, modified));
    }

    bencher.bench(|| {
        let mut patches = Vec::new();

        for (filename, original, modified) in &file_pairs {
            let differ = Differ::new(original, modified, DiffAlgorithmType::Myers);
            let mut patch = differ.generate();

            // Set file paths
            patch.old_file = filename.clone();
            patch.new_file = filename.clone();

            patches.push(patch);
        }

        black_box(MultifilePatch { patches })
    });
}

use patcher::{DiffAlgorithm, Differ, PatchAlgorithm, Patcher};

fn main() {
    let old = include_str!("../fixtures/code/old.py");
    let new = include_str!("../fixtures/code/new.py");

    // Generate a patch
    let differ = Differ::new(old, new);
    let patch = differ.generate();

    println!("Generated patch:\n");
    println!("{}", patch);

    // Apply it to the original content
    let patcher = Patcher::new(patch);
    let result = patcher.apply(old, false).unwrap();

    println!("Applied patch:\n");
    println!("{}", result);

    assert_eq!(result, new);
}

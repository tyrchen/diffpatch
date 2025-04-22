use anyhow::Result;
use diffpatch::{Differ, Patch, Patcher};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let original = "line1\nline2\nline3\nline4\nline5";
    let modified = "line1\nmodified line2\nline3\nnew line\nline5";

    println!("=== Original Content ===");
    println!("{}", original);

    println!("\n=== Modified Content ===");
    println!("{}", modified);

    // Generate a patch
    let differ = Differ::new(original, modified);
    let patch = differ.generate();

    println!("\n=== Generated Patch ===");
    println!("{}", patch);

    // Apply the patch
    let patcher = Patcher::new(patch.clone());
    let result = patcher.apply(original, false)?;

    println!("\n=== Result After Applying Patch ===");
    println!("{}", result);
    assert_eq!(result, modified);

    // Apply the patch in reverse
    let reverse_result = patcher.apply(modified, true)?;

    println!("\n=== Result After Applying Patch in Reverse ===");
    println!("{}", reverse_result);
    assert_eq!(reverse_result, original);

    // Let's save the patch to a file
    let examples_dir = Path::new("examples");
    if !examples_dir.exists() {
        fs::create_dir(examples_dir)?;
    }

    let patch_path = examples_dir.join("example.patch");
    fs::write(&patch_path, patch.to_string())?;
    println!("\nPatch saved to: {:?}", patch_path);

    // Now parse the patch from the file
    let patch_content = fs::read_to_string(&patch_path)?;
    let parsed_patch = Patch::parse(&patch_content)?;

    println!("\n=== Parsed Patch ===");
    println!("Old file: {}", parsed_patch.old_file);
    println!("New file: {}", parsed_patch.new_file);
    println!("Number of chunks: {}", parsed_patch.chunks.len());

    Ok(())
}

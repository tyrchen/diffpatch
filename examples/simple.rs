use anyhow::Result;
use diffpatch::differ::DiffAlgorithmType;
use diffpatch::{Differ, Patch, Patcher};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    // Basic example - single file
    println!("=== Single File Patch Example ===");
    single_file_example()?;

    // Advanced example - multi-file patch creation
    println!("\n=== Multi-File Patch Example ===");
    multi_file_example()?;

    Ok(())
}

fn single_file_example() -> Result<()> {
    let original = "line1\nline2\nline3\nline4\nline5";
    let modified = "line1\nmodified line2\nline3\nnew line\nline5";

    println!("=== Original Content ===");
    println!("{}", original);

    println!("\n=== Modified Content ===");
    println!("{}", modified);

    // Generate a patch
    let differ = Differ::new(original, modified, DiffAlgorithmType::Myers);
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

fn multi_file_example() -> Result<()> {
    use diffpatch::{MultifilePatch, MultifilePatcher};

    // Setup example directory structure
    let examples_dir = Path::new("examples");
    let tmp_dir = examples_dir.join("tmp");

    if !tmp_dir.exists() {
        fs::create_dir_all(&tmp_dir)?;
    }

    // Create some example files
    let file1_path = tmp_dir.join("file1.txt");
    let file2_path = tmp_dir.join("file2.txt");

    let file1_original = "This is file 1\nwith multiple lines\nof content\nto be modified.";
    let file2_original = "This is file 2\nwith different content\nthat will also change.";

    fs::write(&file1_path, file1_original)?;
    fs::write(&file2_path, file2_original)?;

    println!("Created two files in: {:?}", tmp_dir);

    // Generate patches for both files
    let file1_modified =
        "This is file 1\nwith MODIFIED lines\nof content\nto be changed.\nPlus a new line.";
    let file2_modified = "This is file 2\nwith different content\nthat has been changed.";

    // Create patches for each file
    let differ1 = Differ::new(file1_original, file1_modified, DiffAlgorithmType::Myers);
    let mut patch1 = differ1.generate();
    patch1.old_file = file1_path.to_str().unwrap().to_string();
    patch1.new_file = file1_path.to_str().unwrap().to_string();

    let differ2 = Differ::new(file2_original, file2_modified, DiffAlgorithmType::Myers);
    let mut patch2 = differ2.generate();
    patch2.old_file = file2_path.to_str().unwrap().to_string();
    patch2.new_file = file2_path.to_str().unwrap().to_string();

    // Create a multi-file patch
    let multi_patch = MultifilePatch::new(vec![patch1, patch2]);

    // Save the multi-file patch
    let patch_path = examples_dir.join("multi.patch");
    let patch_content = multi_patch
        .patches
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&patch_path, patch_content)?;

    println!("Created multi-file patch at: {:?}", patch_path);

    // Apply the multi-file patch
    let multi_patcher = MultifilePatcher::new(multi_patch);
    let patched_files = multi_patcher.apply_and_write(false)?;

    println!("\nPatched {} files:", patched_files.len());
    for file in &patched_files {
        println!("- {}", file);
    }

    // Verify the changes
    let file1_new_content = fs::read_to_string(&file1_path)?;
    let file2_new_content = fs::read_to_string(&file2_path)?;

    // Assert the changes were correctly applied
    assert_eq!(file1_new_content, file1_modified);
    assert_eq!(file2_new_content, file2_modified);

    println!("\nFile contents after patching:");
    println!("=== file1.txt ===");
    println!("{}", file1_new_content);
    println!("\n=== file2.txt ===");
    println!("{}", file2_new_content);

    // Now apply the patch in reverse to revert changes
    let multi_patch = MultifilePatch::parse_from_file(&patch_path)?;
    let multi_patcher = MultifilePatcher::new(multi_patch);
    multi_patcher.apply_and_write(true)?;

    println!("\nReverted changes using reverse patch application");

    // Verify the reverted content
    let file1_reverted = fs::read_to_string(&file1_path)?;
    let file2_reverted = fs::read_to_string(&file2_path)?;

    assert_eq!(file1_reverted, file1_original);
    assert_eq!(file2_reverted, file2_original);

    println!("All files successfully reverted to original state");

    Ok(())
}

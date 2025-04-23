use anyhow::Result;
use patcher::{ApplyResult, DiffAlgorithm, Differ, MultifilePatch, MultifilePatcher};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    println!("=== Multi-File Patch Example ===");

    // Setup example directory structure
    let tmp_dir = Path::new("/tmp/patcher-examples");

    if !tmp_dir.exists() {
        fs::create_dir_all(tmp_dir)?;
    }

    // Create test files
    create_test_files(tmp_dir)?;

    // Create multi-file patch
    let patch_path = create_multi_file_patch(tmp_dir)?;

    // Apply the patch to modify files
    apply_patch(tmp_dir, &patch_path, false)?;

    // Apply the patch in reverse to restore original files
    apply_patch(tmp_dir, &patch_path, true)?;

    Ok(())
}

fn create_test_files(dir: &Path) -> Result<()> {
    println!("Creating test files...");

    // Define some test files
    let files = [
        (
            "config.json",
            "{\n  \"name\": \"patcher\",\n  \"version\": \"0.1.0\",\n  \"debug\": false\n}",
        ),
        (
            "README.txt",
            "# Test Project\n\nThis is a test project for patcher.\n\nMore information will be added later.",
        ),
        (
            "src/main.rs",
            "fn main() {\n    println!(\"Hello, world!\");\n}",
        ),
    ];

    // Create each file
    for (path, content) in &files {
        let file_path = dir.join(path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write the file
        fs::write(&file_path, content)?;
        println!("  Created: {}", path);
    }

    println!("Test files created successfully");

    Ok(())
}

fn create_multi_file_patch(dir: &Path) -> Result<std::path::PathBuf> {
    println!("\nCreating multi-file patch...");

    // Define the original and modified content pairs
    let changes = [
        (
            "config.json",
            "{\n  \"name\": \"patcher\",\n  \"version\": \"0.1.0\",\n  \"debug\": false\n}",
            "{\n  \"name\": \"patcher\",\n  \"version\": \"0.2.0\",\n  \"debug\": true,\n  \"logLevel\": \"info\"\n}",
        ),
        (
            "README.txt",
            "# Test Project\n\nThis is a test project for patcher.\n\nMore information will be added later.",
            "# Patcher Test\n\nThis is a test project showcasing the patcher library.\n\nSee examples for more details.",
        ),
        (
            "src/main.rs",
            "fn main() {\n    println!(\"Hello, world!\");\n}",
            "fn main() {\n    println!(\"Hello, patcher!\");\n    println!(\"Version 0.2.0\");\n}",
        ),
    ];

    // Generate patches for each file
    let mut patches = Vec::new();

    for (path, original, modified) in &changes {
        let differ = Differ::new(original, modified);
        let mut patch = differ.generate();

        // Set the file paths in the patch
        patch.old_file = path.to_string();
        patch.new_file = path.to_string();

        patches.push(patch);
        println!("  Created patch for: {}", path);
    }

    // Create multi-file patch
    let multi_patch = MultifilePatch::new(patches);

    // Save the patch to a file
    let patch_path = dir.join("changes.patch");

    fs::write(&patch_path, multi_patch.to_string())?;

    println!("Multi-file patch created at: {:?}", patch_path);

    Ok(patch_path)
}

fn apply_patch(root: &Path, patch_path: &Path, reverse: bool) -> Result<()> {
    let action = if reverse { "Reverting" } else { "Applying" };
    println!("\n{} multi-file patch...", action);

    // Parse the patch from file
    let multi_patch = MultifilePatch::parse_from_file(patch_path)?;

    // Print summary of the patch
    println!("Patch contains {} files:", multi_patch.patches.len());
    for patch in &multi_patch.patches {
        println!("  - {}", patch.old_file);
    }

    // Apply the patch
    let multi_patcher = MultifilePatcher::with_root(multi_patch, root);
    let results = multi_patcher.apply_and_write(reverse)?;

    println!(
        "\n{} action resulted in {} outcomes:",
        action,
        results.len()
    );

    let mut success_count = 0;
    for result in results {
        match result {
            ApplyResult::Applied(file) => {
                println!(
                    "  - Applied: {} {}",
                    file.path,
                    if file.is_new { "(new file)" } else { "" }
                );
                // Read and display the file content
                match fs::read_to_string(&file.path) {
                    Ok(content) => println!(
                        "    Content (first 50 chars): {}",
                        content.chars().take(50).collect::<String>()
                    ),
                    Err(e) => println!("    Error reading file {}: {}", file.path, e),
                };
                success_count += 1;
            }
            ApplyResult::Deleted(path) => {
                println!("  - Deleted: {}", path);
                success_count += 1;
            }
            ApplyResult::Skipped(reason) => {
                println!("  - Skipped: {}", reason);
            }
            ApplyResult::Failed(path, error) => {
                println!("  - Failed: {} - {}", path, error);
            }
        }
    }
    println!("Successfully processed {} files/patches.", success_count);

    Ok(())
}

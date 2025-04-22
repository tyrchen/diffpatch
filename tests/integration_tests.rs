use diffpatch::{MultifilePatch, MultifilePatcher, Operation};
use git2::Repository;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Helper function to get the path to the fixtures directory
fn fixtures_path() -> PathBuf {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir).join("fixtures")
}

#[test]
#[ignore = "This test is sensitive to git diff format and line position in git history. Use test_apply_multifile_git_diff instead."]
fn test_apply_multifile_patch() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Get the current directory (repository root)
    let repo_path = env::current_dir().unwrap();

    // Clone the current repository to the temp directory
    let repo = Repository::clone(repo_path.to_str().unwrap(), temp_path).unwrap();

    // Set up checkout options
    let mut checkout_options = git2::build::CheckoutBuilder::new();
    checkout_options.force(); // Force checkout to overwrite local changes

    // Checkout the diff-test1 tag
    match repo.revparse_single("diff-test1") {
        Ok(object) => {
            // We found the tag or reference, check it out
            repo.checkout_tree(&object, Some(&mut checkout_options))
                .unwrap();
            // Detach HEAD to the object
            repo.set_head_detached(object.id()).unwrap();
            println!("Successfully checked out diff-test1 tag");

            // Print the contents of src/lib.rs for debugging
            let lib_rs_path = temp_path.join("src/lib.rs");
            if lib_rs_path.exists() {
                match fs::read_to_string(&lib_rs_path) {
                    Ok(content) => {
                        println!("\nContents of src/lib.rs after checkout:");
                        println!("===========================================");
                        for (i, line) in content.lines().enumerate() {
                            println!("{}: '{}'", i + 1, line);
                        }
                        println!("===========================================\n");
                    }
                    Err(e) => println!("Failed to read lib.rs: {}", e),
                }
            } else {
                println!("src/lib.rs does not exist after checkout");
            }
        }
        Err(_) => {
            // If the tag doesn't exist, we'll just use the current HEAD
            println!("Tag diff-test1 not found, using current HEAD");
        }
    }

    // Get the path to the patch file
    let patch_path = fixtures_path().join("diff-test1.diff");

    // Parse and apply the patch
    let multifile_patch = MultifilePatch::parse_from_file(patch_path).unwrap();

    // Update file paths in the parsed patch to point to our temp directory
    let mut updated_patches = Vec::new();
    for mut patch in multifile_patch.patches {
        // Convert relative paths to absolute paths
        if patch.old_file == "/dev/null" {
            // For new files, keep /dev/null as is
        } else {
            patch.old_file = temp_path
                .join(&patch.old_file)
                .to_str()
                .unwrap()
                .to_string();
        }

        if patch.new_file == "/dev/null" {
            // For deleted files, keep /dev/null as is
        } else {
            patch.new_file = temp_path
                .join(&patch.new_file)
                .to_str()
                .unwrap()
                .to_string();
        }

        updated_patches.push(patch);
    }

    // Debug info: Print first patch details
    if !updated_patches.is_empty() {
        let first_patch = &updated_patches[0];
        println!(
            "First patch: {} -> {}",
            first_patch.old_file, first_patch.new_file
        );

        // Check if file exists and print first few lines
        if first_patch.old_file != "/dev/null" {
            let file_content = match fs::read_to_string(&first_patch.old_file) {
                Ok(content) => content,
                Err(e) => format!("Error reading file: {}", e),
            };
            let first_few_lines: Vec<&str> = file_content.lines().take(5).collect();
            println!("First few lines of content:");
            for (i, line) in first_few_lines.iter().enumerate() {
                println!("Line {}: '{}'", i + 1, line);
            }
        }
    }

    let patcher = MultifilePatcher::new(MultifilePatch::new(updated_patches));
    let patched_files = patcher.apply_and_write(false).unwrap();

    // Verify patches were applied
    assert!(!patched_files.is_empty());

    // Check for specific files we know should exist after patching
    let src_dir = temp_path.join("src");
    assert!(src_dir.exists());

    // Verify all files from the patch are present and have content
    let paths_to_check = [
        "src/differ.rs",
        "src/lib.rs",
        "src/patch.rs",
        "src/patcher.rs",
    ];

    for path in paths_to_check.into_iter() {
        let file_path = temp_path.join(path);
        assert!(file_path.exists(), "File does not exist: {}", path);

        // Verify the file has content
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(!content.is_empty(), "File is empty: {}", path);

        // Verify specific content in each file
        match path {
            "src/differ.rs" => assert!(content.contains("The Differ struct")),
            "src/lib.rs" => assert!(content.contains("patch represents all the changes")),
            "src/patch.rs" => assert!(content.contains("Parse a patch from a string")),
            "src/patcher.rs" => assert!(content.contains("Apply the patch to the content")),
            _ => {}
        }
    }
}

// A simpler test for basic multifile patching functionality
#[test]
fn test_apply_patch_file() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create required directory structure
    fs::create_dir_all(temp_path.join("src")).unwrap();

    // Create a simple patch file manually for testing
    let patch_content = "\
diff --git a/src/test.txt b/src/test.txt
--- a/src/test.txt
+++ b/src/test.txt
@@ -1,2 +1,3 @@
 line1
-line2
+line2 modified
+line3
";

    let patch_file = temp_path.join("test.patch");
    fs::write(&patch_file, patch_content).unwrap();

    // Create the file to be patched
    fs::write(temp_path.join("src/test.txt"), "line1\nline2\n").unwrap();

    // Parse and apply the patch
    let multifile_patch = MultifilePatch::parse_from_file(patch_file).unwrap();

    // Update file paths in the parsed patch to point to our temp directory
    let mut updated_patches = Vec::new();
    for mut patch in multifile_patch.patches {
        patch.old_file = temp_path.join("src/test.txt").to_str().unwrap().to_string();
        patch.new_file = temp_path.join("src/test.txt").to_str().unwrap().to_string();
        updated_patches.push(patch);
    }

    let patcher = MultifilePatcher::new(MultifilePatch::new(updated_patches));
    let patched_files = patcher.apply_and_write(false).unwrap();

    // Verify the patched file
    assert_eq!(patched_files.len(), 1);
    let content = fs::read_to_string(temp_path.join("src/test.txt")).unwrap();
    let expected = "line1\nline2 modified\nline3\n";
    assert_eq!(content.trim_end(), expected.trim_end());
}

#[test]
fn test_apply_multifile_git_diff() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a minimal test structure with simple content
    fs::create_dir_all(temp_path.join("src")).unwrap();

    // Create a simple test file
    let simple_content = "line1\nline2\nline3\nline4\nline5\n";
    fs::write(temp_path.join("src/test.txt"), simple_content).unwrap();

    // Print the test file with line numbers
    println!("Original file content:");
    for (i, line) in simple_content.lines().enumerate() {
        println!("{}: '{}'", i + 1, line);
    }

    // Create a simple patch that should work
    let patch_content = r#"diff --git a/src/file1.txt b/src/file1.txt
new file mode 100644
index 0000000..b1e6722
--- /dev/null
+++ b/src/file1.txt
@@ -0,0 +1,3 @@
+New file line 1
+New file line 2
+New file line 3
diff --git a/src/test.txt b/src/test.txt
index 1234..5678 100644
--- a/src/test.txt
+++ b/src/test.txt
@@ -2,3 +2,3 @@
 line2
-line3
+line3 modified
 line4
"#;

    let patch_file = temp_path.join("test.patch");
    fs::write(&patch_file, patch_content).unwrap();

    // Parse the patch
    let multifile_patch = MultifilePatch::parse_from_file(&patch_file).unwrap();

    // Print the parsed patches
    println!("Parsed {} patches", multifile_patch.patches.len());
    for (i, patch) in multifile_patch.patches.iter().enumerate() {
        println!("Patch {}: {} -> {}", i, patch.old_file, patch.new_file);
        println!("  Chunks: {}", patch.chunks.len());
        for (j, chunk) in patch.chunks.iter().enumerate() {
            println!(
                "  Chunk {}: old_start={}, old_lines={}, new_start={}, new_lines={}",
                j, chunk.old_start, chunk.old_lines, chunk.new_start, chunk.new_lines
            );
            println!("    Operations: {}", chunk.operations.len());
            for (k, op) in chunk.operations.iter().enumerate() {
                match op {
                    Operation::Context(line) => println!("      [{}] Context: '{}'", k, line),
                    Operation::Add(line) => println!("      [{}] Add: '{}'", k, line),
                    Operation::Remove(line) => println!("      [{}] Remove: '{}'", k, line),
                }
            }
        }
    }

    // Update file paths to point to our temp directory
    let mut updated_patches = Vec::new();
    for mut patch in multifile_patch.patches {
        // Convert relative paths to absolute paths
        if patch.old_file == "/dev/null" {
            // For new files, keep /dev/null as is
        } else {
            patch.old_file = temp_path
                .join(&patch.old_file)
                .to_str()
                .unwrap()
                .to_string();
        }

        if patch.new_file == "/dev/null" {
            // For deleted files, keep /dev/null as is
        } else {
            patch.new_file = temp_path
                .join(&patch.new_file)
                .to_str()
                .unwrap()
                .to_string();
        }

        updated_patches.push(patch);
    }

    // Apply the patch
    let patcher = MultifilePatcher::new(MultifilePatch::new(updated_patches));
    let patched_files = patcher.apply_and_write(false).unwrap();

    // Verify the results
    assert!(patched_files.len() == 2);

    // Check that file1.txt was created
    let file1_path = temp_path.join("src/file1.txt");
    assert!(file1_path.exists());
    let content = fs::read_to_string(&file1_path).unwrap();
    assert!(content.contains("New file line 1"));

    // Check that test.txt was modified
    let test_path = temp_path.join("src/test.txt");
    let content = fs::read_to_string(&test_path).unwrap();
    println!("Modified file content:");
    for (i, line) in content.lines().enumerate() {
        println!("{}: '{}'", i + 1, line);
    }
    assert!(content.contains("line3 modified"));
}

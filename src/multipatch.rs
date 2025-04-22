use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use crate::{Error, MultifilePatch, MultifilePatcher, Patch, PatchedFile, Patcher};

impl MultifilePatch {
    /// Create a new MultifilePatch with the given patches
    pub fn new(patches: Vec<Patch>) -> Self {
        Self { patches }
    }

    /// Parse a multi-file patch from a string
    pub fn parse(content: &str) -> Result<Self, Error> {
        let mut patches = Vec::new();
        let mut current_lines = Vec::new();
        let mut in_patch = false;
        let mut preamble = None;

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.starts_with("diff ") {
                // We've found a new patch section
                if !current_lines.is_empty() {
                    // Parse the previous patch if there is one
                    let patch_content = current_lines.join("\n");
                    let mut patch = Patch::parse(&patch_content)?;
                    if let Some(pre) = preamble.take() {
                        patch.preemble = Some(pre);
                    }
                    patches.push(patch);
                    current_lines.clear();
                }

                // Start a new patch section
                preamble = Some(line.to_string());
                in_patch = false; // Wait for --- and +++ headers

                // Skip lines like "new file mode", "index", etc. until we find "---"
                i += 1;
                while i < lines.len() && !lines[i].starts_with("--- ") {
                    // Store these lines in preamble but don't add to current_lines
                    preamble = Some(format!("{}\n{}", preamble.unwrap_or_default(), lines[i]));
                    i += 1;
                }

                if i < lines.len() {
                    // Found the start of patch content (---)
                    in_patch = true;
                    current_lines.push(lines[i]);
                }
            } else if in_patch {
                current_lines.push(line);
            }

            i += 1;
        }

        // Don't forget the last patch
        if !current_lines.is_empty() {
            let patch_content = current_lines.join("\n");
            let mut patch = Patch::parse(&patch_content)?;
            if let Some(pre) = preamble {
                patch.preemble = Some(pre);
            }
            patches.push(patch);
        }

        Ok(Self { patches })
    }

    /// Parse a multi-file patch from a file
    pub fn parse_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Self::parse(&content)
    }
}

impl MultifilePatcher {
    /// Create a new MultifilePatcher with the given patches
    pub fn new(patch: MultifilePatch) -> Self {
        Self {
            patches: patch.patches,
        }
    }

    /// Apply the patches to files in the current directory
    pub fn apply(&self, reverse: bool) -> Result<Vec<PatchedFile>, Error> {
        let mut patched_files = Vec::new();

        for (i, patch) in self.patches.iter().enumerate() {
            let file_path = if reverse {
                &patch.new_file
            } else {
                &patch.old_file
            };

            println!(
                "Applying patch {}: {} -> {}",
                i, patch.old_file, patch.new_file
            );

            // Read the file content
            let content = match fs::read_to_string(file_path) {
                Ok(content) => {
                    println!("  Successfully read file: {}", file_path);
                    println!(
                        "  First few lines: {}",
                        content.lines().take(3).collect::<Vec<_>>().join("\n  ")
                    );
                    content
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    if reverse {
                        // If we're applying in reverse and the new file doesn't exist,
                        // this is likely a file creation patch being undone, so we skip it
                        println!(
                            "  Skipping reverse patch for non-existent file: {}",
                            file_path
                        );
                        continue;
                    } else {
                        // If the file to patch doesn't exist, check if it's a new file
                        if patch.old_file.contains("/dev/null") || patch.old_file.is_empty() {
                            // It's a new file, use empty content
                            println!("  Creating new file: {}", patch.new_file);
                            String::new()
                        } else {
                            println!("  ERROR: File not found: {}", file_path);
                            return Err(Error::FileNotFound(file_path.clone()));
                        }
                    }
                }
                Err(err) => {
                    println!("  ERROR: IO Error reading {}: {}", file_path, err);
                    return Err(Error::IoError(err));
                }
            };

            // Apply the patch
            let patcher = Patcher::new(patch.clone());
            match patcher.apply(&content, reverse) {
                Ok(new_content) => {
                    let target_path = if reverse {
                        &patch.old_file
                    } else {
                        &patch.new_file
                    };

                    // If target is /dev/null, this is a file deletion
                    if target_path.contains("/dev/null") || target_path.is_empty() {
                        // File deletion - we don't need to create a patched file
                        // Instead, we'll delete the original file
                        println!("  Deleting file: {}", file_path);
                        if !reverse {
                            if let Err(err) = fs::remove_file(file_path) {
                                println!("  ERROR: Failed to delete file: {}", err);
                                return Err(Error::IoError(err));
                            }
                        }
                        continue;
                    }

                    patched_files.push(PatchedFile {
                        path: target_path.clone(),
                        content: new_content,
                    });
                }
                Err(e) => {
                    println!("  ERROR: Failed to apply patch {}: {}", i, e);
                    return Err(e);
                }
            }
        }

        Ok(patched_files)
    }

    /// Apply the patches to files in the current directory and write the results
    pub fn apply_and_write(&self, reverse: bool) -> Result<Vec<String>, Error> {
        let patched_files = self.apply(reverse)?;
        let mut written_files = Vec::new();

        for file in patched_files {
            // Create parent directories if they don't exist
            if let Some(parent) = Path::new(&file.path).parent() {
                fs::create_dir_all(parent)?;
            }

            // Write the file
            let mut output_file = File::create(&file.path)?;
            output_file.write_all(file.content.as_bytes())?;
            written_files.push(file.path);
        }

        Ok(written_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Differ;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_multifile_patch() {
        // Setup temporary directory
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create test files
        let file1_path = temp_path.join("file1.txt");
        let file2_path = temp_path.join("file2.txt");

        fs::write(&file1_path, "line1\nline2\nline3\n").unwrap();
        fs::write(&file2_path, "foo\nbar\nbaz\n").unwrap();

        // Create patches
        let old1 = "line1\nline2\nline3\n";
        let new1 = "line1\nmodified\nline3\n";
        let differ1 = Differ::new(old1, new1);
        let mut patch1 = differ1.generate();
        patch1.old_file = file1_path.to_str().unwrap().to_string();
        patch1.new_file = file1_path.to_str().unwrap().to_string();

        let old2 = "foo\nbar\nbaz\n";
        let new2 = "foo\nbar\nqux\n";
        let differ2 = Differ::new(old2, new2);
        let mut patch2 = differ2.generate();
        patch2.old_file = file2_path.to_str().unwrap().to_string();
        patch2.new_file = file2_path.to_str().unwrap().to_string();

        // Create multifile patch and clone the patches for later use
        let patch1_clone = patch1.clone();
        let patch2_clone = patch2.clone();
        let multipatch = MultifilePatch::new(vec![patch1, patch2]);

        // Apply the patches
        let patcher = MultifilePatcher::new(multipatch);
        let written_files = patcher.apply_and_write(false).unwrap();

        // Verify the results
        assert_eq!(written_files.len(), 2);
        let file1_content = fs::read_to_string(&file1_path).unwrap();
        let file2_content = fs::read_to_string(&file2_path).unwrap();
        assert_eq!(file1_content.trim_end(), new1.trim_end());
        assert_eq!(file2_content.trim_end(), new2.trim_end());

        // Test reverse patching
        let patcher = MultifilePatcher::new(MultifilePatch::new(vec![patch1_clone, patch2_clone]));
        patcher.apply_and_write(true).unwrap();

        // Verify the reverse
        let file1_content = fs::read_to_string(&file1_path).unwrap();
        let file2_content = fs::read_to_string(&file2_path).unwrap();
        assert_eq!(file1_content.trim_end(), old1.trim_end());
        assert_eq!(file2_content.trim_end(), old2.trim_end());
    }

    #[test]
    fn test_parse_multifile_patch() {
        let patch_content = "diff --git a/file1.txt b/file1.txt
--- a/file1.txt
+++ b/file1.txt
@@ -1,3 +1,3 @@
 line1
-line2
+modified
 line3
diff --git a/file2.txt b/file2.txt
--- a/file2.txt
+++ b/file2.txt
@@ -1,3 +1,3 @@
 foo
 bar
-baz
+qux";

        let multipatch = MultifilePatch::parse(patch_content).unwrap();
        assert_eq!(multipatch.patches.len(), 2);
        assert_eq!(multipatch.patches[0].old_file, "file1.txt");
        assert_eq!(multipatch.patches[0].new_file, "file1.txt");
        assert_eq!(multipatch.patches[1].old_file, "file2.txt");
        assert_eq!(multipatch.patches[1].new_file, "file2.txt");
    }

    #[test]
    fn test_file_creation() {
        // Setup temporary directory
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create patch for a new file
        let new_content = "This is a new file\nwith some content\n";
        let mut patch = Patch {
            preemble: Some("diff --git a/dev/null b/newfile.txt".to_string()),
            old_file: "/dev/null".to_string(),
            new_file: temp_path.join("newfile.txt").to_str().unwrap().to_string(),
            chunks: vec![],
        };

        // Add a chunk that adds all lines of the new file
        let operations = new_content
            .lines()
            .map(|line| crate::Operation::Add(line.to_string()))
            .collect();

        patch.chunks.push(crate::Chunk {
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: new_content.lines().count(),
            operations,
        });

        // Apply the patch
        let multipatch = MultifilePatch::new(vec![patch]);
        let patcher = MultifilePatcher::new(multipatch);
        let written_files = patcher.apply_and_write(false).unwrap();

        // Verify the result
        assert_eq!(written_files.len(), 1);
        let new_file_path = temp_path.join("newfile.txt");
        assert!(new_file_path.exists());
        let file_content = fs::read_to_string(&new_file_path).unwrap();
        assert_eq!(file_content.trim_end(), new_content.trim_end());
    }

    #[test]
    fn test_file_deletion() {
        // Setup temporary directory
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file to be deleted
        let file_to_delete = temp_path.join("delete.txt");
        let content = "This file will be deleted\n";
        fs::write(&file_to_delete, content).unwrap();

        // Create patch that deletes the file
        let mut patch = Patch {
            preemble: Some("diff --git a/delete.txt b/dev/null".to_string()),
            old_file: file_to_delete.to_str().unwrap().to_string(),
            new_file: "/dev/null".to_string(),
            chunks: vec![],
        };

        // Add a chunk that removes all lines
        let operations = content
            .lines()
            .map(|line| crate::Operation::Remove(line.to_string()))
            .collect();

        patch.chunks.push(crate::Chunk {
            old_start: 0,
            old_lines: content.lines().count(),
            new_start: 0,
            new_lines: 0,
            operations,
        });

        // Apply the patch
        let multipatch = MultifilePatch::new(vec![patch]);
        let patcher = MultifilePatcher::new(multipatch);
        patcher.apply_and_write(false).unwrap();

        // Verify the file is deleted
        assert!(!file_to_delete.exists());
    }

    #[test]
    fn test_parse_git_diff_format() {
        // Path to the fixtures directory
        let fixtures_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let diff_path = fixtures_path.join("diff-test1.diff");

        // Parse the git diff file
        let multipatch = MultifilePatch::parse_from_file(diff_path).unwrap_or_else(|e| {
            panic!("Failed to parse diff-test1.diff: {}", e);
        });

        // Check that we have at least some patches
        assert!(!multipatch.patches.is_empty());

        // Verify first patch details (should be for differ.rs)
        let first_patch = &multipatch.patches[0];
        assert_eq!(first_patch.old_file, "/dev/null");
        assert_eq!(first_patch.new_file, "src/differ.rs");
    }
}

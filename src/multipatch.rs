use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::{Error, Patch, PatchAlgorithm, Patcher};

/// Represents a file that has been patched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchedFile {
    /// Path to the file relative to the application root.
    pub path: String,
    /// New content of the file after patching.
    pub content: String,
    /// Indicates whether the file was newly created by the patch.
    pub is_new: bool,
    /// Indicates whether the file was deleted by the patch.
    pub is_deleted: bool,
}

/// A collection of patches for multiple files, typically parsed from a unified diff format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultifilePatch {
    /// List of individual file patches.
    pub patches: Vec<Patch>,
}

/// Applies a `MultifilePatch` to a set of files.
#[derive(Debug)]
pub struct MultifilePatcher {
    /// The collection of patches to apply.
    multifile_patch: MultifilePatch,
    /// Optional root directory to apply patches relative to.
    /// If None, paths in the patch are treated as relative to the current working directory.
    root_dir: Option<PathBuf>,
}

/// Represents the status of applying a single patch within a multifile patch operation.
#[derive(Debug)]
pub enum ApplyResult {
    /// Patch applied successfully, resulting in file content change or creation.
    Applied(PatchedFile),
    /// Patch applied successfully, resulting in file deletion.
    Deleted(String), // Path of the deleted file
    /// Patch was skipped (e.g., reverse patch for a non-existent file).
    Skipped(String), // Reason for skipping
    /// Patch failed to apply.
    Failed(String, Error), // Path and Error
}

impl MultifilePatch {
    /// Creates a new `MultifilePatch` with the given patches.
    pub fn new(patches: Vec<Patch>) -> Self {
        Self { patches }
    }

    /// Parses a multi-file patch (unified diff format) from a string.
    ///
    /// Handles concatenated diffs (multiple `diff --git ...` sections).
    pub fn parse(content: &str) -> Result<Self, Error> {
        let mut patches = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            // Handle empty input gracefully
            return Ok(Self { patches: vec![] });
        }

        let mut patch_start_index: Option<usize> = None;

        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("diff --git ") {
                // If we found the start of a new patch, process the previous one (if any)
                if let Some(start) = patch_start_index {
                    let patch_lines_slice = &lines[start..i];
                    // Check if the slice is non-empty before joining and parsing
                    if !patch_lines_slice.is_empty() {
                        let patch_content = patch_lines_slice.join("\n"); // Join only the slice
                        match Patch::parse(&patch_content) {
                            Ok(patch) => patches.push(patch),
                            Err(e) => {
                                // Provide more context in the warning
                                warn!(
                                    "Warning: Skipping malformed patch section (lines {}-{}): {}\n--- Patch Content Start ---\n{}\n--- Patch Content End ---",
                                    start + 1, i, e, patch_content
                                );
                            }
                        }
                    }
                }
                // Mark the start line index of the new patch section
                patch_start_index = Some(i);
            }
        }

        // Process the last patch section found in the file (from last diff to EOF)
        if let Some(start) = patch_start_index {
            let patch_lines_slice = &lines[start..]; // Slice from start to the end
            if !patch_lines_slice.is_empty() {
                let patch_content = patch_lines_slice.join("\n"); // Join the last slice
                match Patch::parse(&patch_content) {
                    Ok(patch) => patches.push(patch),
                    Err(e) => {
                        warn!(
                            "Warning: Skipping malformed patch section at end of file (lines {}-{}): {}\n--- Patch Content Start ---\n{}\n--- Patch Content End ---",
                            start + 1, lines.len(), e, patch_content
                        );
                    }
                }
            }
        }

        // Check for validity: If the input wasn't empty but no patches were parsed,
        // determine if it was due to missing 'diff' lines or parsing errors.
        if patches.is_empty() && !content.trim().is_empty() {
            if !content.lines().any(|l| l.starts_with("diff ")) {
                // Content exists but no 'diff --git' lines found
                return Err(Error::InvalidPatchFormat(
                    "No patch sections found starting with 'diff '".to_string(),
                ));
            } else {
                // Found 'diff --git' lines, but all sections failed parsing (warnings printed above)
                return Err(Error::InvalidPatchFormat(
                    "Found 'diff --git' lines, but failed to parse any valid patch sections."
                        .to_string(),
                ));
            }
        }

        Ok(Self { patches })
    }

    /// Parses a multi-file patch from a file specified by the path.
    pub fn parse_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let content = fs::read_to_string(path.as_ref()).map_err(Error::IoError)?;
        Self::parse(&content)
    }
}

impl MultifilePatcher {
    /// Creates a new `MultifilePatcher` for the given `MultifilePatch`.
    /// Patches will be applied relative to the current working directory.
    pub fn new(multifile_patch: MultifilePatch) -> Self {
        Self {
            multifile_patch,
            root_dir: None,
        }
    }

    /// Creates a new `MultifilePatcher` for the given `MultifilePatch`,
    /// applying patches relative to the specified `root_dir`.
    pub fn with_root<P: AsRef<Path>>(multifile_patch: MultifilePatch, root_dir: P) -> Self {
        Self {
            multifile_patch,
            root_dir: Some(root_dir.as_ref().to_path_buf()),
        }
    }

    /// Resolves a patch file path relative to the `root_dir` if set,
    /// otherwise returns the path as is.
    fn resolve_path(&self, patch_path: &str) -> PathBuf {
        match &self.root_dir {
            Some(root) => root.join(patch_path),
            None => PathBuf::from(patch_path),
        }
    }

    /// Applies all patches in the `MultifilePatch` to the corresponding files.
    ///
    /// This method performs the patching in memory.
    /// Use `apply_and_write` to write changes directly to the filesystem.
    ///
    /// # Arguments
    ///
    /// * `reverse` - If `true`, applies the patches in reverse (reverting changes).
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<ApplyResult>)` - A vector containing the result status for each patch.
    /// * `Err(Error)` - If a fatal error occurs during setup (e.g., reading root dir fails, though unlikely here).
    pub fn apply(&self, reverse: bool) -> Result<Vec<ApplyResult>, Error> {
        let mut results = Vec::with_capacity(self.multifile_patch.patches.len());

        for patch in &self.multifile_patch.patches {
            let (source_path_str, target_path_str, is_new_file, is_delete_file) = if reverse {
                // When reversing:
                // Source is the *new* file (or /dev/null if it was a deletion).
                // Target is the *old* file (or /dev/null if it was a creation).
                (
                    &patch.new_file,
                    &patch.old_file,
                    patch.new_file == "/dev/null" || patch.new_file.ends_with("/dev/null"), // Reversing a delete results in creation
                    patch.old_file == "/dev/null" || patch.old_file.ends_with("/dev/null"), // Reversing a create results in deletion
                )
            } else {
                // When applying normally:
                // Source is the *old* file (or /dev/null if it was a creation).
                // Target is the *new* file (or /dev/null if it was a deletion).
                (
                    &patch.old_file,
                    &patch.new_file,
                    patch.old_file == "/dev/null" || patch.old_file.ends_with("/dev/null"), // New file if old is /dev/null
                    patch.new_file == "/dev/null" || patch.new_file.ends_with("/dev/null"), // Deleting if new is /dev/null
                )
            };

            // Determine the actual file path to read content from.
            let source_path = self.resolve_path(source_path_str);
            let target_path_str = target_path_str.to_string(); // Target path as string for PatchedFile

            // Read the source file content.
            let source_content_result = if is_new_file {
                // If it's a new file patch, the source content is empty.
                Ok(String::new())
            } else {
                fs::read_to_string(&source_path)
            };

            let result = match source_content_result {
                Ok(content) => {
                    // Apply the individual patch.
                    let patcher = Patcher::new(patch.clone());
                    match patcher.apply(&content, reverse) {
                        Ok(new_content) => {
                            if is_delete_file {
                                // If the target is /dev/null, it signifies a deletion.
                                ApplyResult::Deleted(source_path_str.to_string())
                            } else {
                                // Otherwise, it's a modification or creation.
                                ApplyResult::Applied(PatchedFile {
                                    path: target_path_str,
                                    content: new_content,
                                    is_new: is_new_file, // is_new determined earlier
                                    is_deleted: false,
                                })
                            }
                        }
                        Err(e) => {
                            // Failed to apply the patch logic.
                            // Report failure associated with the *target* path, as that's the intended outcome.
                            // Using source_path_str here is misleading if apply fails.
                            // Example: Apply fails for "a -> b", report should relate to "b".
                            ApplyResult::Failed(target_path_str, e)
                        }
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    if reverse && (is_new_file || is_delete_file) {
                        // If reversing a creation (is_delete_file true) or deletion (is_new_file true)
                        // and the source file doesn't exist, it means the state is already as expected.
                        // e.g. Reversing creation of file X -> delete X. If X doesn't exist, skip.
                        // e.g. Reversing deletion of file Y -> create Y. If Y doesn't exist (target of delete), skip?
                        // This skip logic might need refinement depending on desired behavior for reverse.
                        // Current: Skip reversing creation/deletion if the file is already gone/present.
                        ApplyResult::Skipped(format!(
                            "Skipping reverse for non-existent file involved in creation/deletion: {}",
                            source_path.display()
                        ))
                    } else {
                        // File genuinely not found when expected.
                        ApplyResult::Failed(
                            source_path_str.to_string(),
                            Error::FileNotFound {
                                path: source_path.display().to_string(),
                            },
                        )
                    }
                }
                Err(err) => {
                    // Other I/O error reading the file.
                    // Report failure associated with the *target* path, as failure to read source
                    // prevents the target operation.
                    ApplyResult::Failed(target_path_str, Error::IoError(err))
                }
            };
            results.push(result);
        }

        Ok(results)
        // Note: The two-pass retry logic from the original code is removed for simplicity.
        // It can be added back if needed, perhaps as a separate method or strategy.
        // Retrying often indicates underlying issues with the patch or the source files.
    }

    /// Applies the patches and writes the results directly to the filesystem.
    ///
    /// Creates necessary directories, writes modified/new files, and deletes files marked for deletion.
    ///
    /// # Arguments
    ///
    /// * `reverse` - If `true`, applies the patches in reverse (reverting changes).
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<ApplyResult>)` - A vector containing the result status for each patch applied.
    /// * `Err(Error)` - If a fatal error occurs during file I/O.
    pub fn apply_and_write(&self, reverse: bool) -> Result<Vec<ApplyResult>, Error> {
        let results = self.apply(reverse)?;
        let mut final_results = Vec::with_capacity(results.len());

        for result in results {
            match result {
                ApplyResult::Applied(ref file) => {
                    let target_path = self.resolve_path(&file.path);

                    // Create parent directories if they don't exist.
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent).map_err(Error::IoError)?;
                    }

                    // Write the patched content to the file.
                    match File::create(&target_path) {
                        Ok(mut output_file) => {
                            if let Err(e) = output_file.write_all(file.content.as_bytes()) {
                                final_results.push(ApplyResult::Failed(
                                    file.path.clone(),
                                    Error::IoError(e),
                                ));
                            } else {
                                final_results.push(result); // Keep original successful ApplyResult::Applied
                            }
                        }
                        Err(e) => {
                            final_results
                                .push(ApplyResult::Failed(file.path.clone(), Error::IoError(e)));
                        }
                    }
                }
                ApplyResult::Deleted(ref path_str) => {
                    let path_to_delete = self.resolve_path(path_str);
                    if path_to_delete.exists() {
                        match fs::remove_file(&path_to_delete) {
                            Ok(_) => final_results.push(result), // Keep original successful ApplyResult::Deleted
                            Err(e) => {
                                final_results
                                    .push(ApplyResult::Failed(path_str.clone(), Error::IoError(e)));
                            }
                        }
                    } else {
                        // File to delete doesn't exist, treat as success/skipped for deletion? Push original result.
                        final_results.push(result);
                    }
                }
                ApplyResult::Skipped(_) | ApplyResult::Failed(_, _) => {
                    // Pass through Skipped and Failed results without further action.
                    final_results.push(result);
                }
            }
        }

        Ok(final_results)
    }
}

impl fmt::Display for MultifilePatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for patch in &self.patches {
            writeln!(f, "{}", patch)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffAlgorithm, Differ};
    use std::fs;
    use tempfile::tempdir; // Use tempdir instead of TempDir for simpler Result handling

    // Helper to create a basic MultifilePatch for testing
    fn create_test_multifile_patch(
        temp_path: &Path,
        files: &[(&str, &str, &str)], // (filename, old_content, new_content)
    ) -> MultifilePatch {
        let patches = files
            .iter()
            .map(|(name, old_content, new_content)| {
                let file_path = temp_path.join(name);
                let mut patch = Differ::new(old_content, new_content).generate();
                patch.old_file = file_path.to_str().unwrap().to_string();
                patch.new_file = file_path.to_str().unwrap().to_string();
                patch
            })
            .collect();
        MultifilePatch::new(patches)
    }

    #[test]
    fn test_apply_multifile_patch() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();

        // File definitions
        let file1_name = "file1.txt";
        let file1_old = "line1\nline2\nline3\n";
        let file1_new = "line1\nmodified\nline3\n";
        let file2_name = "file2.txt";
        let file2_old = "foo\nbar\nbaz\n";
        let file2_new = "foo\nbar\nqux\n";

        // Create initial files
        fs::write(temp_path.join(file1_name), file1_old)?;
        fs::write(temp_path.join(file2_name), file2_old)?;

        // Create and apply patch
        let multipatch = create_test_multifile_patch(
            temp_path,
            &[
                (file1_name, file1_old, file1_new),
                (file2_name, file2_old, file2_new),
            ],
        );
        let patcher = MultifilePatcher::new(multipatch.clone()); // Clone for reverse patch later
        let results = patcher.apply_and_write(false)?;

        // Verify results
        assert_eq!(results.len(), 2);
        let mut applied_count = 0;
        for result in &results {
            if let ApplyResult::Applied(file) = result {
                applied_count += 1;
                let content = fs::read_to_string(temp_path.join(&file.path))?;
                if file.path == temp_path.join(file1_name).to_str().unwrap() {
                    assert_eq!(content.trim_end(), file1_new.trim_end());
                } else if file.path == temp_path.join(file2_name).to_str().unwrap() {
                    assert_eq!(content.trim_end(), file2_new.trim_end());
                } else {
                    panic!("Unexpected patched file path: {}", file.path);
                }
            }
        }
        assert_eq!(applied_count, 2, "Expected 2 files to be applied");

        // Test reverse patching
        let reverse_patcher = MultifilePatcher::new(multipatch);
        let reverse_results = reverse_patcher.apply_and_write(true)?;

        // Verify the reverse
        assert_eq!(reverse_results.len(), 2);
        applied_count = 0;
        for result in &reverse_results {
            if let ApplyResult::Applied(file) = result {
                applied_count += 1;
                let content = fs::read_to_string(temp_path.join(&file.path))?;
                if file.path == temp_path.join(file1_name).to_str().unwrap() {
                    assert_eq!(content.trim_end(), file1_old.trim_end());
                } else if file.path == temp_path.join(file2_name).to_str().unwrap() {
                    assert_eq!(content.trim_end(), file2_old.trim_end());
                } else {
                    panic!("Unexpected reversed file path: {}", file.path);
                }
            }
        }
        assert_eq!(
            applied_count, 2,
            "Expected 2 files to be applied in reverse"
        );

        Ok(())
    }

    #[test]
    fn test_parse_git_diff_format() {
        let patch_content = "diff --git a/src/main.rs b/src/main.rs
index 123..456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 use std::fs;

 fn main() {
+    println!(\"Hello\");
 }
diff --git a/README.md b/README.md
new file mode 100644
index 000..abc
--- /dev/null
+++ b/README.md
@@ -0,0 +1 @@
+# My Project
diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
index def..000
--- a/src/old.rs
+++ /dev/null
@@ -1 +0,0 @@
-println!(\"Old file\");
";

        let multipatch =
            MultifilePatch::parse(patch_content).expect("Failed to parse valid git diff");
        assert_eq!(multipatch.patches.len(), 3);

        // Check first patch (modification)
        assert_eq!(multipatch.patches[0].old_file, "src/main.rs");
        assert_eq!(multipatch.patches[0].new_file, "src/main.rs");
        assert!(multipatch.patches[0].preamble.is_some());
        assert!(!multipatch.patches[0].chunks.is_empty());

        // Check second patch (creation)
        assert_eq!(multipatch.patches[1].old_file, "/dev/null");
        assert_eq!(multipatch.patches[1].new_file, "README.md");
        assert!(!multipatch.patches[1].chunks.is_empty());
        assert_eq!(multipatch.patches[1].chunks[0].old_lines, 0);
        assert_eq!(multipatch.patches[1].chunks[0].new_lines, 1);

        // Check third patch (deletion)
        assert_eq!(multipatch.patches[2].old_file, "src/old.rs");
        assert_eq!(multipatch.patches[2].new_file, "/dev/null");
        assert!(!multipatch.patches[2].chunks.is_empty());
        assert_eq!(multipatch.patches[2].chunks[0].old_lines, 1);
        assert_eq!(multipatch.patches[2].chunks[0].new_lines, 0);
    }

    #[test]
    fn test_parse_empty_or_invalid_content() {
        assert!(MultifilePatch::parse("").unwrap().patches.is_empty());
        assert!(MultifilePatch::parse("some random text\nwithout diff header").is_err());
        assert!(MultifilePatch::parse("diff --git a/file b/file\n--- a/file\n").is_err());
        // Missing +++
    }

    #[test]
    fn test_apply_file_creation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();
        let new_file_name = "newly_created.txt";
        let new_content = "This is a brand new file.\n";

        // Create a patch for a new file
        let patch = Patch {
            preamble: Some(format!("diff --git a/dev/null b/{}", new_file_name)),
            old_file: "/dev/null".to_string(),
            new_file: new_file_name.to_string(),
            chunks: vec![crate::Chunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 1,
                operations: vec![crate::Operation::Add(new_content.to_string())],
            }],
        };

        let multipatch = MultifilePatch::new(vec![patch]);
        let patcher = MultifilePatcher::with_root(multipatch, temp_path); // Apply relative to temp_path
        let results = patcher.apply_and_write(false)?;

        // Verify result
        assert_eq!(results.len(), 1);
        let target_path_abs = temp_path.join(new_file_name);
        match &results[0] {
            ApplyResult::Applied(file) => {
                assert_eq!(file.path, new_file_name);
                assert!(file.is_new);
                assert!(!file.is_deleted);
                assert!(target_path_abs.exists(), "File should have been created");
                let written_content = fs::read_to_string(&target_path_abs)?;
                assert_eq!(written_content.trim_end(), new_content.trim_end());
            }
            _ => panic!("Expected ApplyResult::Applied, got {:?}", results[0]),
        }

        Ok(())
    }

    #[test]
    fn test_apply_file_deletion() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();
        let file_to_delete_name = "to_delete.txt";
        let content = "Delete me.\n";

        // Create the file to be deleted
        let file_path_abs = temp_path.join(file_to_delete_name);
        fs::write(&file_path_abs, content)?;
        assert!(file_path_abs.exists());

        // Create patch that deletes the file
        let mut patch = Patch {
            preamble: Some(format!("diff --git a/{} b/dev/null", file_to_delete_name)),
            old_file: file_to_delete_name.to_string(), // Relative path
            new_file: "/dev/null".to_string(),
            chunks: vec![],
        };
        patch.chunks.push(crate::Chunk {
            old_start: 0, // 1-based in header, 0-based internally
            old_lines: 1,
            new_start: 0,
            new_lines: 0,
            operations: vec![crate::Operation::Remove(content.trim_end().to_string())],
        });

        let multipatch = MultifilePatch::new(vec![patch]);
        let patcher = MultifilePatcher::with_root(multipatch, temp_path); // Apply relative to temp_path
        let results = patcher.apply_and_write(false)?;

        // Verify result
        assert_eq!(results.len(), 1);
        match &results[0] {
            ApplyResult::Deleted(deleted_path) => {
                assert_eq!(deleted_path, file_to_delete_name);
                assert!(!file_path_abs.exists(), "File should have been deleted");
            }
            _ => panic!("Expected ApplyResult::Deleted, got {:?}", results[0]),
        }

        Ok(())
    }

    #[test]
    fn test_apply_with_root_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();
        let sub_dir_name = "subdir";
        let file_name = "in_subdir.txt";
        let file_rel_path = Path::new(sub_dir_name).join(file_name);
        let file_abs_path = temp_path.join(&file_rel_path);

        let old_content = "Version 1\n";
        let new_content = "Version 2\n";

        // Create initial file in subdir
        fs::create_dir_all(file_abs_path.parent().unwrap())?;
        fs::write(&file_abs_path, old_content)?;

        // Create patch with relative paths (as typically generated by git)
        let mut patch = Differ::new(old_content, new_content).generate();
        patch.old_file = file_rel_path.to_str().unwrap().to_string();
        patch.new_file = file_rel_path.to_str().unwrap().to_string();

        let multipatch = MultifilePatch::new(vec![patch]);
        // Crucially, provide the root directory
        let patcher = MultifilePatcher::with_root(multipatch, temp_path);
        let results = patcher.apply_and_write(false)?;

        // Verify
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], ApplyResult::Applied(_)));
        let updated_content = fs::read_to_string(&file_abs_path)?;
        assert_eq!(updated_content.trim_end(), new_content.trim_end());

        Ok(())
    }

    #[test]
    fn test_apply_fails_file_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();
        let file_name = "non_existent.txt";
        let old_content = "line1\n";
        let new_content = "line2\n";

        let mut patch = Differ::new(old_content, new_content).generate();
        patch.old_file = file_name.to_string();
        patch.new_file = file_name.to_string();

        let multipatch = MultifilePatch::new(vec![patch]);
        let patcher = MultifilePatcher::with_root(multipatch, temp_path);
        let results = patcher.apply(false)?; // Don't write, just check results

        assert_eq!(results.len(), 1);
        match &results[0] {
            ApplyResult::Failed(path, err) => {
                assert_eq!(path, file_name);
                assert!(matches!(err, Error::FileNotFound { .. }));
            }
            _ => panic!(
                "Expected ApplyResult::Failed(FileNotFound), got {:?}",
                results[0]
            ),
        }

        Ok(())
    }

    #[test]
    fn test_apply_and_write_handles_io_error() -> Result<(), Box<dyn std::error::Error>> {
        // Setup scenario where writing will fail (e.g., target is a directory)
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();
        let file_name = "target_file.txt";
        let dir_path = temp_path.join(file_name); // Create a directory where the file should be
        fs::create_dir(&dir_path)?;

        let new_content = "b";
        // Directly create a patch for a new file (without using Differ)
        let patch = Patch {
            preamble: Some(format!("diff --git a/dev/null b/{}", file_name)),
            old_file: "/dev/null".to_string(),
            new_file: file_name.to_string(),
            chunks: vec![crate::Chunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 1,
                operations: vec![crate::Operation::Add(new_content.to_string())],
            }],
        };

        let multipatch = MultifilePatch::new(vec![patch]);
        let patcher = MultifilePatcher::with_root(multipatch, temp_path);
        let results = patcher.apply_and_write(false)?; // This should attempt to write

        // Check that the result indicates failure
        assert_eq!(results.len(), 1);
        match &results[0] {
            // Check that the failed path matches the intended target file name
            ApplyResult::Failed(path, err) => {
                assert_eq!(
                    path, file_name,
                    "The path in the Failed result should match the target filename"
                );
                assert!(
                    matches!(err, Error::IoError(_)),
                    "Error should be an IoError, got: {:?}",
                    err
                );
            }
            _ => panic!(
                "Expected ApplyResult::Failed(IoError), got {:?}",
                results[0]
            ),
        }

        Ok(())
    }
}

use thiserror::Error;

mod differ;
mod patch;
mod patcher;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to apply patch: {0}")]
    ApplyError(String),

    #[error("Invalid patch format: {0}")]
    InvalidPatchFormat(String),

    #[error("Line not found: {0}")]
    LineNotFound(String),
}

/// A patch represents all the changes between two versions of a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// Preemble of the patch, something like "diff -u a/file.txt b/file.txt"
    pub preemble: Option<String>,
    /// Original file path
    pub old_file: String,
    /// New file path
    pub new_file: String,
    /// Chunks of changes
    pub chunks: Vec<Chunk>,
}

/// The Differ struct is used to generate a patch between old and new content
pub struct Differ {
    old: String,
    new: String,
    context_lines: usize,
}

/// The Patcher struct is used to apply a patch to content
pub struct Patcher {
    patch: Patch,
}

/// Represents a change operation in the patch
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Add a new line
    Add(String),
    /// Remove a line
    Remove(String),
    /// Context line (unchanged)
    Context(String),
}

/// A chunk represents a continuous section of changes in a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Starting line in the original file (0-based)
    pub old_start: usize,
    /// Number of lines in the original file
    pub old_lines: usize,
    /// Starting line in the new file (0-based)
    pub new_start: usize,
    /// Number of lines in the new file
    pub new_lines: usize,
    /// The operations in this chunk
    pub operations: Vec<Operation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";

        // Generate a patch
        let differ = Differ::new(old, new);
        let patch = differ.generate();

        // Apply the patch
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }
}

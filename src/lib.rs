use thiserror::Error;

mod differ;
mod multipatch;
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

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),
}

/// The Diff trait allows implementing a diffing algorithm for custom types
pub trait Diff {
    /// The error type returned by the diff implementation
    type Error;

    /// Called when elements are equal between sequences
    fn equal(&mut self, old_idx: usize, new_idx: usize, count: usize) -> Result<(), Self::Error>;

    /// Called when elements need to be deleted from the old sequence
    fn delete(&mut self, old_idx: usize, count: usize, new_idx: usize) -> Result<(), Self::Error>;

    /// Called when elements need to be inserted from the new sequence
    fn insert(&mut self, old_idx: usize, new_idx: usize, count: usize) -> Result<(), Self::Error>;

    /// Called when the diff is complete
    fn finish(&mut self) -> Result<(), Self::Error>;
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

/// Represents a file that has been patched
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchedFile {
    /// Path to the file
    pub path: String,
    /// New content of the file
    pub content: String,
}

/// A collection of patches for multiple files
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultifilePatch {
    /// List of individual file patches
    pub patches: Vec<Patch>,
}

/// The MultifilePatcher struct is used to apply multiple patches
pub struct MultifilePatcher {
    /// List of patches to apply
    pub patches: Vec<Patch>,
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

/// Myers diff algorithm. Creates a diff between two sequences
/// using the efficient Myers algorithm. The provided diff callback
/// will be called for each operation (equal, insert, delete).
pub fn myers_diff<S, T, D>(
    d: &mut D,
    a: &S,
    a0: usize,
    a1: usize,
    b: &T,
    b0: usize,
    b1: usize,
) -> Result<(), D::Error>
where
    S: std::ops::Index<usize> + ?Sized,
    T: std::ops::Index<usize> + ?Sized,
    T::Output: PartialEq<S::Output>,
    D: Diff,
{
    // Implement the Myers diff algorithm to find shortest edit path
    // This uses the algorithm from the provided code example
    differ::diff(d, a, a0, a1, b, b0, b1)
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

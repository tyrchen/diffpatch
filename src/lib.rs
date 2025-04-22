use thiserror::Error;

pub mod differ;
mod multipatch;
mod patch;
mod patcher;

// Re-export the differ implementations for convenience
pub use differ::{DiffAlgorithm, Differ, MyersDiffer, NaiveDiffer};
pub use multipatch::{ApplyResult, MultifilePatch, MultifilePatcher, PatchedFile};
pub use patch::{Chunk, Operation, Patch};
pub use patcher::Patcher;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to apply patch: {0}")]
    ApplyError(String),

    #[error("Invalid patch format: {0}")]
    InvalidPatchFormat(String),

    #[error("Line {line_num} not found in content while applying patch")]
    LineNotFound { line_num: usize },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Could not parse chunk header: {header}")]
    InvalidChunkHeader { header: String },

    #[error("Could not parse number '{value}' for {field}: {source}")]
    InvalidNumberFormat {
        value: String,
        field: String,
        #[source]
        source: std::num::ParseIntError,
    },
}

#[cfg(test)]
mod tests {
    // Bring necessary items into scope for the test
    use super::{Differ, Error, Patcher};

    #[test]
    fn test_integration_diff_and_patch() -> Result<(), Error> {
        let old_content = "line1
line2
line3
line4";
        let new_content = "line1
line2 modified
line3
line4";

        // Arrange: Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Act: Apply the patch
        let patcher = Patcher::new(patch);
        let actual_content = patcher.apply(old_content, false)?;

        // Assert: Check if the patched content matches the new content
        assert_eq!(actual_content, new_content);
        Ok(())
    }
}

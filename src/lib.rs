use std::fmt;
use thiserror::Error;

mod differ;
mod patch;
mod patcher;

pub use differ::Differ;
pub use patch::Patch;
pub use patcher::Patcher;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to apply patch: {0}")]
    ApplyError(String),

    #[error("Invalid patch format: {0}")]
    InvalidPatchFormat(String),

    #[error("Line not found: {0}")]
    LineNotFound(String),
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

impl Operation {
    pub(crate) fn to_char(&self) -> char {
        match self {
            Operation::Add(_) => '+',
            Operation::Remove(_) => '-',
            Operation::Context(_) => ' ',
        }
    }

    pub(crate) fn line(&self) -> &str {
        match self {
            Operation::Add(line) => line,
            Operation::Remove(line) => line,
            Operation::Context(line) => line,
        }
    }
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

impl fmt::Display for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "@@ -{},{} +{},{} @@",
            self.old_start + 1,
            self.old_lines,
            self.new_start + 1,
            self.new_lines
        )?;

        for op in &self.operations {
            writeln!(f, "{}{}", op.to_char(), op.line())?;
        }

        Ok(())
    }
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

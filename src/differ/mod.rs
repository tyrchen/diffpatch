mod common;
mod myers;
mod naive;
pub mod similar;
mod xdiff;

use crate::Patch;

pub use myers::MyersDiffer;
pub use naive::NaiveDiffer;
pub use similar::SimilarDiffer;
pub use xdiff::XDiffDiffer;

pub use common::*;

/// Trait for different diffing algorithms
pub trait DiffAlgorithm {
    /// Generate a patch between the old and new content
    fn generate(&self) -> Patch;
}

/// Enum to specify which diffing algorithm to use
pub enum DiffAlgorithmType {
    Myers,
    Naive,
    XDiff,
    Similar,
}

/// The base Differ struct that orchestrates the diffing process
pub struct Differ {
    pub(crate) algorithm: DiffAlgorithmType,
    pub(crate) old: String,
    pub(crate) new: String,
    pub(crate) context_lines: usize,
}

impl Differ {
    /// Create a new Differ with the old and new content using the default algorithm (XDiff).
    pub fn new(old: &str, new: &str) -> Self {
        Self::new_with_algorithm(old, new, DiffAlgorithmType::XDiff)
    }

    /// Create a new Differ with the old and new content and a specified algorithm.
    pub fn new_with_algorithm(old: &str, new: &str, algorithm: DiffAlgorithmType) -> Self {
        Self {
            algorithm,
            old: old.to_string(),
            new: new.to_string(),
            context_lines: 3, // Default number of context lines
        }
    }

    /// Set the number of context lines to include in the generated patch.
    pub fn context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }
}

impl DiffAlgorithm for Differ {
    fn generate(&self) -> Patch {
        match self.algorithm {
            DiffAlgorithmType::Myers => MyersDiffer::new(self).generate(),
            DiffAlgorithmType::Naive => NaiveDiffer::new(self).generate(),
            DiffAlgorithmType::XDiff => XDiffDiffer::new(self).generate(),
            DiffAlgorithmType::Similar => SimilarDiffer::new(self).generate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PatchAlgorithm, Patcher};

    #[test]
    fn test_different_algorithms_produce_valid_patches() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";
        // Create a differ
        let differ = Differ::new(old, new);
        // Test naive algorithm
        let naive = NaiveDiffer::new(&differ);
        let naive_patch = naive.generate();
        let naive_result = Patcher::new(naive_patch).apply(old, false).unwrap();
        assert_eq!(naive_result, new);
        // Test Myers algorithm
        let myers = MyersDiffer::new(&differ);
        let myers_patch = myers.generate();
        let myers_result = Patcher::new(myers_patch).apply(old, false).unwrap();
        assert_eq!(myers_result, new);
        // Test XDiff algorithm
        let xdiff = XDiffDiffer::new(&differ);
        let xdiff_patch = xdiff.generate();
        let xdiff_result = Patcher::new(xdiff_patch).apply(old, false).unwrap();
        assert_eq!(xdiff_result, new);
    }

    #[test]
    fn test_complex_diff_comparison() {
        let old = "This is a test file\nwith multiple lines\nthat will be modified\nin various ways\nto test the diff algorithms\nend of file";
        let new = "This is a changed test file\nwith multiple modified lines\nthat will be completely changed\nand some lines removed\nto test the diff algorithms\nnew line at end\nend of file";
        // Create a differ with more context lines
        let differ = Differ::new(old, new).context_lines(2);
        // Test all algorithms and make sure they all produce valid patches
        let naive = NaiveDiffer::new(&differ);
        let naive_patch = naive.generate();
        let naive_result = Patcher::new(naive_patch).apply(old, false).unwrap();
        assert_eq!(naive_result, new);
        let myers = MyersDiffer::new(&differ);
        let myers_patch = myers.generate();
        let myers_result = Patcher::new(myers_patch).apply(old, false).unwrap();
        assert_eq!(myers_result, new);
        let xdiff = XDiffDiffer::new(&differ);
        let xdiff_patch = xdiff.generate();
        let xdiff_result = Patcher::new(xdiff_patch).apply(old, false).unwrap();
        assert_eq!(xdiff_result, new);
    }
}

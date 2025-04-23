use crate::diff_result::DiffResult;
use crate::myers::myers_diff;
// Add other diff algorithm imports when implemented
// use crate::naive::naive_diff;
// use crate::xdiff::xdiff_diff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffAlgorithmType {
    Myers,
    Naive, // To be implemented
    XDiff, // To be implemented
}

impl Default for DiffAlgorithmType {
    fn default() -> Self {
        DiffAlgorithmType::Myers
    }
}

pub struct Differ<'a> {
    original_lines: Vec<&'a str>,
    modified_lines: Vec<&'a str>,
    algorithm: DiffAlgorithmType,
}

impl<'a> Differ<'a> {
    pub fn new(original: &'a str, modified: &'a str) -> Self {
        Self::new_with_algorithm(original, modified, DiffAlgorithmType::default())
    }

    pub fn new_with_algorithm(
        original: &'a str,
        modified: &'a str,
        algorithm: DiffAlgorithmType,
    ) -> Self {
        Self {
            original_lines: original.lines().collect(),
            modified_lines: modified.lines().collect(),
            algorithm,
        }
    }

    pub fn generate(&self) -> Vec<DiffResult> {
        match self.algorithm {
            DiffAlgorithmType::Myers => myers_diff(&self.original_lines, &self.modified_lines),
            DiffAlgorithmType::Naive => {
                // naive_diff(&self.original_lines, &self.modified_lines)
                panic!("Naive algorithm not yet implemented");
            }
            DiffAlgorithmType::XDiff => {
                // xdiff_diff(&self.original_lines, &self.modified_lines)
                panic!("XDiff algorithm not yet implemented");
            }
        }
    }
}

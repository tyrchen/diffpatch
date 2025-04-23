use crate::diff_result::DiffResult;
use crate::myers::myers_diff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffAlgorithmType {
    Myers,
    Naive,
    XDiff,
}

pub struct Differ<'a> {
    original: &'a str,
    modified: &'a str,
    algorithm: DiffAlgorithmType,
}

impl<'a> Differ<'a> {
    pub fn new(original: &'a str, modified: &'a str) -> Self {
        Self {
            original,
            modified,
            algorithm: DiffAlgorithmType::Myers, // Default algorithm
        }
    }

    pub fn new_with_algorithm(
        original: &'a str,
        modified: &'a str,
        algorithm: DiffAlgorithmType,
    ) -> Self {
        Self {
            original,
            modified,
            algorithm,
        }
    }

    pub fn generate(&self) -> Vec<DiffResult> {
        match self.algorithm {
            DiffAlgorithmType::Myers => {
                let original_lines: Vec<&str> = self.original.lines().collect();
                let modified_lines: Vec<&str> = self.modified.lines().collect();
                myers_diff(&original_lines, &modified_lines)
            }
            DiffAlgorithmType::Naive => {
                // Placeholder for Naive implementation
                vec![]
            }
            DiffAlgorithmType::XDiff => {
                // Placeholder for XDiff implementation
                vec![]
            }
        }
    }
}

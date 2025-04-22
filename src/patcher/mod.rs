mod naive;
mod similar;

use crate::{Error, Patch};

pub use naive::NaivePatcher;
pub use similar::SimilarPatcher;

#[derive(Clone)]
pub struct Patcher {
    patch: Patch,
    algorithm: PatcherAlgorithm,
}

impl Patcher {
    pub fn new(patch: Patch) -> Self {
        Self::new_with_algorithm(patch, PatcherAlgorithm::Naive)
    }

    pub fn new_with_algorithm(patch: Patch, algorithm: PatcherAlgorithm) -> Self {
        Self { patch, algorithm }
    }
}

/// Trait for different patching algorithms
pub trait PatchAlgorithm {
    /// Applies the patch to the provided content.
    ///
    /// # Arguments
    ///
    /// * `content` - The original content (as a string slice) to patch.
    /// * `reverse` - If `true`, applies the patch in reverse (reverting changes).
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The patched content.
    /// * `Err(Error)` - If the patch cannot be applied cleanly.
    fn apply(&self, content: &str, reverse: bool) -> Result<String, Error>;
}

/// Enum to specify which patching algorithm to use
#[derive(Clone, Default)]
pub enum PatcherAlgorithm {
    #[default]
    Naive,
    Similar,
}

impl PatchAlgorithm for Patcher {
    fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
        match self.algorithm {
            PatcherAlgorithm::Naive => NaivePatcher::new(&self.patch).apply(content, reverse),
            PatcherAlgorithm::Similar => SimilarPatcher::new(&self.patch).apply(content, reverse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differ::{DiffAlgorithm, Differ};

    #[test]
    fn test_create_patchers() {
        let old_content = "line1\nline2\nline3";
        let new_content = "line1\nline2 modified\nline3";

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Test naive patcher
        let naive_patcher = Patcher::new_with_algorithm(patch.clone(), PatcherAlgorithm::Naive);
        let naive_result = naive_patcher.apply(old_content, false).unwrap();
        assert_eq!(naive_result, new_content);

        // Test similar patcher
        let similar_patcher = Patcher::new_with_algorithm(patch.clone(), PatcherAlgorithm::Similar);
        let similar_result = similar_patcher.apply(old_content, false).unwrap();
        assert_eq!(similar_result, new_content);
    }
}

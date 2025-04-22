use super::{DiffAlgorithm, Differ};
use crate::patch::{Chunk, Operation, Patch};
use similar::{Algorithm as SimilarAlgorithm, DiffTag, TextDiff};

pub struct SimilarDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> SimilarDiffer<'a> {
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }
}

impl DiffAlgorithm for SimilarDiffer<'_> {
    fn generate(&self) -> Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();

        let diff = TextDiff::configure()
            .algorithm(SimilarAlgorithm::Patience)
            .diff_lines(&self.differ.old, &self.differ.new);

        let mut patch_chunks = Vec::new();

        let grouped_ops = diff.grouped_ops(self.differ.context_lines);

        for group in grouped_ops {
            let first_op = group.first().expect("Group should not be empty");
            let _last_op = group.last().expect("Group should not be empty");

            let chunk_old_start = first_op.old_range().start;
            let chunk_new_start = first_op.new_range().start;

            let mut actual_old_lines = 0;
            let mut actual_new_lines = 0;
            let mut chunk_operations = Vec::new();

            for op in group {
                match op.tag() {
                    DiffTag::Equal => {
                        for i in op.old_range() {
                            chunk_operations.push(Operation::Context(old_lines[i].to_string()));
                        }
                        actual_old_lines += op.old_range().len();
                        actual_new_lines += op.new_range().len();
                    }
                    DiffTag::Delete => {
                        for i in op.old_range() {
                            chunk_operations.push(Operation::Remove(old_lines[i].to_string()));
                        }
                        actual_old_lines += op.old_range().len();
                    }
                    DiffTag::Insert => {
                        for j in op.new_range() {
                            chunk_operations.push(Operation::Add(new_lines[j].to_string()));
                        }
                        actual_new_lines += op.new_range().len();
                    }
                    DiffTag::Replace => {
                        for i in op.old_range() {
                            chunk_operations.push(Operation::Remove(old_lines[i].to_string()));
                        }
                        for j in op.new_range() {
                            chunk_operations.push(Operation::Add(new_lines[j].to_string()));
                        }
                        actual_old_lines += op.old_range().len();
                        actual_new_lines += op.new_range().len();
                    }
                }
            }

            if chunk_operations
                .iter()
                .any(|op| !matches!(op, Operation::Context(_)))
            {
                patch_chunks.push(Chunk {
                    old_start: chunk_old_start,
                    old_lines: actual_old_lines,
                    new_start: chunk_new_start,
                    new_lines: actual_new_lines,
                    operations: chunk_operations,
                });
            }
        }

        Patch {
            old_file: "a".to_string(),
            new_file: "b".to_string(),
            chunks: patch_chunks,
            preamble: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differ::{DiffAlgorithmType, Differ};
    use crate::patcher::Patcher;
    use crate::test_utils::load_fixture;

    fn run_diff_and_apply(old: &str, new: &str, context: usize) -> String {
        let base_differ =
            Differ::new_with_algorithm(old, new, DiffAlgorithmType::Similar).context_lines(context);
        let differ = SimilarDiffer::new(&base_differ);
        let patch = differ.generate();
        if old == new {
            assert!(
                patch.chunks.is_empty(),
                "Patch should be empty for identical content"
            );
        } else {
            assert!(
                !patch.chunks.is_empty(),
                "Patch should not be empty for different content. Patch: \n{}",
                patch
            );
        }
        Patcher::new(patch)
            .apply(old, false)
            .expect("Patch application failed")
    }

    #[test]
    fn test_simple_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";
        let result = run_diff_and_apply(old, new, 3);
        assert_eq!(result, new);
    }

    #[test]
    fn test_add_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3\nline4";
        let result = run_diff_and_apply(old, new, 3);
        assert_eq!(result, new);
    }

    #[test]
    fn test_remove_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline3";
        let result = run_diff_and_apply(old, new, 3);
        assert_eq!(result, new);
    }

    #[test]
    fn test_modify_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2_modified\nline3";
        let result = run_diff_and_apply(old, new, 3);
        assert_eq!(result, new);
    }

    #[test]
    fn test_empty_files() {
        // Empty to non-empty
        let old = "";
        let new = "line1\nline2";
        let result1 = run_diff_and_apply(old, new, 3);
        assert_eq!(result1, new);

        // Non-empty to empty
        let old = "line1\nline2";
        let new = "";
        let result2 = run_diff_and_apply(old, new, 3);
        assert_eq!(result2, new);

        // Empty to empty
        let old = "";
        let new = "";
        let result3 = run_diff_and_apply(old, new, 3);
        assert_eq!(result3, new);
    }

    #[test]
    fn test_similar_fixture_simple() {
        let old = load_fixture("simple_before.rs");
        let new = load_fixture("simple_after.rs");
        let result = run_diff_and_apply(&old, &new, 3);
        assert_eq!(result, new);
    }

    #[test]
    fn test_similar_fixture_complex() {
        let old = load_fixture("complex_before.rs");
        let new = load_fixture("complex_after.rs");
        let result = run_diff_and_apply(&old, &new, 3);
        assert_eq!(result, new);
    }

    // Test integration with the main Differ struct
    #[test]
    fn test_differ_integration() {
        let old = "aaa\nbbb\nccc";
        let new = "aaa\nzzz\nccc";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::Similar);
        let patch = differ.generate(); // This calls SimilarDiffer::generate indirectly
        assert!(!patch.chunks.is_empty());
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }
}

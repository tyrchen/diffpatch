use crate::patcher::PatchAlgorithm;
use crate::{Error, Operation, Patch};
use std::borrow::Cow;

/// A naive implementation of the Patcher trait.
/// This implementation simply applies the patch operations in order.
pub struct NaivePatcher<'a> {
    patch: &'a Patch,
}

impl<'a> NaivePatcher<'a> {
    pub fn new(patch: &'a Patch) -> Self {
        Self { patch }
    }
}

impl PatchAlgorithm for NaivePatcher<'_> {
    fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = String::with_capacity(content.len());
        let mut current_line_index = 0;
        let mut first_line = true;

        for chunk in &self.patch.chunks {
            let start_line = if reverse {
                chunk.new_start
            } else {
                chunk.old_start
            };
            let operations = if reverse {
                Cow::Owned(self.reverse_operations(&chunk.operations))
            } else {
                Cow::Borrowed(&chunk.operations)
            };

            // Copy lines until the start of the chunk
            while current_line_index < start_line {
                if current_line_index >= lines.len() {
                    return Err(Error::LineNotFound {
                        line_num: current_line_index + 1,
                    });
                }

                if !first_line {
                    result.push('\n');
                } else {
                    first_line = false;
                }

                result.push_str(lines[current_line_index]);
                current_line_index += 1;
            }

            // Apply the operations in the chunk
            for op in operations.iter() {
                match op {
                    Operation::Context(expected_line) => {
                        if current_line_index >= lines.len() {
                            return Err(Error::LineNotFound {
                                line_num: current_line_index + 1,
                            });
                        }

                        let actual_line = lines[current_line_index];
                        if actual_line != expected_line {
                            return Err(Error::ApplyError(format!(
                                "Context mismatch at line {}: expected '{}', got '{}'",
                                current_line_index + 1,
                                expected_line,
                                actual_line
                            )));
                        }

                        if !first_line {
                            result.push('\n');
                        } else {
                            first_line = false;
                        }

                        result.push_str(actual_line);
                        current_line_index += 1;
                    }
                    Operation::Add(line) => {
                        if !first_line {
                            result.push('\n');
                        } else {
                            first_line = false;
                        }

                        result.push_str(line);
                    }
                    Operation::Remove(expected_line) => {
                        if current_line_index >= lines.len() {
                            return Err(Error::LineNotFound {
                                line_num: current_line_index + 1,
                            });
                        }

                        // This is the key fix - we need to check if the line being removed
                        // matches what we expect to remove
                        let actual_line = lines[current_line_index];
                        if actual_line != expected_line {
                            return Err(Error::ApplyError(format!(
                                "Remove line mismatch at line {}: expected to remove '{}', but found '{}'",
                                current_line_index + 1,
                                expected_line,
                                actual_line
                            )));
                        }

                        current_line_index += 1;
                    }
                }
            }
        }

        // Copy remaining lines
        while current_line_index < lines.len() {
            if !first_line {
                result.push('\n');
            } else {
                first_line = false;
            }

            result.push_str(lines[current_line_index]);
            current_line_index += 1;
        }

        // Ensure final newline if original content had one
        if content.ends_with('\n') && !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }
}

impl NaivePatcher<'_> {
    /// Reverses the operations (Add -> Remove, Remove -> Add) for applying a patch in reverse.
    fn reverse_operations(&self, operations: &[Operation]) -> Vec<Operation> {
        operations
            .iter()
            .map(|op| match op {
                Operation::Add(line) => Operation::Remove(line.clone()),
                Operation::Remove(line) => Operation::Add(line.clone()),
                Operation::Context(line) => Operation::Context(line.clone()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differ::{DiffAlgorithm, Differ};

    #[test]
    fn test_apply_simple_modification() {
        let old_content = "line1\nline2\nline3";
        let new_content = "line1\nline2 modified\nline3";

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Apply the patch
        let patcher = NaivePatcher::new(&patch);
        let result = patcher.apply(old_content, false).unwrap();

        assert_eq!(result, new_content);
    }

    #[test]
    fn test_apply_reverse() {
        let old_content = "line1\nline2\nline3";
        let new_content = "line1\nline2 modified\nline3";

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Apply the patch in reverse
        let patcher = NaivePatcher::new(&patch);
        let result = patcher.apply(new_content, true).unwrap();

        assert_eq!(result, old_content);
    }

    #[test]
    fn test_context_mismatch() {
        let old_content = "line1\nline2\nline3";
        let new_content = "line1\nline2 modified\nline3";
        let bad_content = "line1\nbad line\nline3";

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        println!("patch: {}", patch);

        // Apply the patch to content with mismatched context
        let patcher = NaivePatcher::new(&patch);
        let result = patcher.apply(bad_content, false);
        println!("result: {:?}", result);

        assert!(result.is_err());
        if let Err(Error::ApplyError(msg)) = result {
            // Check for either type of mismatch message
            assert!(
                msg.contains("Context mismatch") || msg.contains("Remove line mismatch"),
                "Error message '{}' doesn't contain expected mismatch text",
                msg
            );
        } else {
            panic!("Expected ApplyError");
        }
    }
}

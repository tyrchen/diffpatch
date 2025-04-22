use crate::{Error, Operation, Patch, Patcher};

impl Patcher {
    /// Create a new Patcher with the given patch
    pub fn new(patch: Patch) -> Self {
        Self { patch }
    }

    /// Apply the patch to the content
    pub fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut current_line = 0;

        for chunk in &self.patch.chunks {
            let (start, operations) = if reverse {
                // In reverse mode, we use new_start and reverse the operations
                (chunk.new_start, self.reverse_operations(&chunk.operations))
            } else {
                (chunk.old_start, chunk.operations.clone())
            };

            // Add lines up to the start of the chunk
            while current_line < start {
                if current_line >= lines.len() {
                    return Err(Error::LineNotFound(format!(
                        "Line {} not found in content",
                        current_line + 1
                    )));
                }
                result.push(lines[current_line].to_string());
                current_line += 1;
            }

            // Apply operations
            for op in operations {
                match op {
                    Operation::Context(line) => {
                        // Context lines should match the content
                        if current_line >= lines.len() || lines[current_line] != line {
                            let actual = if current_line < lines.len() {
                                format!("'{}'", lines[current_line])
                            } else {
                                "EOF".to_string()
                            };
                            return Err(Error::ApplyError(format!(
                                "Context mismatch at line {}. Expected '{}', got {}",
                                current_line + 1,
                                line,
                                actual
                            )));
                        }
                        result.push(line);
                        current_line += 1;
                    }
                    Operation::Add(line) => {
                        // Add the new line
                        result.push(line);
                    }
                    Operation::Remove(_) => {
                        // Skip the line to be removed
                        if current_line >= lines.len() {
                            return Err(Error::LineNotFound(format!(
                                "Line {} not found to remove",
                                current_line + 1
                            )));
                        }
                        current_line += 1;
                    }
                }
            }
        }

        // Add any remaining lines
        while current_line < lines.len() {
            result.push(lines[current_line].to_string());
            current_line += 1;
        }

        Ok(result.join("\n"))
    }

    /// Reverse the operations in a chunk
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
    use crate::Differ;

    #[test]
    fn test_apply_patch() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_reverse_patch() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3\nnew line";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);

        // Apply forward
        let forward = patcher.apply(old, false).unwrap();
        assert_eq!(forward, new);

        // Apply backward
        let backward = patcher.apply(new, true).unwrap();
        assert_eq!(backward, old);
    }
}

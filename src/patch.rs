use crate::{Chunk, Error, Operation};
use std::fmt;

/// A patch represents all the changes between two versions of a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// Original file path
    pub old_file: String,
    /// New file path
    pub new_file: String,
    /// Chunks of changes
    pub chunks: Vec<Chunk>,
}

impl Patch {
    /// Parse a patch from a string
    pub fn parse(content: &str) -> Result<Self, Error> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < 2 {
            return Err(Error::InvalidPatchFormat(
                "Patch must contain at least header lines".to_string(),
            ));
        }

        // Parse header lines (--- and +++)
        let old_file_line = lines[0];
        let new_file_line = lines[1];

        if !old_file_line.starts_with("--- ") || !new_file_line.starts_with("+++ ") {
            return Err(Error::InvalidPatchFormat(
                "Invalid patch header".to_string(),
            ));
        }

        let old_file = old_file_line
            .strip_prefix("--- a/")
            .or_else(|| old_file_line.strip_prefix("--- "))
            .ok_or_else(|| Error::InvalidPatchFormat("Invalid old file header".to_string()))?
            .to_string();

        let new_file = new_file_line
            .strip_prefix("+++ b/")
            .or_else(|| new_file_line.strip_prefix("+++ "))
            .ok_or_else(|| Error::InvalidPatchFormat("Invalid new file header".to_string()))?
            .to_string();

        let mut chunks = Vec::new();
        let mut i = 2; // Start after the header lines

        while i < lines.len() {
            let chunk_header = lines[i];
            if !chunk_header.starts_with("@@ ") || !chunk_header.ends_with(" @@") {
                return Err(Error::InvalidPatchFormat(format!(
                    "Invalid chunk header: {}",
                    chunk_header
                )));
            }

            // Extract the line numbers from the chunk header
            // Format: @@ -old_start,old_lines +new_start,new_lines @@
            let header_parts: Vec<&str> = chunk_header
                .strip_prefix("@@ ")
                .unwrap_or(chunk_header)
                .strip_suffix(" @@")
                .unwrap_or(chunk_header)
                .split(' ')
                .collect();

            if header_parts.len() != 2 {
                return Err(Error::InvalidPatchFormat(format!(
                    "Invalid chunk header format: {}",
                    chunk_header
                )));
            }

            let old_range = header_parts[0].strip_prefix('-').unwrap_or(header_parts[0]);
            let new_range = header_parts[1].strip_prefix('+').unwrap_or(header_parts[1]);

            let old_range_parts: Vec<&str> = old_range.split(',').collect();
            let new_range_parts: Vec<&str> = new_range.split(',').collect();

            if old_range_parts.len() != 2 || new_range_parts.len() != 2 {
                return Err(Error::InvalidPatchFormat(format!(
                    "Invalid range format in chunk header: {}",
                    chunk_header
                )));
            }

            let old_start = old_range_parts[0].parse::<usize>().map_err(|_| {
                Error::InvalidPatchFormat(format!(
                    "Invalid old start number: {}",
                    old_range_parts[0]
                ))
            })?;
            let old_lines = old_range_parts[1].parse::<usize>().map_err(|_| {
                Error::InvalidPatchFormat(format!(
                    "Invalid old lines number: {}",
                    old_range_parts[1]
                ))
            })?;
            let new_start = new_range_parts[0].parse::<usize>().map_err(|_| {
                Error::InvalidPatchFormat(format!(
                    "Invalid new start number: {}",
                    new_range_parts[0]
                ))
            })?;
            let new_lines = new_range_parts[1].parse::<usize>().map_err(|_| {
                Error::InvalidPatchFormat(format!(
                    "Invalid new lines number: {}",
                    new_range_parts[1]
                ))
            })?;

            // Adjust to 0-based indexing
            let old_start = old_start.saturating_sub(1);
            let new_start = new_start.saturating_sub(1);

            i += 1; // Move past the chunk header

            // Parse operations in the chunk
            let mut operations = Vec::new();
            let mut remaining_old_lines = old_lines;
            let mut remaining_new_lines = new_lines;

            while i < lines.len() && (remaining_old_lines > 0 || remaining_new_lines > 0) {
                let line = lines[i];

                if line.starts_with("@@ ") {
                    // We've reached the next chunk
                    break;
                }

                if line.is_empty() {
                    // Skip empty lines
                    i += 1;
                    continue;
                }

                if let Some(content) = line.strip_prefix('+') {
                    // Add operation
                    operations.push(Operation::Add(content.to_string()));
                    remaining_new_lines = remaining_new_lines.saturating_sub(1);
                } else if let Some(content) = line.strip_prefix('-') {
                    // Remove operation
                    operations.push(Operation::Remove(content.to_string()));
                    remaining_old_lines = remaining_old_lines.saturating_sub(1);
                } else if let Some(content) = line.strip_prefix(' ') {
                    // Context operation
                    operations.push(Operation::Context(content.to_string()));
                    remaining_old_lines = remaining_old_lines.saturating_sub(1);
                    remaining_new_lines = remaining_new_lines.saturating_sub(1);
                } else {
                    return Err(Error::InvalidPatchFormat(format!(
                        "Invalid operation line: {}",
                        line
                    )));
                }

                i += 1;
            }

            chunks.push(Chunk {
                old_start,
                old_lines,
                new_start,
                new_lines,
                operations,
            });
        }

        Ok(Patch {
            old_file,
            new_file,
            chunks,
        })
    }
}

impl fmt::Display for Patch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "--- a/{}", self.old_file)?;
        writeln!(f, "+++ b/{}", self.new_file)?;

        for chunk in &self.chunks {
            write!(f, "{}", chunk)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patch() {
        let patch_str = "\
--- a/file.txt
+++ b/file.txt
@@ -1,4 +1,4 @@
 line1
-line2
+line2 modified
 line3
 line4
";

        let patch = Patch::parse(patch_str).unwrap();

        assert_eq!(patch.old_file, "file.txt");
        assert_eq!(patch.new_file, "file.txt");
        assert_eq!(patch.chunks.len(), 1);

        let chunk = &patch.chunks[0];

        assert_eq!(chunk.old_start, 0);
        assert_eq!(chunk.old_lines, 4);
        assert_eq!(chunk.new_start, 0);
        assert_eq!(chunk.new_lines, 4);

        assert_eq!(chunk.operations.len(), 5);
        assert!(matches!(chunk.operations[0], Operation::Context(_)));
        assert!(matches!(chunk.operations[1], Operation::Remove(_)));
        assert!(matches!(chunk.operations[2], Operation::Add(_)));
        assert!(matches!(chunk.operations[3], Operation::Context(_)));
        assert!(matches!(chunk.operations[4], Operation::Context(_)));
    }
}

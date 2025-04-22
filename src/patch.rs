use crate::{Chunk, Error, Operation, Patch};
use std::fmt;

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

impl Patch {
    /// Parse a patch from a string
    pub fn parse(content: &str) -> Result<Self, Error> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < 2 {
            return Err(Error::InvalidPatchFormat(
                "Patch must contain at least header lines".to_string(),
            ));
        }

        // Check for preamble (diff -u or similar)
        let mut start_idx = 0;
        let mut preemble = None;

        if lines[0].starts_with("diff ") {
            preemble = Some(lines[0].to_string());
            start_idx = 1;
        }

        // We need at least two more lines for the file headers
        if start_idx + 1 >= lines.len() {
            return Err(Error::InvalidPatchFormat(
                "Patch must contain file header lines".to_string(),
            ));
        }

        // Parse header lines (--- and +++)
        let old_file_line = lines[start_idx];
        let new_file_line = lines[start_idx + 1];

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
        let mut i = start_idx + 2; // Start after the header lines

        while i < lines.len() {
            let chunk_header = lines[i];
            if !chunk_header.starts_with("@@ ") {
                return Err(Error::InvalidPatchFormat(format!(
                    "Invalid chunk header: {}",
                    chunk_header
                )));
            }

            // Find the positions of the @@ markers
            let start_pos = chunk_header.find("@@ ").unwrap_or(0) + 3; // Skip past the opening @@
            let end_pos = chunk_header[start_pos..]
                .find(" @@")
                .map(|pos| start_pos + pos)
                .unwrap_or_else(|| {
                    // If we can't find closing @@, check for @@ followed by context
                    chunk_header[start_pos..]
                        .find(" @@ ")
                        .map(|pos| start_pos + pos)
                        .unwrap_or(chunk_header.len())
                });

            let header_content = &chunk_header[start_pos..end_pos];

            // Extract the line numbers from the chunk header
            // Format: -old_start,old_lines +new_start,new_lines
            let header_parts: Vec<&str> = header_content.split(' ').collect();

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
            preemble,
            old_file,
            new_file,
            chunks,
        })
    }
}

impl fmt::Display for Patch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(preemble) = &self.preemble {
            writeln!(f, "{}", preemble)?;
        }
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
diff -u a/file.txt b/file.txt
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
        assert_eq!(
            patch.preemble,
            Some("diff -u a/file.txt b/file.txt".to_string())
        );

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

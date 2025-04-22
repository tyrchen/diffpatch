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
                // Skip lines that don't start with @@ - could be empty lines or other git metadata
                i += 1;
                continue;
            }

            // Extract the line numbers from the chunk header using a more flexible approach
            let (old_start, old_lines, new_start, new_lines) = parse_chunk_header(chunk_header)?;

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
                    // Try to handle malformed patches more gracefully - assume it's context if it doesn't have a prefix
                    let content = line;
                    operations.push(Operation::Context(content.to_string()));
                    remaining_old_lines = remaining_old_lines.saturating_sub(1);
                    remaining_new_lines = remaining_new_lines.saturating_sub(1);
                    println!(
                        "Warning: Line without proper prefix treated as context: '{}'",
                        line
                    );
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

/// Parse a chunk header with more flexibility to handle various Git diff formats
/// Returns (old_start, old_lines, new_start, new_lines)
fn parse_chunk_header(header: &str) -> Result<(usize, usize, usize, usize), Error> {
    // Find the positions of the @@ markers
    let start_pos = header.find("@@ ").unwrap_or(0) + 3; // Skip past the opening @@
    let end_pos = header[start_pos..]
        .find(" @@")
        .map(|pos| start_pos + pos)
        .unwrap_or_else(|| {
            // If we can't find closing @@, check for @@ followed by context
            header[start_pos..]
                .find(" @@ ")
                .map(|pos| start_pos + pos)
                .unwrap_or(header.len())
        });

    let header_content = &header[start_pos..end_pos];

    // Extract the line numbers from the chunk header
    // Format: -old_start,old_lines +new_start,new_lines
    let parts: Vec<&str> = header_content.split_whitespace().collect();

    // Handle different header formats more flexibly
    let (old_part, new_part) = match parts.len() {
        2 => (parts[0], parts[1]),
        // Handle combined diff format or other variations
        _ => {
            let mut old_part = None;
            let mut new_part = None;

            for part in parts {
                if part.starts_with('-') {
                    old_part = Some(part);
                } else if part.starts_with('+') {
                    new_part = Some(part);
                }
            }

            (
                old_part.ok_or_else(|| {
                    Error::InvalidPatchFormat(format!(
                        "Missing old range in chunk header: {}",
                        header
                    ))
                })?,
                new_part.ok_or_else(|| {
                    Error::InvalidPatchFormat(format!(
                        "Missing new range in chunk header: {}",
                        header
                    ))
                })?,
            )
        }
    };

    // Parse the old range
    let old_range = old_part.strip_prefix('-').unwrap_or(old_part);
    let old_range_parts: Vec<&str> = old_range.split(',').collect();

    let (old_start, old_lines) = match old_range_parts.len() {
        1 => {
            // If only one number, assume it's just the start line with 1 line of context
            let start = parse_number(old_range_parts[0], "old start")?;
            (start, 1)
        }
        2 => {
            let start = parse_number(old_range_parts[0], "old start")?;
            let lines = parse_number(old_range_parts[1], "old lines")?;
            (start, lines)
        }
        _ => {
            return Err(Error::InvalidPatchFormat(format!(
                "Invalid old range format: {}",
                old_range
            )))
        }
    };

    // Parse the new range
    let new_range = new_part.strip_prefix('+').unwrap_or(new_part);
    let new_range_parts: Vec<&str> = new_range.split(',').collect();

    let (new_start, new_lines) = match new_range_parts.len() {
        1 => {
            // If only one number, assume it's just the start line with 1 line of context
            let start = parse_number(new_range_parts[0], "new start")?;
            (start, 1)
        }
        2 => {
            let start = parse_number(new_range_parts[0], "new start")?;
            let lines = parse_number(new_range_parts[1], "new lines")?;
            (start, lines)
        }
        _ => {
            return Err(Error::InvalidPatchFormat(format!(
                "Invalid new range format: {}",
                new_range
            )))
        }
    };

    // Adjust to 0-based indexing
    Ok((
        old_start.saturating_sub(1),
        old_lines,
        new_start.saturating_sub(1),
        new_lines,
    ))
}

/// Parse a number from a string with better error handling
fn parse_number(s: &str, field_name: &str) -> Result<usize, Error> {
    s.parse::<usize>()
        .map_err(|_| Error::InvalidPatchFormat(format!("Invalid {} number: {}", field_name, s)))
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

    #[test]
    fn test_parse_patch_with_extra_lines() {
        // This test verifies that we can handle extra lines in the patch
        // like git index lines and empty lines
        let patch_str = "--- a/file.txt\n+++ b/file.txt\n\n@@ -1,4 +1,4 @@\n line1\n-line2\n+line2 modified\n line3\n line4\n";

        match Patch::parse(patch_str) {
            Ok(patch) => {
                assert_eq!(patch.chunks.len(), 1);

                let chunk = &patch.chunks[0];
                assert_eq!(chunk.old_start, 0);
                assert_eq!(chunk.old_lines, 4);
                assert_eq!(chunk.new_start, 0);
                assert_eq!(chunk.new_lines, 4);
            }
            Err(e) => {
                panic!("Failed to parse patch: {}", e);
            }
        }

        // Test a different variant with preamble
        let patch_str2 = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,4 +1,4 @@\n line1\n-line2\n+line2 modified\n line3\n line4\n";

        let patch2 = Patch::parse(patch_str2).unwrap();
        assert_eq!(
            patch2.preemble,
            Some("diff --git a/file.txt b/file.txt".to_string())
        );
    }

    #[test]
    fn test_parse_patch_simple_header() {
        let patch_str = "\
--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-old content
+new content
";

        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.chunks.len(), 1);

        let chunk = &patch.chunks[0];
        assert_eq!(chunk.old_start, 0);
        assert_eq!(chunk.old_lines, 1);
        assert_eq!(chunk.new_start, 0);
        assert_eq!(chunk.new_lines, 1);

        assert_eq!(chunk.operations.len(), 2);
        if let Operation::Remove(line) = &chunk.operations[0] {
            assert_eq!(line, "old content");
        } else {
            panic!("Expected Remove operation");
        }

        if let Operation::Add(line) = &chunk.operations[1] {
            assert_eq!(line, "new content");
        } else {
            panic!("Expected Add operation");
        }
    }

    #[test]
    fn test_parse_patch_with_context() {
        let patch_str = "\
--- a/file.txt
+++ b/file.txt
@@ -10,6 +10,7 @@ context line before
 another context line
-removed line
+added line 1
+added line 2
 final context line
";

        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.chunks.len(), 1);

        let chunk = &patch.chunks[0];
        assert_eq!(chunk.old_start, 9);
        assert_eq!(chunk.old_lines, 6);
        assert_eq!(chunk.new_start, 9);
        assert_eq!(chunk.new_lines, 7);
    }
}

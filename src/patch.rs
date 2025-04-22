use crate::Error;
use std::fmt;

/// Represents a change operation in the patch
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Add a new line
    Add(String),
    /// Remove a line
    Remove(String),
    /// Context line (unchanged)
    Context(String),
}

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

/// A chunk represents a continuous section of changes in a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Starting line in the original file (0-based)
    pub old_start: usize,
    /// Number of lines in the original file
    pub old_lines: usize,
    /// Starting line in the new file (0-based)
    pub new_start: usize,
    /// Number of lines in the new file
    pub new_lines: usize,
    /// The operations in this chunk
    pub operations: Vec<Operation>,
}

impl fmt::Display for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "@@ -{},{} +{},{} @@",
            self.old_start + 1, // Display as 1-based index
            self.old_lines,
            self.new_start + 1, // Display as 1-based index
            self.new_lines
        )?;

        for op in &self.operations {
            writeln!(f, "{}{}", op.to_char(), op.line())?;
        }

        Ok(())
    }
}

/// A patch represents all the changes between two versions of a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// Preemble of the patch, something like "diff -u a/file.txt b/file.txt"
    pub preamble: Option<String>,
    /// Original file path, often prefixed with `a/`
    pub old_file: String,
    /// New file path, often prefixed with `b/`
    pub new_file: String,
    /// Chunks of changes
    pub chunks: Vec<Chunk>,
}

impl Patch {
    /// Parse a patch from a string following the unified diff format.
    pub fn parse(content: &str) -> Result<Self, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let mut line_iter = lines.iter().peekable();
        let mut current_line_num = 0;

        // --- Find Preamble and Headers ---
        let mut preamble: Option<String> = None;
        let mut old_file: Option<String> = None;
        let mut new_file: Option<String> = None;

        while let Some(line) = line_iter.peek() {
            current_line_num += 1;
            let line = line.trim_end(); // Handle potential trailing whitespace

            if line.starts_with("diff ") {
                if preamble.is_some() || old_file.is_some() || new_file.is_some() {
                    // Found a new diff header before finishing the previous one?
                    // This case might occur in concatenated diffs, treat as preamble for the *next* patch.
                    break;
                }
                preamble = Some(line.to_string());
                line_iter.next(); // Consume the preamble line
            } else if line.starts_with("--- ") {
                if old_file.is_some() {
                    return Err(Error::InvalidPatchFormat(format!(
                        "Duplicate '---' header found at line {}",
                        current_line_num
                    )));
                }
                old_file = Some(parse_file_header_line(line, "---")?);
                line_iter.next(); // Consume the old file header line
            } else if line.starts_with("+++ ") {
                if new_file.is_some() {
                    return Err(Error::InvalidPatchFormat(format!(
                        "Duplicate '+++' header found at line {}",
                        current_line_num
                    )));
                }
                if old_file.is_none() {
                    return Err(Error::InvalidPatchFormat(format!(
                        "'+++' header found before '---' header at line {}",
                        current_line_num
                    )));
                }
                new_file = Some(parse_file_header_line(line, "+++")?);
                line_iter.next(); // Consume the new file header line
                break; // Headers found, move to parsing chunks
            } else {
                // Skip other potential header lines like "index", "mode", etc.
                line_iter.next();
            }
        }

        let old_file = old_file
            .ok_or_else(|| Error::InvalidPatchFormat("Missing '---' header".to_string()))?;
        let new_file = new_file
            .ok_or_else(|| Error::InvalidPatchFormat("Missing '+++' header".to_string()))?;

        // --- Parse Chunks ---
        let mut chunks = Vec::new();
        while let Some(line) = line_iter.peek() {
            let line_content = line.trim_end();
            if line_content.is_empty() {
                // Skip empty lines between chunks
                line_iter.next();
                current_line_num += 1;
                continue;
            }

            if line_content.starts_with("@@ ") {
                line_iter.next(); // Consume chunk header line
                current_line_num += 1;
                let (old_start, old_lines, new_start, new_lines) =
                    parse_chunk_header(line_content)?;

                let mut operations = Vec::new();
                let mut actual_old_lines = 0;
                let mut actual_new_lines = 0;

                // Read all lines until next @@ or EOF
                while let Some(op_line_peek) = line_iter.peek() {
                    if op_line_peek.starts_with("@@ ") {
                        break; // Stop reading for this chunk
                    }

                    let op_line = op_line_peek.trim_end();
                    line_iter.next(); // Consume the line
                    current_line_num += 1;

                    // Parse the operation, requiring a prefix
                    if let Some(content) = op_line.strip_prefix('+') {
                        operations.push(Operation::Add(content.to_string()));
                        actual_new_lines += 1;
                    } else if let Some(content) = op_line.strip_prefix('-') {
                        operations.push(Operation::Remove(content.to_string()));
                        actual_old_lines += 1;
                    } else if let Some(content) = op_line.strip_prefix(' ') {
                        operations.push(Operation::Context(content.to_string()));
                        actual_old_lines += 1;
                        actual_new_lines += 1;
                    } else if op_line == "\\ No newline at end of file" || op_line.is_empty() {
                        // Ignore NOEOL marker and skip truly empty lines within chunk body
                        continue; // Ignore this marker
                    } else {
                        // Strict: No prefix is an error
                        return Err(Error::InvalidPatchFormat(format!(
                           "Line {}: Line without context/add/remove prefix found in chunk body: \"{}\"",
                           current_line_num, op_line
                        )));
                    }
                }

                // Validate counts AFTER reading the whole chunk
                if actual_old_lines != old_lines || actual_new_lines != new_lines {
                    return Err(Error::InvalidPatchFormat(format!(
                        "Chunk line count mismatch: Header expected (-{}, +{}), Parsed content counts (-{}, +{}). Chunk Header: {}",
                        old_lines, new_lines, actual_old_lines, actual_new_lines, line_content
                    )));
                }

                chunks.push(Chunk {
                    old_start,
                    old_lines,
                    new_start,
                    new_lines,
                    operations,
                });
            } else {
                // Line doesn't start with @@, and we are outside a chunk
                // Should only be preamble lines or errors
                return Err(Error::InvalidPatchFormat(format!(
                    "Unexpected content found outside of chunk: '{}' at line {}",
                    line_content, current_line_num
                )));
            }
        }

        Ok(Patch {
            preamble,
            old_file,
            new_file,
            chunks,
        })
    }
}

/// Parses the file path from a `---` or `+++` header line.
/// Handles optional `a/` or `b/` prefixes and potential timestamp info.
fn parse_file_header_line(line: &str, prefix: &str) -> Result<String, Error> {
    let content = line
        .strip_prefix(prefix)
        .ok_or_else(|| {
            Error::InvalidPatchFormat(format!("Invalid {} header format: {}", prefix, line))
        })?
        .trim_start(); // Remove leading space after `---` or `+++`

    // Git format often includes a/ or b/
    let path_part = content
        .strip_prefix("a/")
        .or_else(|| content.strip_prefix("b/"))
        .unwrap_or(content);

    // Strip potential timestamp/mode info separated by tabs or multiple spaces
    Ok(path_part
        .split(['\t', ' '])
        .next()
        .unwrap_or("")
        .to_string())
}

/// Parse a chunk header with more flexibility to handle various Git diff formats
/// Returns (old_start, old_lines, new_start, new_lines) - 0-based start index.
fn parse_chunk_header(header: &str) -> Result<(usize, usize, usize, usize), Error> {
    // Example: @@ -1,5 +1,6 @@ optional context
    let parts: Vec<&str> = header.split(" @@").collect();
    if !parts[0].starts_with("@@ ") || parts.len() < 2 {
        return Err(Error::InvalidChunkHeader {
            header: header.to_string(),
        });
    }

    let range_part = parts[0].strip_prefix("@@ ").unwrap().trim(); // "-1,5 +1,6"
    let range_parts: Vec<&str> = range_part.split_whitespace().collect();

    if range_parts.len() != 2
        || !range_parts[0].starts_with('-')
        || !range_parts[1].starts_with('+')
    {
        return Err(Error::InvalidChunkHeader {
            header: header.to_string(),
        });
    }

    // Parse old range: "-1,5"
    let old_range_str = range_parts[0].strip_prefix('-').unwrap();
    let (old_start, old_lines) = parse_range(old_range_str, header)?;

    // Parse new range: "+1,6"
    let new_range_str = range_parts[1].strip_prefix('+').unwrap();
    let (new_start, new_lines) = parse_range(new_range_str, header)?;

    // Adjust to 0-based indexing for start lines
    Ok((
        old_start.saturating_sub(1),
        old_lines,
        new_start.saturating_sub(1),
        new_lines,
    ))
}

/// Parses a range string like "1,5" or "1" into (start, count).
fn parse_range(range_str: &str, header: &str) -> Result<(usize, usize), Error> {
    let parts: Vec<&str> = range_str.splitn(2, ',').collect();
    let start_str = parts[0];
    let start = parse_number(start_str, "range start")?;

    let count = match parts.len() {
        1 => {
            // Format like "-1" or "+1" means 1 line affected unless start is 0
            if start == 0 {
                0
            } else {
                1
            }
        }
        2 => {
            // Format like "-1,5" or "+1,6"
            let count_str = parts[1];
            parse_number(count_str, "range count")?
        }
        _ => unreachable!(), // splitn(2,...) ensures max 2 parts
    };

    if start == 0 && count > 0 { // e.g. --- /dev/null, @@ -0,0 +1,5 @@
         // Allow count > 0 only if start is also > 0, or if start is 0 (empty file case)
         // This condition is slightly redundant with the start==0 check in match arms, but provides clarity
    } else if start > 0 && count == 0 {
        return Err(Error::InvalidChunkHeader {
            header: header.to_string(),
        });
        // If start > 0, count must be at least 1
        //        return Err(Error::InvalidPatchFormat(format!(
        //            "Invalid range format '{}': count cannot be 0 if start is non-zero", range_str
        //        )));
    }

    Ok((start, count))
}

/// Parse a number from a string with better error handling
fn parse_number(s: &str, field_name: &str) -> Result<usize, Error> {
    s.parse::<usize>().map_err(|e| Error::InvalidNumberFormat {
        value: s.to_string(),
        field: field_name.to_string(),
        source: e,
    })
}

impl fmt::Display for Patch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(preamble) = &self.preamble {
            writeln!(f, "{}", preamble)?;
        }
        // Always use the a/ b/ prefixes for consistency, even if not present in parsed paths
        writeln!(f, "--- a/{}", self.old_file)?;
        writeln!(f, "+++ b/{}", self.new_file)?;

        for chunk in &self.chunks {
            write!(f, "{}", chunk)?; // Chunk::fmt already includes newline
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Operation; // Explicit import needed if `use super::*` isn't used fully

    #[test]
    fn test_parse_simple_patch() {
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

        assert!(patch.preamble.is_none());
        assert_eq!(patch.old_file, "file.txt");
        assert_eq!(patch.new_file, "file.txt");
        assert_eq!(patch.chunks.len(), 1);

        let chunk = &patch.chunks[0];
        assert_eq!(chunk.old_start, 0);
        assert_eq!(chunk.old_lines, 4);
        assert_eq!(chunk.new_start, 0);
        assert_eq!(chunk.new_lines, 4);

        assert_eq!(chunk.operations.len(), 5);
        assert_eq!(chunk.operations[0], Operation::Context("line1".into()));
        assert_eq!(chunk.operations[1], Operation::Remove("line2".into()));
        assert_eq!(chunk.operations[2], Operation::Add("line2 modified".into()));
        assert_eq!(chunk.operations[3], Operation::Context("line3".into()));
        assert_eq!(chunk.operations[4], Operation::Context("line4".into()));
    }

    #[test]
    fn test_parse_with_preamble() {
        let patch_str = "\
diff --git a/file.txt b/file.txt
index 12345..67890 100644
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-hello
+world
";
        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(
            patch.preamble,
            Some("diff --git a/file.txt b/file.txt".to_string())
        );
        assert_eq!(patch.old_file, "file.txt");
        assert_eq!(patch.new_file, "file.txt");
        assert_eq!(patch.chunks.len(), 1);
        assert_eq!(patch.chunks[0].old_start, 0);
        assert_eq!(patch.chunks[0].old_lines, 1);
        assert_eq!(patch.chunks[0].new_start, 0);
        assert_eq!(patch.chunks[0].new_lines, 1);
        assert_eq!(patch.chunks[0].operations.len(), 2);
    }

    #[test]
    fn test_parse_new_file() {
        let patch_str = "\
--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,3 @@
+line one
+line two
+line three
";
        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.old_file, "/dev/null");
        assert_eq!(patch.new_file, "new_file.txt");
        assert_eq!(patch.chunks.len(), 1);
        let chunk = &patch.chunks[0];
        assert_eq!(chunk.old_start, 0);
        assert_eq!(chunk.old_lines, 0);
        assert_eq!(chunk.new_start, 0);
        assert_eq!(chunk.new_lines, 3);
        assert_eq!(chunk.operations.len(), 3);
        assert!(matches!(chunk.operations[0], Operation::Add(_)));
        assert!(matches!(chunk.operations[1], Operation::Add(_)));
        assert!(matches!(chunk.operations[2], Operation::Add(_)));
    }

    #[test]
    fn test_parse_delete_file() {
        let patch_str = "\
--- a/old_file.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-content line 1
-content line 2
";
        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.old_file, "old_file.txt");
        assert_eq!(patch.new_file, "/dev/null");
        assert_eq!(patch.chunks.len(), 1);
        let chunk = &patch.chunks[0];
        assert_eq!(chunk.old_start, 0);
        assert_eq!(chunk.old_lines, 2);
        assert_eq!(chunk.new_start, 0);
        assert_eq!(chunk.new_lines, 0);
        assert_eq!(chunk.operations.len(), 2);
        assert!(matches!(chunk.operations[0], Operation::Remove(_)));
        assert!(matches!(chunk.operations[1], Operation::Remove(_)));
    }

    #[test]
    fn test_parse_patch_with_context() {
        let patch_str = "\
--- a/file.txt
+++ b/file.txt
@@ -10,3 +10,4 @@ context line before
 another context line
-removed line
+added line 1
+added line 2
 final context line
";

        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.chunks.len(), 1);

        let chunk = &patch.chunks[0];
        assert_eq!(chunk.old_start, 9); // 10 becomes 9 (0-based)
        assert_eq!(chunk.old_lines, 3);
        assert_eq!(chunk.new_start, 9); // 10 becomes 9 (0-based)
        assert_eq!(chunk.new_lines, 4);
        assert_eq!(chunk.operations.len(), 5); // Context + Remove + Add + Add + Context = 5
    }

    #[test]
    fn test_parse_header_with_timestamps() {
        let patch_str = "\
--- a/file.txt\t2023-01-01 10:00:00.000000000 +0000
+++ b/file.txt\t2023-01-01 10:01:00.000000000 +0000
@@ -1 +1 @@
-a
+b
";
        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.old_file, "file.txt");
        assert_eq!(patch.new_file, "file.txt");
    }

    #[test]
    fn test_parse_header_no_prefix() {
        let patch_str = "\
--- file.txt
+++ file.txt
@@ -1 +1 @@
-a
+b
";
        let patch = Patch::parse(patch_str).unwrap();
        assert_eq!(patch.old_file, "file.txt");
        assert_eq!(patch.new_file, "file.txt");
    }

    #[test]
    fn test_parse_empty_patch() {
        let patch_str = "---
+++
";
        let result = Patch::parse(patch_str);
        assert!(matches!(result, Err(Error::InvalidPatchFormat(_))));

        let patch_str_2 = "--- a/file.txt
+++
";
        let result_2 = Patch::parse(patch_str_2);
        assert!(matches!(result_2, Err(Error::InvalidPatchFormat(_))));
    }

    #[test]
    fn test_parse_malformed_header() {
        let patch_str = "\
--- a/first.txt
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-a
+b
";
        let result = Patch::parse(patch_str);
        // Check the specific error message
        assert!(
            matches!(result, Err(Error::InvalidPatchFormat(s)) if s.contains("Duplicate '---' header"))
        );

        let patch_str_2 = "+++ b/file.txt
--- a/file.txt
@@ -1,1 +1,1 @@
-a
+b
";
        let result_2 = Patch::parse(patch_str_2);
        // Check the specific error message
        assert!(
            matches!(result_2, Err(Error::InvalidPatchFormat(s)) if s.contains("'+++' header found before '---' header"))
        );
    }

    #[test]
    fn test_parse_malformed_chunk_header() {
        let patch_str = "\
--- a/file.txt
+++ b/file.txt
@@ malformed @@
-a
+b
";
        let result = Patch::parse(patch_str);
        assert!(matches!(result, Err(Error::InvalidChunkHeader { .. })));

        let patch_str_2 = "\
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
+b
-a
";
        let result_2 = Patch::parse(patch_str_2);
        // This should parse OK now, but the resulting patch might be weird/unapplicable
        // It reads +b (actual_new=1), then -a (actual_old=1). Loop stops.
        // Validation checks: actual_old(1) == old_lines(1), actual_new(1) == new_lines(1). OK.
        assert!(result_2.is_ok());
        if let Ok(patch) = result_2 {
            assert_eq!(patch.chunks[0].operations.len(), 2);
            assert!(matches!(patch.chunks[0].operations[0], Operation::Add(_)));
            assert!(matches!(
                patch.chunks[0].operations[1],
                Operation::Remove(_)
            ));
        }
    }

    #[test]
    fn test_parse_chunk_line_count_mismatch() {
        // More lines than expected - Should fail validation *after* reading
        let patch_str_more = "\
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-a
+b
+c // Extra add line
";
        let result_more = Patch::parse(patch_str_more);
        assert!(matches!(result_more, Err(Error::InvalidPatchFormat(_))));
        let err_str = result_more.err().unwrap().to_string();
        assert!(err_str.contains("Chunk line count mismatch"));
        // Check the reported parsed counts
        assert!(
            err_str.contains("Parsed content counts (-1, +2)"),
            "Error was: {}",
            err_str
        );

        // Fewer lines than expected - Should fail validation *after* reading
        let patch_str_less = "\
--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
-a
+b
"; // Missing lines, stops before next @@ or EOF
        let result_less = Patch::parse(patch_str_less);
        assert!(matches!(result_less, Err(Error::InvalidPatchFormat(_))));
        let err_str_less = result_less.err().unwrap().to_string();
        assert!(err_str_less.contains("Chunk line count mismatch"));
        // Check the reported parsed counts
        assert!(
            err_str_less.contains("Parsed content counts (-1, +1)"),
            "Error was: {}",
            err_str_less
        );
    }

    #[test]
    fn test_parse_line_without_prefix_in_chunk() {
        let patch_str = "\
--- a/file.txt
+++ b/file.txt
@@ -0,0 +0,1 @@
invalid_line_without_prefix
";
        // Expect InvalidPatchFormat because strict prefix is required
        let result = Patch::parse(patch_str);
        assert!(
            matches!(result, Err(Error::InvalidPatchFormat(s)) if s.contains("Line without context/add/remove prefix"))
        );
    }

    #[test]
    fn test_display_patch() {
        let patch = Patch {
            preamble: Some("diff -u a/old b/new".to_string()),
            old_file: "old".to_string(),
            new_file: "new".to_string(),
            chunks: vec![
                Chunk {
                    old_start: 0,
                    old_lines: 2,
                    new_start: 0,
                    new_lines: 3,
                    operations: vec![
                        Operation::Context("line1".into()),
                        Operation::Remove("line2".into()),
                        Operation::Add("line2 mod".into()),
                        Operation::Add("line2.5".into()),
                    ],
                },
                Chunk {
                    old_start: 5,
                    old_lines: 1,
                    new_start: 7,
                    new_lines: 1,
                    operations: vec![Operation::Context("line6".into())],
                },
            ],
        };

        let expected_str = "\
diff -u a/old b/new
--- a/old
+++ b/new
@@ -1,2 +1,3 @@
 line1
-line2
+line2 mod
+line2.5
@@ -6,1 +8,1 @@
 line6
";
        assert_eq!(patch.to_string().trim(), expected_str.trim());
    }
}

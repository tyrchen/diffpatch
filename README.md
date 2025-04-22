![](https://github.com/tyrchen/rust-lib-template/workflows/build/badge.svg)

# Diffpatch

A Rust library for generating and applying Git-style unified diff patches.

## Features

- Generate patches from original and modified content
- Apply patches to content, both forward and in reverse
- Parse patches from text format
- Command-line interface for generating and applying patches

## Installation

Add to your Cargo.toml:

```toml
[dependencies]
diffpatch = "0.1.0"
```

Or install the CLI tool:

```bash
cargo install diffpatch
```

## Library Usage

### Generate a Patch

```rust
use diffpatch::Differ;

fn main() {
    let old_content = "line1\nline2\nline3\nline4";
    let new_content = "line1\nline2 modified\nline3\nline4";

    let differ = Differ::new(old_content, new_content);
    let patch = differ.generate();

    println!("{}", patch);
}
```

### Apply a Patch

```rust
use diffpatch::{Differ, Patcher};

fn main() {
    let old_content = "line1\nline2\nline3\nline4";
    let new_content = "line1\nline2 modified\nline3\nline4";

    // Generate a patch
    let differ = Differ::new(old_content, new_content);
    let patch = differ.generate();

    // Apply it to the original content
    let patcher = Patcher::new(patch);
    let result = patcher.apply(old_content, false).unwrap();

    assert_eq!(result, new_content);
}
```

### Parse a Patch

```rust
use diffpatch::Patch;

fn main() {
    let patch_content = "\
--- a/file.txt
+++ b/file.txt
@@ -1,4 +1,4 @@
 line1
-line2
+line2 modified
 line3
 line4
";

    let patch = Patch::parse(patch_content).unwrap();

    println!("Original file: {}", patch.old_file);
    println!("Modified file: {}", patch.new_file);
    println!("Number of chunks: {}", patch.chunks.len());
}
```

## CLI Usage

### Generate a Patch

```bash
diffpatch-cli generate --old original_file.txt --new modified_file.txt --output patch.diff
```

### Apply a Patch

```bash
diffpatch-cli apply --patch patch.diff --file original_file.txt --output result.txt
```

### Apply a Patch in Reverse

```bash
diffpatch-cli apply --patch patch.diff --file modified_file.txt --output original.txt --reverse
```

## Data Structures

- `Diff`: Represents a complete diff between two files
- `Hunk`: Represents a contiguous section of changes
- `DiffLine`: Represents a single line in a diff (addition, deletion, or context)
- `LineType`: Enum for the type of change a line represents

## Limitations

- The current diff creation algorithm is naive and doesn't create optimal diffs
- Limited support for various diff formats (focuses on git-style diffs)

## License

This project is distributed under the terms of MIT.

See [LICENSE](LICENSE.md) for details.

Copyright 2025 Tyr Chen

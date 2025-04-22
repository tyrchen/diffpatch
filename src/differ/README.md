# Differ Module

This module contains different implementations of diffing algorithms used to generate patches between text files.

## Overview

The differ module provides a trait-based approach to implementing different diffing algorithms. Each algorithm must implement the `DiffAlgorithm` trait with its `generate()` method that produces a `Patch` result.

## Algorithms

### NaiveDiffer

A simple diffing algorithm that looks for line-by-line changes and tries to match lines that are equal. This algorithm works well for most cases but might not produce the most optimal diffs for complex changes.

### MyersDiffer

An implementation of the Myers algorithm for diffing, which is known to find the shortest edit script between two sequences. This algorithm is more sophisticated and generally produces better diffs than the naive approach, especially for complex changes.

## Usage

```rust
use crate::{Differ, Patch, Patcher};
use crate::differ::{DiffAlgorithm, NaiveDiffer, MyersDiffer};

// Create a base differ with the old and new content
let differ = Differ::new(old_text, new_text);

// Use the naive algorithm (this is the default when calling differ.generate())
let naive = NaiveDiffer::new(&differ);
let patch = naive.generate();

// Or use the Myers algorithm for potentially better diffs
let myers = MyersDiffer::new(&differ);
let patch = myers.generate();

// Apply the patch
let patcher = Patcher::new(patch);
let result = patcher.apply(old_text, false).unwrap();
```

## Adding New Algorithms

To add a new diffing algorithm:

1. Create a new file for your algorithm implementation
2. Define a struct that holds a reference to the base `Differ`
3. Implement the `DiffAlgorithm` trait for your struct
4. Add your algorithm to the module exports in `mod.rs`

## Testing

Each algorithm has its own test suite to ensure it correctly handles various edge cases and common scenarios.

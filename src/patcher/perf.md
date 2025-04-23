# Performance Comparison: Naive vs Similar Patchers

## Implementation Differences

### NaivePatcher

- **Simple approach**: Applies patches at exact line numbers specified in the patch
- **Exact matching**: Requires perfect matches for context lines
- **No search logic**: Does not attempt to find alternative locations for patches
- **Minimal overhead**: Uses basic string comparisons and data structures
- **Strict error handling**: Fails immediately on any context mismatch

### SimilarPatcher

- **Sophisticated approach**: Uses fuzzy matching to find best location for patches
- **Flexible matching**: Can handle whitespace differences, typos, and other minor variations
- **Search mechanism**: Searches within a range to find optimal patch locations
- **Multiple fallback strategies**:
  - Exact match at expected position
  - Fuzzy matching with configurable thresholds
  - Partial matching for subset of context
- **Complex string handling**: Uses Levenshtein distance, whitespace normalization, and similarity scoring

## Performance Considerations

The naive implementation likely performs better than the similar implementation for several reasons:

1. **Computational Complexity**:
   - `NaivePatcher` performs simple string equality checks (O(n) where n is string length)
   - `SimilarPatcher` calculates Levenshtein distances (O(n*m) where n and m are string lengths)

2. **Search Overhead**:
   - `NaivePatcher` applies operations at fixed positions without searching
   - `SimilarPatcher` may scan up to `SEARCH_RANGE` (50) lines both before and after the expected position

3. **Memory Usage**:
   - `NaivePatcher` creates minimal intermediate data structures
   - `SimilarPatcher` creates additional vectors and strings for fuzzy matching

4. **Algorithmic Branching**:
   - `NaivePatcher` has simple, predictable execution paths
   - `SimilarPatcher` has multiple fallback strategies with complex branching logic

5. **String Operations**:
   - `NaivePatcher` performs simple string equality checks
   - `SimilarPatcher` performs multiple string operations including normalization and similarity scoring

## When to Use Each

- **NaivePatcher**: When performance is critical and patches will be applied to content that exactly matches the context
- **SimilarPatcher**: When robustness is more important than performance, especially when:
  - Content may have minor variations from what the patch expects
  - Line numbers might have shifted slightly
  - Whitespace differences exist
  - Exact context matching would fail but semantically equivalent matches would work

## Trade-offs

The performance vs. flexibility trade-off is clear:

- `NaivePatcher` sacrifices flexibility for speed
- `SimilarPatcher` sacrifices speed for flexibility

The choice between them depends on the specific requirements of the application and the nature of the content being patched.

## Performance Results

```bash
cargo bench --bench patcher
patcher             fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ naive_forward                  │               │               │               │         │
│  ├─ 0             241.2 ns      │ 8.616 µs      │ 241.2 ns      │ 350.8 ns      │ 100     │ 100
│  │                max alloc:    │               │               │               │         │
│  │                  2           │ 2             │ 2             │ 2             │         │
│  │                  374 B       │ 374 B         │ 374 B         │ 374 B         │         │
│  │                alloc:        │               │               │               │         │
│  │                  2           │ 2             │ 2             │ 2             │         │
│  │                  187 B       │ 187 B         │ 187 B         │ 187 B         │         │
│  │                dealloc:      │               │               │               │         │
│  │                  2           │ 2             │ 2             │ 2             │         │
│  │                  251 B       │ 251 B         │ 251 B         │ 251 B         │         │
│  │                grow:         │               │               │               │         │
│  │                  2           │ 2             │ 2             │ 2             │         │
│  │                  187 B       │ 187 B         │ 187 B         │ 187 B         │         │
│  ╰─ 1             1.256 µs      │ 2.131 µs      │ 1.288 µs      │ 1.298 µs      │ 100     │ 400
│                   max alloc:    │               │               │               │         │
│                     0.5         │ 0.5           │ 0.5           │ 0.5           │         │
│                     2.004 KB    │ 2.004 KB      │ 2.004 KB      │ 2.004 KB      │         │
│                   alloc:        │               │               │               │         │
│                     2           │ 2             │ 2             │ 2             │         │
│                     1.463 KB    │ 1.463 KB      │ 1.463 KB      │ 1.463 KB      │         │
│                   dealloc:      │               │               │               │         │
│                     2           │ 2             │ 2             │ 2             │         │
│                     2.423 KB    │ 2.423 KB      │ 2.423 KB      │ 2.423 KB      │         │
│                   grow:         │               │               │               │         │
│                     5           │ 5             │ 5             │ 5             │         │
│                     2.359 KB    │ 2.359 KB      │ 2.359 KB      │ 2.359 KB      │         │
├─ naive_reverse                  │               │               │               │         │
│  ├─ 0             871.1 ns      │ 1.256 µs      │ 892.1 ns      │ 923.5 ns      │ 100     │ 400
│  │                max alloc:    │               │               │               │         │
│  │                  4.25        │ 4.25          │ 4.25          │ 4.25          │         │
│  │                  332.5 B     │ 332.5 B       │ 332.5 B       │ 332.5 B       │         │
│  │                alloc:        │               │               │               │         │
│  │                  17          │ 17            │ 17            │ 17            │         │
│  │                  1.138 KB    │ 1.138 KB      │ 1.138 KB      │ 1.138 KB      │         │
│  │                dealloc:      │               │               │               │         │
│  │                  17          │ 17            │ 17            │ 17            │         │
│  │                  1.42 KB     │ 1.42 KB       │ 1.42 KB       │ 1.42 KB       │         │
│  │                grow:         │               │               │               │         │
│  │                  2           │ 2             │ 2             │ 2             │         │
│  │                  192 B       │ 192 B         │ 192 B         │ 192 B         │         │
│  ╰─ 1             3.774 µs      │ 7.232 µs      │ 3.899 µs      │ 4.013 µs      │ 100     │ 100
│                   max alloc:    │               │               │               │         │
│                     38          │ 38            │ 38            │ 38            │         │
│                     4.992 KB    │ 4.992 KB      │ 4.992 KB      │ 4.992 KB      │         │
│                   alloc:        │               │               │               │         │
│                     72          │ 72            │ 72            │ 72            │         │
│                     6.258 KB    │ 6.258 KB      │ 6.258 KB      │ 6.258 KB      │         │
│                   dealloc:      │               │               │               │         │
│                     72          │ 72            │ 72            │ 72            │         │
│                     7.218 KB    │ 7.218 KB      │ 7.218 KB      │ 7.218 KB      │         │
│                   grow:         │               │               │               │         │
│                     4           │ 4             │ 4             │ 4             │         │
│                     960 B       │ 960 B         │ 960 B         │ 960 B         │         │
├─ similar_forward                │               │               │               │         │
│  ├─ 0             2.155 µs      │ 6.989 µs      │ 2.239 µs      │ 2.32 µs       │ 100     │ 100
│  │                max alloc:    │               │               │               │         │
│  │                  6           │ 6             │ 6             │ 6             │         │
│  │                  793 B       │ 793 B         │ 793 B         │ 793 B         │         │
│  │                alloc:        │               │               │               │         │
│  │                  20          │ 20            │ 20            │ 20            │         │
│  │                  1.843 KB    │ 1.843 KB      │ 1.843 KB      │ 1.843 KB      │         │
│  │                dealloc:      │               │               │               │         │
│  │                  20          │ 20            │ 20            │ 20            │         │
│  │                  1.872 KB    │ 1.872 KB      │ 1.872 KB      │ 1.872 KB      │         │
│  │                grow:         │               │               │               │         │
│  │                  2           │ 2             │ 2             │ 2             │         │
│  │                  128 B       │ 128 B         │ 128 B         │ 128 B         │         │
│  ╰─ 1             121.2 µs      │ 187.1 µs      │ 141.2 µs      │ 140.9 µs      │ 100     │ 100
│                   max alloc:    │               │               │               │         │
│                     8           │ 8             │ 8             │ 8             │         │
│                     3.919 KB    │ 3.919 KB      │ 3.919 KB      │ 3.919 KB      │         │
│                   alloc:        │               │               │               │         │
│                     492         │ 492           │ 492           │ 492           │         │
│                     49.2 KB     │ 49.2 KB       │ 49.2 KB       │ 49.2 KB       │         │
│                   dealloc:      │               │               │               │         │
│                     492         │ 492           │ 492           │ 492           │         │
│                     50.57 KB    │ 50.57 KB      │ 50.57 KB      │ 50.57 KB      │         │
│                   grow:         │               │               │               │         │
│                     9           │ 9             │ 9             │ 9             │         │
│                     1.6 KB      │ 1.6 KB        │ 1.6 KB        │ 1.6 KB        │         │
╰─ similar_reverse                │               │               │               │         │
   ├─ 0             6.766 µs      │ 13.18 µs      │ 6.892 µs      │ 7.232 µs      │ 100     │ 100
   │                max alloc:    │               │               │               │         │
   │                  20          │ 20            │ 20            │ 20            │         │
   │                  1.947 KB    │ 1.947 KB      │ 1.947 KB      │ 1.947 KB      │         │
   │                alloc:        │               │               │               │         │
   │                  59          │ 59            │ 59            │ 59            │         │
   │                  4.685 KB    │ 4.685 KB      │ 4.685 KB      │ 4.685 KB      │         │
   │                dealloc:      │               │               │               │         │
   │                  59          │ 59            │ 59            │ 59            │         │
   │                  4.842 KB    │ 4.842 KB      │ 4.842 KB      │ 4.842 KB      │         │
   │                grow:         │               │               │               │         │
   │                  3           │ 3             │ 3             │ 3             │         │
   │                  256 B       │ 256 B         │ 256 B         │ 256 B         │         │
   ╰─ 1             143.8 µs      │ 271.2 µs      │ 153.5 µs      │ 159.4 µs      │ 100     │ 100
                    max alloc:    │               │               │               │         │
                      42          │ 42            │ 42            │ 42            │         │
                      6.514 KB    │ 6.514 KB      │ 6.514 KB      │ 6.514 KB      │         │
                    alloc:        │               │               │               │         │
                      492         │ 492           │ 492           │ 492           │         │
                      57.91 KB    │ 57.91 KB      │ 57.91 KB      │ 57.91 KB      │         │
                    dealloc:      │               │               │               │         │
                      492         │ 492           │ 492           │ 492           │         │
                      59.28 KB    │ 59.28 KB      │ 59.28 KB      │ 59.28 KB      │         │
                    grow:         │               │               │               │         │
                      9           │ 9             │ 9             │ 9             │         │
                      1.6 KB      │ 1.6 KB        │ 1.6 KB        │ 1.6 KB        │         │
```

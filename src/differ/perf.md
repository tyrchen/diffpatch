# Performance Analysis: NaiveDiffer vs XDiffDiffer

## NaiveDiffer Implementation

### Algorithm Overview

The NaiveDiffer implements a simple, greedy strategy that iterates through both files line by line.

### Key Characteristics

- **Matching Strategy**: When lines match, it advances to the next line in both files (Change::Equal)
- **Mismatch Handling**: Uses `find_next_match` with a fixed lookahead of 10 lines in both files
- **Complexity**: Linear progression through files with at most 100 comparisons in mismatch cases
- **Performance**: Fast but suboptimal - prioritizes speed over finding the shortest possible diff
- **Overhead**: Minimal, primarily string comparisons and basic vector operations

## XDiffDiffer Implementation

The XDiffDiffer implements a variation of the Myers diff algorithm, adapted from LibXDiff, designed to find the shortest edit script.

### Key Components

1. **Preprocessing**
   - Calculates hash values for every line in both files

2. **Core Logic**
   - Uses dynamic programming with K-vectors (kvd)
   - Implements "middle snake" finding through `find_split_point`
   - Maintains forward and backward search paths
   - Tracks edit sequences as diagonals in the edit graph

3. **Optimization Features**
   - Recursive problem solving (`compare_recursive`)
   - Heuristics for search space pruning (mxcost, heur_min, snake_cnt)

### Performance Characteristics

- **Complexity**: O((N+M)D) where N and M are file lengths and D is edit distance
- **Overhead**:
  - Line hashing
  - K-vector array management
  - Recursive function calls
  - Complex diagonal tracking and overlap calculations

## Performance Comparison

XDiffDiffer Slower Due To:

1. Complex algorithm design for optimal diff finding
2. Significant preprocessing overhead
3. Higher memory requirements
4. Complex path tracking calculations
5. Recursive implementation

NaiveDiffer Faster Due To:

1. Simple greedy approach
2. Limited lookahead strategy
3. Minimal computational overhead
4. Linear progression through files

## Conclusion

While XDiffDiffer produces more optimal diffs, its complexity results in higher computational overhead. The NaiveDiffer's simpler approach, though suboptimal, provides significantly better performance in benchmarks.

## Performance Results

```bash
cargo bench --bench differ
differ              fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ myers_algorithm                │               │               │               │         │
│  ├─ 1000          1.951 µs      │ 17.82 µs      │ 2.555 µs      │ 2.663 µs      │ 100     │ 100
│  │                max alloc:    │               │               │               │         │
│  │                  39          │ 39            │ 39            │ 39            │         │
│  │                  6.772 KB    │ 6.772 KB      │ 6.772 KB      │ 6.772 KB      │         │
│  │                alloc:        │               │               │               │         │
│  │                  54          │ 54            │ 54            │ 54            │         │
│  │                  5.908 KB    │ 5.908 KB      │ 5.908 KB      │ 5.908 KB      │         │
│  │                dealloc:      │               │               │               │         │
│  │                  20          │ 20            │ 20            │ 20            │         │
│  │                  4.88 KB     │ 4.88 KB       │ 4.88 KB       │ 4.88 KB       │         │
│  │                grow:         │               │               │               │         │
│  │                  11          │ 11            │ 11            │ 11            │         │
│  │                  2.208 KB    │ 2.208 KB      │ 2.208 KB      │ 2.208 KB      │         │
│  ├─ 10000         34.15 µs      │ 50.85 µs      │ 34.42 µs      │ 35.5 µs       │ 100     │ 100
│  │                max alloc:    │               │               │               │         │
│  │                  282         │ 282           │ 282           │ 282           │         │
│  │                  127.4 KB    │ 127.4 KB      │ 127.4 KB      │ 127.4 KB      │         │
│  │                alloc:        │               │               │               │         │
│  │                  429         │ 429           │ 429           │ 429           │         │
│  │                  127.3 KB    │ 127.3 KB      │ 127.3 KB      │ 127.3 KB      │         │
│  │                dealloc:      │               │               │               │         │
│  │                  152         │ 152           │ 152           │ 152           │         │
│  │                  127.4 KB    │ 127.4 KB      │ 127.4 KB      │ 127.4 KB      │         │
│  │                grow:         │               │               │               │         │
│  │                  26          │ 26            │ 26            │ 26            │         │
│  │                  36.51 KB    │ 36.51 KB      │ 36.51 KB      │ 36.51 KB      │         │
│  ╰─ 100000        2.908 ms      │ 4.265 ms      │ 3.318 ms      │ 3.362 ms      │ 100     │ 100
│                   max alloc:    │               │               │               │         │
│                     2935        │ 2935          │ 2935          │ 2935          │         │
│                     9.495 MB    │ 9.495 MB      │ 9.495 MB      │ 9.495 MB      │         │
│                   alloc:        │               │               │               │         │
│                     4432        │ 4432          │ 4432          │ 4432          │         │
│                     9.528 MB    │ 9.528 MB      │ 9.528 MB      │ 9.528 MB      │         │
│                   dealloc:      │               │               │               │         │
│                     1502        │ 1502          │ 1502          │ 1502          │         │
│                     9.495 MB    │ 9.495 MB      │ 9.495 MB      │ 9.495 MB      │         │
│                   grow:         │               │               │               │         │
│                     38          │ 38            │ 38            │ 38            │         │
│                     294.5 KB    │ 294.5 KB      │ 294.5 KB      │ 294.5 KB      │         │
├─ naive_algorithm                │               │               │               │         │
│  ├─ 1000          1.613 µs      │ 4.28 µs       │ 1.697 µs      │ 1.979 µs      │ 100     │ 100
│  │                max alloc:    │               │               │               │         │
│  │                  39          │ 39            │ 39            │ 39            │         │
│  │                  6.772 KB    │ 6.772 KB      │ 6.772 KB      │ 6.772 KB      │         │
│  │                alloc:        │               │               │               │         │
│  │                  39          │ 39            │ 39            │ 39            │         │
│  │                  4.564 KB    │ 4.564 KB      │ 4.564 KB      │ 4.564 KB      │         │
│  │                dealloc:      │               │               │               │         │
│  │                  5           │ 5             │ 5             │ 5             │         │
│  │                  3.536 KB    │ 3.536 KB      │ 3.536 KB      │ 3.536 KB      │         │
│  │                grow:         │               │               │               │         │
│  │                  11          │ 11            │ 11            │ 11            │         │
│  │                  2.208 KB    │ 2.208 KB      │ 2.208 KB      │ 2.208 KB      │         │
│  ├─ 10000         11.76 µs      │ 20.68 µs      │ 13.22 µs      │ 13.22 µs      │ 100     │ 100
│  │                max alloc:    │               │               │               │         │
│  │                  282         │ 282           │ 282           │ 282           │         │
│  │                  70.65 KB    │ 70.65 KB      │ 70.65 KB      │ 70.65 KB      │         │
│  │                alloc:        │               │               │               │         │
│  │                  282         │ 282           │ 282           │ 282           │         │
│  │                  40.28 KB    │ 40.28 KB      │ 40.28 KB      │ 40.28 KB      │         │
│  │                dealloc:      │               │               │               │         │
│  │                  5           │ 5             │ 5             │ 5             │         │
│  │                  34.33 KB    │ 34.33 KB      │ 34.33 KB      │ 34.33 KB      │         │
│  │                grow:         │               │               │               │         │
│  │                  25          │ 25            │ 25            │ 25            │         │
│  │                  30.36 KB    │ 30.36 KB      │ 30.36 KB      │ 30.36 KB      │         │
│  ╰─ 100000        153 µs        │ 307.9 µs      │ 167.6 µs      │ 172.7 µs      │ 100     │ 100
│                   max alloc:    │               │               │               │         │
│                     2942        │ 2942          │ 2942          │ 2942          │         │
│                     691.9 KB    │ 691.9 KB      │ 691.9 KB      │ 691.9 KB      │         │
│                   alloc:        │               │               │               │         │
│                     2942        │ 2942          │ 2942          │ 2942          │         │
│                     397.4 KB    │ 397.4 KB      │ 397.4 KB      │ 397.4 KB      │         │
│                   dealloc:      │               │               │               │         │
│                     5           │ 5             │ 5             │ 5             │         │
│                     363.8 KB    │ 363.8 KB      │ 363.8 KB      │ 363.8 KB      │         │
│                   grow:         │               │               │               │         │
│                     38          │ 38            │ 38            │ 38            │         │
│                     294.5 KB    │ 294.5 KB      │ 294.5 KB      │ 294.5 KB      │         │
╰─ xdiff_algorithm                │               │               │               │         │
   ├─ 1000          3.023 µs      │ 8.773 µs      │ 3.085 µs      │ 3.374 µs      │ 100     │ 100
   │                max alloc:    │               │               │               │         │
   │                  39          │ 39            │ 39            │ 39            │         │
   │                  6.1 KB      │ 6.1 KB        │ 6.1 KB        │ 6.1 KB        │         │
   │                alloc:        │               │               │               │         │
   │                  44          │ 44            │ 44            │ 44            │         │
   │                  5.378 KB    │ 5.378 KB      │ 5.378 KB      │ 5.378 KB      │         │
   │                dealloc:      │               │               │               │         │
   │                  10          │ 10            │ 10            │ 10            │         │
   │                  3.678 KB    │ 3.678 KB      │ 3.678 KB      │ 3.678 KB      │         │
   │                grow:         │               │               │               │         │
   │                  8           │ 8             │ 8             │ 8             │         │
   │                  1.536 KB    │ 1.536 KB      │ 1.536 KB      │ 1.536 KB      │         │
   ├─ 10000         84.38 µs      │ 128.6 µs      │ 92.3 µs       │ 92.98 µs      │ 100     │ 100
   │                max alloc:    │               │               │               │         │
   │                  282         │ 282           │ 282           │ 282           │         │
   │                  65.27 KB    │ 65.27 KB      │ 65.27 KB      │ 65.27 KB      │         │
   │                alloc:        │               │               │               │         │
   │                  287         │ 287           │ 287           │ 287           │         │
   │                  47.52 KB    │ 47.52 KB      │ 47.52 KB      │ 47.52 KB      │         │
   │                dealloc:      │               │               │               │         │
   │                  10          │ 10            │ 10            │ 10            │         │
   │                  36.19 KB    │ 36.19 KB      │ 36.19 KB      │ 36.19 KB      │         │
   │                grow:         │               │               │               │         │
   │                  22          │ 22            │ 22            │ 22            │         │
   │                  24.99 KB    │ 24.99 KB      │ 24.99 KB      │ 24.99 KB      │         │
   ╰─ 100000        2.092 ms      │ 2.534 ms      │ 2.237 ms      │ 2.252 ms      │ 100     │ 100
                    max alloc:    │               │               │               │         │
                      2939        │ 2939          │ 2939          │ 2939          │         │
                      599.8 KB    │ 599.8 KB      │ 599.8 KB      │ 599.8 KB      │         │
                    alloc:        │               │               │               │         │
                      2944        │ 2944          │ 2944          │ 2944          │         │
                      472.8 KB    │ 472.8 KB      │ 472.8 KB      │ 472.8 KB      │         │
                    dealloc:      │               │               │               │         │
                      10          │ 10            │ 10            │ 10            │         │
                      347 KB      │ 347 KB        │ 347 KB        │ 347 KB        │         │
                    grow:         │               │               │               │         │
                      34          │ 34            │ 34            │ 34            │         │
                      202.4 KB    │ 202.4 KB      │ 202.4 KB      │ 202.4 KB      │         │
```

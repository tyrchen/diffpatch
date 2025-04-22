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
differ      fastest       │ slowest       │ median        │ mean          │ samples │ iters
├─ myers                  │               │               │               │         │
│  ├─ 0     1.714 µs      │ 37.92 µs      │ 1.798 µs      │ 2.347 µs      │ 100     │ 100
│  │        max alloc:    │               │               │               │         │
│  │          22          │ 22            │ 22            │ 22            │         │
│  │          2.137 KB    │ 2.137 KB      │ 2.137 KB      │ 2.137 KB      │         │
│  │        alloc:        │               │               │               │         │
│  │          32          │ 32            │ 32            │ 32            │         │
│  │          1.893 KB    │ 1.893 KB      │ 1.893 KB      │ 1.893 KB      │         │
│  │        dealloc:      │               │               │               │         │
│  │          15          │ 15            │ 15            │ 15            │         │
│  │          1.797 KB    │ 1.797 KB      │ 1.797 KB      │ 1.797 KB      │         │
│  │        grow:         │               │               │               │         │
│  │          7           │ 7             │ 7             │ 7             │         │
│  │          928 B       │ 928 B         │ 928 B         │ 928 B         │         │
│  ╰─ 1     11.27 µs      │ 20.32 µs      │ 11.44 µs      │ 11.83 µs      │ 100     │ 100
│           max alloc:    │               │               │               │         │
│             78          │ 78            │ 78            │ 78            │         │
│             22.26 KB    │ 22.26 KB      │ 22.26 KB      │ 22.26 KB      │         │
│           alloc:        │               │               │               │         │
│             135         │ 135           │ 135           │ 135           │         │
│             20.05 KB    │ 20.05 KB      │ 20.05 KB      │ 20.05 KB      │         │
│           dealloc:      │               │               │               │         │
│             62          │ 62            │ 62            │ 62            │         │
│             22.26 KB    │ 22.26 KB      │ 22.26 KB      │ 22.26 KB      │         │
│           grow:         │               │               │               │         │
│             21          │ 21            │ 21            │ 21            │         │
│             8.736 KB    │ 8.736 KB      │ 8.736 KB      │ 8.736 KB      │         │
├─ naive                  │               │               │               │         │
│  ├─ 0     1.154 µs      │ 4.238 µs      │ 1.237 µs      │ 1.287 µs      │ 100     │ 100
│  │        max alloc:    │               │               │               │         │
│  │          22          │ 22            │ 22            │ 22            │         │
│  │          2.137 KB    │ 2.137 KB      │ 2.137 KB      │ 2.137 KB      │         │
│  │        alloc:        │               │               │               │         │
│  │          22          │ 22            │ 22            │ 22            │         │
│  │          1.209 KB    │ 1.209 KB      │ 1.209 KB      │ 1.209 KB      │         │
│  │        dealloc:      │               │               │               │         │
│  │          5           │ 5             │ 5             │ 5             │         │
│  │          1.113 KB    │ 1.113 KB      │ 1.113 KB      │ 1.113 KB      │         │
│  │        grow:         │               │               │               │         │
│  │          7           │ 7             │ 7             │ 7             │         │
│  │          928 B       │ 928 B         │ 928 B         │ 928 B         │         │
│  ╰─ 1     4.349 µs      │ 11.8 µs       │ 4.433 µs      │ 4.606 µs      │ 100     │ 100
│           max alloc:    │               │               │               │         │
│             93          │ 93            │ 93            │ 93            │         │
│             13.54 KB    │ 13.54 KB      │ 13.54 KB      │ 13.54 KB      │         │
│           alloc:        │               │               │               │         │
│             93          │ 93            │ 93            │ 93            │         │
│             6.216 KB    │ 6.216 KB      │ 6.216 KB      │ 6.216 KB      │         │
│           dealloc:      │               │               │               │         │
│             5           │ 5             │ 5             │ 5             │         │
│             6.616 KB    │ 6.616 KB      │ 6.616 KB      │ 6.616 KB      │         │
│           grow:         │               │               │               │         │
│             17          │ 17            │ 17            │ 17            │         │
│             7.328 KB    │ 7.328 KB      │ 7.328 KB      │ 7.328 KB      │         │
├─ similar                │               │               │               │         │
│  ├─ 0     4.289 µs      │ 50.7 µs       │ 4.706 µs      │ 5.256 µs      │ 100     │ 100
│  │        max alloc:    │               │               │               │         │
│  │          24          │ 24            │ 24            │ 24            │         │
│  │          3.129 KB    │ 3.129 KB      │ 3.129 KB      │ 3.129 KB      │         │
│  │        alloc:        │               │               │               │         │
│  │          47          │ 47            │ 47            │ 47            │         │
│  │          3.947 KB    │ 3.947 KB      │ 3.947 KB      │ 3.947 KB      │         │
│  │        dealloc:      │               │               │               │         │
│  │          30          │ 30            │ 30            │ 30            │         │
│  │          5.017 KB    │ 5.017 KB      │ 5.017 KB      │ 5.017 KB      │         │
│  │        grow:         │               │               │               │         │
│  │          15          │ 15            │ 15            │ 15            │         │
│  │          2.08 KB     │ 2.08 KB       │ 2.08 KB       │ 2.08 KB       │         │
│  ╰─ 1     21.18 µs      │ 65.48 µs      │ 22.73 µs      │ 24.02 µs      │ 100     │ 100
│           max alloc:    │               │               │               │         │
│             79          │ 79            │ 79            │ 79            │         │
│             15.65 KB    │ 15.65 KB      │ 15.65 KB      │ 15.65 KB      │         │
│           alloc:        │               │               │               │         │
│             138         │ 138           │ 138           │ 138           │         │
│             16.51 KB    │ 16.51 KB      │ 16.51 KB      │ 16.51 KB      │         │
│           dealloc:      │               │               │               │         │
│             66          │ 66            │ 66            │ 66            │         │
│             24.39 KB    │ 24.39 KB      │ 24.39 KB      │ 24.39 KB      │         │
│           grow:         │               │               │               │         │
│             43          │ 43            │ 43            │ 43            │         │
│             14.39 KB    │ 14.39 KB      │ 14.39 KB      │ 14.39 KB      │         │
╰─ xdiff                  │               │               │               │         │
   ├─ 0     1.559 µs      │ 6.059 µs      │ 1.642 µs      │ 1.717 µs      │ 100     │ 100
   │        max alloc:    │               │               │               │         │
   │          22          │ 22            │ 22            │ 22            │         │
   │          2.137 KB    │ 2.137 KB      │ 2.137 KB      │ 2.137 KB      │         │
   │        alloc:        │               │               │               │         │
   │          27          │ 27            │ 27            │ 27            │         │
   │          1.773 KB    │ 1.773 KB      │ 1.773 KB      │ 1.773 KB      │         │
   │        dealloc:      │               │               │               │         │
   │          10          │ 10            │ 10            │ 10            │         │
   │          1.677 KB    │ 1.677 KB      │ 1.677 KB      │ 1.677 KB      │         │
   │        grow:         │               │               │               │         │
   │          7           │ 7             │ 7             │ 7             │         │
   │          928 B       │ 928 B         │ 928 B         │ 928 B         │         │
   ╰─ 1     9.316 µs      │ 18.85 µs      │ 9.483 µs      │ 9.655 µs      │ 100     │ 100
            max alloc:    │               │               │               │         │
              78          │ 78            │ 78            │ 78            │         │
              13.14 KB    │ 13.14 KB      │ 13.14 KB      │ 13.14 KB      │         │
            alloc:        │               │               │               │         │
              83          │ 83            │ 83            │ 83            │         │
              8.784 KB    │ 8.784 KB      │ 8.784 KB      │ 8.784 KB      │         │
            dealloc:      │               │               │               │         │
              10          │ 10            │ 10            │ 10            │         │
              9.455 KB    │ 9.455 KB      │ 9.455 KB      │ 9.455 KB      │         │
            grow:         │               │               │               │         │
              20          │ 20            │ 20            │ 20            │         │
              7.2 KB      │ 7.2 KB        │ 7.2 KB        │ 7.2 KB        │         │
```

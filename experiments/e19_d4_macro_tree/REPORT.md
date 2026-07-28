# E19: D4-conditioned row/half-row macro tree

## Decision

**REJECT as a production path; retain the measured tree-search lesson.**
Within the same D4-sector generic sparse tensor representation, a half-row
tree is 20.9–24.1% faster than left-fold row blocks and reduces matching entry
pairs by about 30%. Thus D4 and path search are composable. However, both expose
far more open virtual legs than the production C-derived row operator: the
half-row candidate is 3,919x–127,932x slower than the D4 row baseline and does
not lower peak support. The candidate set therefore selects the existing D4
row baseline.

## Algorithm and fidelity

- Branch/worktree: `codex/exp-d4-macro-tree`,
  `.worktrees/e19-d4-macro-tree`; base `c81d971`.
- Every site is the explicit 17-entry C with v0/v1/v2 absorbed exactly.
- The outer D4 sectors fix the first-row queen to one representative of each
  vertical-reflection orbit. Paired sectors have multiplicity 2; an odd center
  fixed point has multiplicity 1. Every sector and aggregate count are exact.
- `RowBlocks` contracts each row left-to-right. `HalfRowBlocks` first contracts
  its left and right half-row macros, joins the halves, then joins rows.
- Costs are actual sparse nnz, matched pairs, wall time, and RSS; no dense
  treewidth score is used.

All release tests and Clippy pass. New tests verify both D4 macro paths against
known counts through N=5.

## Benchmark

AMD Ryzen 9 7945HX, Windows MSVC, rustc 1.94.0, one thread. Macro candidates
are one run per N; production baseline uses five repetitions/median. RSS is
Windows process `PeakWorkingSetSize`.

| N | row tree (s) | half-row (s) | internal speedup | half-row peak support | production D4 (s) | half-row slowdown |
|---:|---:|---:|---:|---:|---:|---:|
| 5 | 0.025763 | 0.020381 | 1.26x | 7,168 | 0.0000052 | 3,919x |
| 6 | 0.365292 | 0.280978 | 1.30x | 65,536 | 0.0000138 | 20,361x |
| 7 | 7.635539 | 5.795351 | 1.32x | 589,824 | 0.0000453 | 127,932x |

At N=7 matching pairs fall from 17,426,551 to 11,921,243, but peak support is
unchanged. The production row macro's partial evaluation of horizontal signals
is much more important than this association improvement.

Commands:

```text
cargo run --release --bin e19_d4_macro_tree -- row 5 7
cargo run --release --bin e19_d4_macro_tree -- half-row 5 7
cargo run --release --bin e12_d4_orbits -- d4-serial 1 5 7 5
```

Raw data:

- `benchmarks/e19_d4_macro_tree_release.csv`
- `benchmarks/e19_d4_row_baseline_release.csv`

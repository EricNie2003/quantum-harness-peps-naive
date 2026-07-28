# E20: D4 bidirectional separator join

## Decision

**REJECT the row-separator construction and stop at N=7.** The exact top and
bottom subnetworks join correctly, but the boundary conditions make the split
strongly asymmetric. With the first-row D4 queen fixed, top support is tiny;
contracting upward from column v1 and two diagonal v2 ends creates a huge
bottom interface. At N=7, peak live separator support is 13,602 versus 86 for
the one-way D4 baseline, and time is 8.41 s versus 55.7 microseconds.

## Fidelity and algorithm

- Branch/worktree: `codex/exp-bidirectional-separator`,
  `.worktrees/e20-bidirectional-separator`; base `c81d971`.
- Both halves are direct contractions of the explicit 17-entry C tensors with
  all v0/v1/v2 endpoints absorbed.
- The outer first-row D4 sectors use exact vertical-reflection orbit weights.
- The separator key is the complete set of column, down-right and down-left
  virtual bonds crossing the horizontal cut. No placement recurrence or
  incomplete signature is used.
- Each half contracts C-derived full-row macros; the final sparse join matches
  complete separator keys and checked-u128 coefficients.

All release tests and Clippy pass. New tests verify N=0–5 against known counts.

## Benchmark

AMD Ryzen 9 7945HX, Windows MSVC, rustc 1.94.0, one thread. Separator results
are one run per N; D4 baseline is the five-run median. RSS is Windows process
`PeakWorkingSetSize`.

| N | top support | bottom support | live support | D4 support | separator time (s) | D4 time (s) | slowdown |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 4 | 2 | 72 | 74 | 3 | 0.001266 | 0.0000028 | 452x |
| 5 | 3 | 292 | 295 | 8 | 0.024293 | 0.0000050 | 4,859x |
| 6 | 7 | 2,296 | 2,303 | 22 | 0.366319 | 0.0000133 | 27,543x |
| 7 | 13 | 13,589 | 13,602 | 86 | 8.409869 | 0.0000557 | 150,985x |

At N=7 the four sector joins have only 22 matching pairs; join lookup is not
the bottleneck. The bottom halves already generate 54,356 aggregate join keys
and generic intermediate support reaches 589,824. Therefore a better hash join
or contraction-tree search cannot repair this split.

The experiment does not reject every bidirectional idea. A future attempt
would first need an exact quotient/hierarchical basis that compresses the
bottom v2-induced superposition *before* materialization; E17 and E18 show that
plain PLUQ and concrete-state quotient construction do not yet provide it.

Commands:

```text
cargo run --release --bin e20_separator_join -- 4 7
cargo run --release --bin e12_d4_orbits -- d4-serial 1 4 7 5
```

Raw data:

- `benchmarks/e20_separator_join_release.csv`
- `benchmarks/e20_d4_baseline_release.csv`

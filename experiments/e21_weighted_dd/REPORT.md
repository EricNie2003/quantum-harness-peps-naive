# E21: C-derived exact weighted decision diagram

## Decision

**REJECT the fixed interleaved variable order; KEEP the implementation as the
canonical E22 baseline.** The solver is a genuine symbolic PEPS contraction:
it never materializes a sparse boundary. Nevertheless, N=8 and N=9 peak
boundary nodes are 4.53x and 3.65x the D4 concrete support, triggering the
preregistered two-point `nodes/support > 0.8` kill gate. It is also 59–71x
slower than direct D4 contraction.

## Algorithm and fidelity

- Branch/worktree: `codex/exp-weighted-dd`,
  `.worktrees/e21-weighted-dd`; base `e0b011e`.
- The algebraic decision diagram has exact `u128` terminals, a canonical
  `(variable, low, high)` unique table, checked arithmetic, and no probabilistic
  equivalence.
- Each row relation is built by scanning the explicit 17 entries of C. The
  horizontal state starts at v0 and terminates at v1.
- Column outputs stay in the same column; down-right/down-left outputs are
  renamed to the adjacent next-row variables. Edge diagonal outputs are summed
  with v2 and new edge inputs are fixed by v0.
- First-row occupied terms use the E12 vertical-reflection D4 representatives
  and exact orbit multiplicities.
- Relational product performs multiplication and immediately abstracts all
  input virtual variables during recursion. There is no concrete-state oracle
  in the production path.
- Final columns are restricted by v1 and both diagonal families are summed by
  v2.

The fixed order is interleaved by column:
`column_in, dr_in, dl_in, column_out, dr_out, dl_out`.

## Correctness

- New tests match known counts for N=0 through N=6.
- The release binary verifies N=1 through N=9.
- All 27 release tests and Clippy `-D warnings` pass.
- Counts use checked `u128`; overflow is an error.

## Benchmark

AMD Ryzen 9 7945HX, Windows MSVC, rustc 1.94.0, one algorithm thread. One DD
run per N; D4 comparator uses five repetitions and reports median. RSS is
Windows process `PeakWorkingSetSize`, including unique/apply tables and runtime
overhead.

```text
cargo run --release --bin e21_weighted_dd -- 1 9
cargo run --release --bin e12_d4_orbits -- d4-serial 1 7 9 5
```

| N | Q(N) | DD time (s) | D4 time (s) | slowdown | peak boundary nodes | D4 support | nodes/support | peak RSS |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 7 | 40 | 0.003279 | 0.0000475 | 69.0x | 558 | 86 | 6.49x | 6.63 MiB |
| 8 | 92 | 0.006547 | 0.0000917 | 71.4x | 1,232 | 272 | 4.53x | 8.17 MiB |
| 9 | 352 | 0.025330 | 0.0004300 | 58.9x | 4,421 | 1,210 | 3.65x | 15.14 MiB |

N=9 allocates 46,089 nodes over the process lifetime, but only 5,029 are in
the largest live boundary+relation set. Relprod cache has 86,630 lookups and
7,554 hits. The low hit rate and node excess identify variable order, rather
than terminal arithmetic, as the immediate target.

E22 may now legally search variable trees using actual live nodes and cache
misses. It must retain this exact relation and compare against this fixed-order
baseline; a dense width score alone is insufficient.

Raw data:

- `benchmarks/e21_weighted_dd_release.csv`
- `benchmarks/e21_d4_baseline_release.csv`

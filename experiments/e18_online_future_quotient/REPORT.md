# E18: online exact future-equivalence quotient

## Decision

**KEEP as a better exact quotient oracle; REJECT as the production solver.**
The implementation removes E14's forward graph build plus backward transition
replay: it recursively interns complete successor-class/multiplicity signatures
from the initial PEPS boundary and memoizes each concrete state once. Counts
and class numbers are exact, but total time is 2.93–4.44x the same-revision D4
contraction, missing the preregistered 2x gate and worsening with N.

## Algorithm and fidelity

- Branch/worktree: `codex/exp-online-future-quotient`,
  `.worktrees/e18-online-future-quotient`; base `c81d971`.
- Every successor is produced by the compiled operator mechanically derived
  from the explicit 17-entry C and the repository v0/v1/v2 convention.
- The E12 first-row D4 projected slices are retained.
- A state's signature is the complete sorted map from next-row exact class ID
  to multiplicity. No completion-count surrogate or probabilistic hash defines
  equivalence.
- Terminal classes are the exact column-v1 acceptance values 0 and 1.
- `u128` checked arithmetic is used throughout.

All release tests (including new known-count checks through N=9) and Clippy
`-D warnings` pass.

## Benchmark

AMD Ryzen 9 7945HX, Windows MSVC, rustc 1.94.0, one thread. E18 is one run per
N; the D4 comparator uses five repetitions and reports the median. RSS is
Windows process `PeakWorkingSetSize` and includes runtime/hash-table overhead.

```text
cargo run --release --bin e18_online_future_quotient -- 10 13
cargo run --release --bin e12_d4_orbits -- d4-serial 1 10 13 5
```

| N | Q(N) | online time (s) | D4 time (s) | ratio | peak concrete | peak classes | class ratio | RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 10 | 724 | 0.004922 | 0.001682 | 2.93x | 4,510 | 735 | 16.30% | 6.79 |
| 11 | 2,680 | 0.031236 | 0.008901 | 3.51x | 22,253 | 3,462 | 15.56% | 11.64 |
| 12 | 14,200 | 0.157905 | 0.043078 | 3.67x | 98,939 | 14,570 | 14.73% | 33.30 |
| 13 | 73,712 | 1.146952 | 0.258596 | 4.44x | 541,745 | 57,215 | 10.56% | 165.10 |

E18 performs 2,334,171 concrete transitions at N=13, once each. E14 needed a
forward pass and a second backward signature pass. The remaining limitation is
fundamental to this implementation: proving a complete signature still visits
and memoizes every reachable concrete state before it can exploit the much
smaller class DAG. Thus class compression saves replay, not construction.

Raw data:

- `benchmarks/e18_online_future_quotient_release.csv`
- `benchmarks/e18_d4_baseline_release.csv`

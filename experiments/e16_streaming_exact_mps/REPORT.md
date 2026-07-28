# E16: single-prime streaming exact MPS/MPO row apply

## Decision

**REJECT this implementation and stop before N=9.** It establishes that a
from-scratch streaming exact MPS representation is possible and reproduces the
expected exact spatial rank, but its dense finite-field factorization and
adjacent-SWAP wire permutation are much slower than direct sparse contraction.
At N=8 it takes 40.7896195 s versus 0.0000879 s for the same-revision D4 row
baseline (about 464,046x slower), far beyond the preregistered 10x gate.

This rejection is specific to the sitewise dense-MPS implementation. It does
not reject E17's two-block skeleton/PLUQ representation, which is intended to
avoid the expensive qubit SWAP network.

## Hypothesis and revision

- Hypothesis: an exact finite-field MPS boundary can be updated by a
  mechanically C-derived row MPO without materializing the full `8^N`
  coefficient vector, while retaining the low spatial ranks observed in E15.
- Branch: `codex/exp-streaming-exact-mps`
- Worktree: `.worktrees/e16-streaming-exact-mps`
- Candidate code revision: `2dfe39fbfa040027667bbc5d89040a99d4e99d9b`
- Base revision: `c81d971686e5685b2d905254592c3cd275360c6c`
- Arithmetic: the prime field `GF(1,000,000,007)`. This experiment is a
  single-prime structural/performance prototype, not a certified integer/CRT
  production result.

## PEPS contraction design

The production path starts at the rank-one top boundary. For each board row:

1. Construct the row MPO by scanning the explicit 17 entries of
   `SiteTensorC::sec_vi()` at each site. The MPO horizontal state is the Sec. VI
   row virtual signal; its ends apply `v0=(1,0)` and `v1=(0,1)`.
2. Apply the MPO directly to an open-boundary physical-dimension-8 MPS. The
   physical index is exactly `(column, down-right, down-left)` and therefore
   represents the three surviving virtual signals at the row cut.
3. Apply the already-proved vertical-reflection D4 projection to the first-row
   occupied C terms: left representatives have multiplicity two, an odd center
   has multiplicity one, and mirrored-right occupied terms are omitted.
4. Split each dimension-8 physical site into the three binary virtual wires by
   deterministic exact rank factorization over the prime field.
5. Contract exiting diagonal wires with `v2=(1,1)`, insert incoming diagonal
   wires fixed by `v0=(1,0)`, and use exact adjacent SWAP factorizations to
   implement the one-column diagonal shifts. Regroup triples into
   dimension-8 sites.
6. After the last row, contract column bits with `v1` and both diagonal
   families with `v2`.

Every compression is zero-threshold exact column-basis elimination. There is
no floating point, SVD, truncation, rounding, handwritten queen-placement
recurrence, or full sparse-boundary materialization. The diagonal orientation
is the existing repository convention: down-right signals move one column
right and down-left signals move one column left; `v0` and `v2` move with those
orientations.

The representation is dense MPS rather than a sparse map, so
`peak_sparse_support` is recorded as `NA`; the directly relevant size metrics
are the peak exact bond ranks. Local tensor examinations count every explicit C
entry scanned, and accepted entries exclude terms rejected by row endpoints or
the first-row D4 projection.

## Correctness and structural checks

- `cargo test --release`: 28 passed, including all existing 17-entry B/C,
  truth-table, boundary, D4, independent-oracle, and known-count tests.
- The new test reconstructs rectangular matrices from the exact rank
  factorization and checks the streaming contraction through N=5.
- The release benchmark verifies all N=1 through N=8 counts against the
  independent known-count table.
- N=8 reaches shifted bond rank 163. This exactly matches E15's independently
  measured finite-field spatial flattening rank at the corresponding peak cut,
  providing a strong structural cross-check.

The preregistered stronger per-layer coefficient/rank comparison against a
sparse oracle was not completed because the wall-time kill gate failed by more
than four orders of magnitude first. This is another reason the candidate is
not eligible for KEEP.

## Benchmark environment and commands

- Date/time zone: 2026-07-28, Asia/Shanghai.
- CPU: AMD Ryzen 9 7945HX, 16 cores / 32 logical processors.
- OS/toolchain target: Windows, `x86_64-pc-windows-msvc`.
- Compiler: `rustc 1.94.0 (4a4ef493e 2026-03-02)`, LLVM 21.1.8.
- Cargo: 1.94.0.
- Threads: one algorithm thread.
- E16 repetitions: one measured from-scratch contraction per N. N=8 already
  takes about 41 seconds, and the kill gate makes further repetitions
  scientifically unnecessary.
- D4 baseline repetitions: five; table reports median and minimum.

Commands:

```text
cargo fmt --all -- --check
cargo test --release
cargo clippy --release --all-targets -- -D warnings
cargo run --release --bin e16_exact_mps -- 1 8
cargo run --release --bin e12_d4_orbits -- d4-serial 1 6 8 5
```

RSS is Windows `GetProcessMemoryInfo().PeakWorkingSetSize`, sampled after each
completed contraction. It is the process-lifetime peak working set, not
allocator-owned live bytes; sequential N runs can inherit an earlier peak, and
OS paging/cache behavior adds noise. It nevertheless shows that E16 avoids a
large sparse materialization: N=8 peaks at only 24,866,816 bytes.

## Results

| N | Q(N) mod p | verified | E16 time (s) | peak RSS (MiB) | peak MPO rank | peak shifted rank | C entries examined / accepted |
|---:|---:|:---:|---:|---:|---:|---:|---:|
| 1 | 1 | yes | 0.0000236 | 5.13 | 1 | 1 | 17 / 1 |
| 2 | 0 | yes | 0.0000410 | 5.17 | 2 | 1 | 68 / 35 |
| 3 | 0 | yes | 0.0000689 | 5.17 | 2 | 2 | 153 / 104 |
| 4 | 2 | yes | 0.0002024 | 5.18 | 5 | 3 | 272 / 206 |
| 5 | 10 | yes | 0.0012984 | 5.27 | 12 | 8 | 425 / 343 |
| 6 | 4 | yes | 0.0134591 | 5.62 | 26 | 17 | 612 / 513 |
| 7 | 40 | yes | 0.9410345 | 8.61 | 84 | 51 | 833 / 718 |
| 8 | 92 | yes | 40.7896195 | 23.71 | 225 | 163 | 1,088 / 956 |

Same-revision D4 sparse-row baseline:

| N | median time (s) | peak support | peak RSS (MiB) | E16 / baseline |
|---:|---:|---:|---:|---:|
| 6 | 0.0000143 | 22 | 5.16 | 941x |
| 7 | 0.0000450 | 86 | 5.23 | 20,912x |
| 8 | 0.0000879 | 272 | 5.39 | 464,046x |

## Mechanism diagnosis

The low rank is real, but bond dimension alone is not a sufficient cost model.
The implementation stores all local MPS factors densely, repeatedly performs
column-wise exact Gaussian elimination, and realizes the diagonal shift as an
O(N^2) adjacent-SWAP network. Each SWAP contracts two qubit tensors and
refactorizes a new dense matrix. At N=8 the post-row MPO rank reaches 225 even
though the shifted peak is 163, so transient factorization work dominates
while RSS remains modest.

Field-operation and fill-in counters were not added to the timed kernel:
instrumenting every modular add/multiply would materially perturb this already
slow prototype. The dense storage choice means fill-in is paid eagerly rather
than represented sparsely. Wall time, ranks, and low RSS are sufficient to
separate the mechanism: the failure is arithmetic/factorization work, not
full-boundary memory.

E17 should therefore preserve the verified low-rank hypothesis but operate on
the E15 left/right flattening directly, using skeleton/PLUQ updates and bulk
diagonal relabeling rather than site splitting plus adjacent SWAPs.

Raw data:

- `benchmarks/e16_streaming_exact_mps_release.csv`
- `benchmarks/e16_streaming_exact_mps_layers.csv`
- `benchmarks/e16_d4_baseline_release.csv`
- `experiments/e16_streaming_exact_mps/results.csv`

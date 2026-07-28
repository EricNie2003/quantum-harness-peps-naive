# E17: exact two-block sparse PLUQ feasibility probe

## Decision

**REJECT the direct sparse-PLUQ update representation.** Exact rank remains
low, but the pivot factors become dense enough that a skeleton reconstruction
already needs 25,875,207 factor products at N=12 for a boundary with only
98,939 nonzeros (261.5x). A generic row update would have to enumerate those
left/right products before applying the C-derived transfer, triggering E17's
kill gate. This does not disprove all structured low-rank formats; it rejects
plain spatial two-block PLUQ without an additional tensor-product basis.

## Scope and fidelity

- Branch/worktree: `codex/exp-exact-pluq`,
  `.worktrees/e17-exact-pluq`.
- Base: main `c81d971`.
- The probe reuses E15's exact boundary oracle, which contracts the explicit
  17-entry `C`, v0/v1/v2 boundaries, and E12 first-row D4 projection.
- Arithmetic is exact in primes 1,000,000,007 and 1,000,000,009. Ranks agree
  in every case.
- The PLUQ diagnostic constructs normalized sparse pivot rows and the exact
  left coefficient factor. `reconstruction_products` is
  `sum_k nnz(U[:,k]) * nnz(V[k,:])`, the work required even to enumerate the
  factorized matrix before a nonseparable row transfer.
- This is a kill-gate feasibility probe, not a production solver: it uses the
  full sparse boundary only as the allowed validation oracle.

## Benchmark

Command:

```text
cargo run --release --bin e17_pluq_probe -- 8 12
```

One run per N; rustc 1.94.0, Windows MSVC, AMD Ryzen 9 7945HX, one thread.
RSS uses Windows `PeakWorkingSetSize`; it includes process/runtime allocations
and is not allocator-live memory.

| N | support | rank | nnz(U) | nnz(V) | factor products | products/support | time (s) | RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 8 | 272 | 163 | 223 | 211 | 285 | 1.05x | 0.000460 | 5.22 |
| 9 | 1,210 | 396 | 513 | 1,146 | 1,428 | 1.18x | 0.001442 | 5.38 |
| 10 | 4,510 | 1,370 | 3,592 | 3,783 | 9,157 | 2.03x | 0.005016 | 6.30 |
| 11 | 22,253 | 2,484 | 6,670 | 38,768 | 94,766 | 4.26x | 0.023179 | 9.13 |
| 12 | 98,939 | 8,334 | 324,582 | 564,466 | 25,875,207 | 261.53x | 1.347819 | 34.57 |

All 28 release tests and Clippy with `-D warnings` pass. Peak sparse support,
tensor-entry fidelity, counts, and row work remain those of the E15/D4 oracle;
the new metrics isolate factor fill-in.

## Interpretation

E15 measured algebraic rank but not factor sparsity. E17 shows the missing
cost: a low-rank matrix can require extremely dense pivot bases, and the
Sec. VI row transfer is not a single separable left/right map. Plain PLUQ
therefore moves cost from sparse boundary support into dense factor products.

A future low-rank revisit requires a basis that respects column/diagonal
tensor-product structure, not merely a better pivot implementation.

Raw data: `benchmarks/e17_pluq_probe_release.csv`.

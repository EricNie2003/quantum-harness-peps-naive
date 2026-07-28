# E11: exact single-GPU expansion + sort-reduce

## Preregistration

- Branch: `codex/exp-gpu-sort-reduce`
- Worktree: `.worktrees/gpu-sort-reduce`
- Baseline commit: `cccc5211ee15e8bcf20c283142e1597be9776db8`
- Candidate commit: pending implementation and RTX 4060 validation
- User-directed priority change: evaluate GPU throughput immediately after the
  E1--E10 review, before the review's structural-support directions.

## Hypothesis and decision gate

The E10 representation already exposes large contiguous batches of packed
boundary states. A single CUDA device may accelerate the mechanically compiled
row expansion, radix sort, and exact reduction enough to beat the same-laptop
parallel CPU PEPS backend without changing the contracted network.

KEEP only if all tensor/exactness gates pass and, at `N=14`, the RTX 4060
`compact64` end-to-end median is at most 80% of the fastest same-laptop
parallel CPU PEPS median. The DFS result is a comparator, not the keep gate.
Peak device allocation must stay below the configured 85% limit.

## PEPS and exactness obligations

1. Rust constructs the explicit rank-9 17-entry `B` and rank-8 17-entry `C`.
2. The GPU transition descriptor is mechanically compiled from `C` and fails
   closed unless all 16 pass-through entries and the occupied entry match the
   Sec. VI truth table with unit coefficients.
3. CUDA expands that descriptor; it does not implement an independently
   handwritten queen-placement recurrence.
4. `compact64` is exact only for `N <= 20`, uses checked `u64` accumulation,
   and relies on the certified bound `N! <= 20! < 2^64`.
5. `wide128` uses two-limb keys and checked two-limb coefficients and retains
   the CPU representation limit `N <= 42`.
6. The `v0`, row/column `v1`, and diagonal `v2` boundaries are unchanged.
7. No floating point, Tensor Core arithmetic, truncation, SVD, or rounding is
   allowed in the count.

## Planned measurement

- GPU: RTX 4060 Laptop GPU under Ubuntu/WSL2; H200 is a later single-card run.
- Build: Rust release/thin-LTO plus CUDA C++/CUB, exact commands and versions
  recorded by the collector.
- Repetitions: two warmups and nine measured samples per GPU point.
- Sizes: `N=12..14`, both `compact64` and `wide128`.
- CPU comparison: serial, detected physical-core, and logical-thread PEPS;
  select the fastest parallel median only after recording every raw result.
- Required fields: count/verification, host wall and CUDA-event time, phase
  times, peak device bytes, peak support, compiled tensor examinations,
  row-operator candidates, and accepted candidates.
- Raw result target: `benchmarks/e11_gpu_sort_reduce_rtx4060.csv`.

This direction changes throughput and memory placement only. Even if kept, it
does not reduce the measured 5--6x-per-step support growth and does not by
itself establish a route to `Q(28)`.

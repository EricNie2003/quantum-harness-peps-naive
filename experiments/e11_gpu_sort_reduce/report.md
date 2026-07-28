# E11: exact single-GPU expansion + sort-reduce

## Preregistration

- Branch: `codex/exp-gpu-sort-reduce`
- Worktree: `.worktrees/gpu-sort-reduce`
- Baseline commit: `cccc5211ee15e8bcf20c283142e1597be9776db8`
- Candidate implementation commit: `4a2165c28d1ae5881946ade6b5bac1bddcdb4d85`
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
  times, Linux `VmHWM` peak host RSS, tracked peak device bytes, peak support,
  compiled tensor examinations, row-operator candidates, and accepted
  candidates.
- Raw result target: `benchmarks/e11_gpu_sort_reduce_rtx4060.csv`.

This direction changes throughput and memory placement only. Even if kept, it
does not reduce the measured 5--6x-per-step support growth and does not by
itself establish a route to `Q(28)`.

## Implemented contraction

The optional Cargo feature `cuda` compiles one CUDA C++ translation unit with
CUDA Runtime and CUB. Rust first constructs the explicit `B` and `C`, invokes
the existing fail-closed `CompiledRowOperator::compile(&C)`, and passes the
resulting occupied transition plus the 17/17 construction counters across a
size-checked C ABI. CUDA independently rejects any descriptor other than the
unit Sec. VI `0 -> 1` transition on all four channels.

For each row, the device counts accepted tensor-derived transitions, performs
an exclusive scan, expands candidates, radix-sorts packed open-virtual-boundary
keys, run-length encodes equal keys, and reduces each run with checked integer
addition. `compact64` uses one 64-bit key and coefficient. `wide128` stores
both as two explicit 64-bit limbs and obtains lexicographic 128-bit ordering
with stable low-word then high-word radix passes. Neither backend uses floating
point arithmetic for keys or coefficients. CUDA events use floating point only
inside the runtime's elapsed-time reporting API and never affect the count.

The initial all-zero boundary applies `v0`. Each row accepts exactly the unique
occupied branch emitted by the compiled row MPO, which applies the row `v1`.
The diagonal shifts discard signals leaving either edge, contracting those
endpoints with `v2=(1,1)`. The final device sum filters for an all-one column
boundary, applying the column `v1`; remaining diagonal bits are unrestricted.

## Local implementation validation

Validation host: Intel i5-2450M, 2 physical cores / 4 logical threads,
x86_64 Linux; `rustc 1.97.1 (8bab26f4f 2026-07-14)`, Cargo 1.97.1. This host
has no `nvcc` and no CUDA device, so it cannot establish native CUDA linking,
runtime correctness, RTX performance, or the keep gate.

Commands completed successfully at the candidate commit:

```text
cargo fmt --all -- --check
NQUEENS_CUDA_SKIP_NATIVE=1 cargo check --features cuda --all-targets
cargo test --release
cargo clippy --all-targets --release -- -D warnings
git diff --check
bash -n scripts/benchmark_gpu_wsl.sh
cargo run --release -- solve 8 --layers
```

The release suite passed all 17 CPU/tensor tests. The N=8 smoke solve returned
`Q(8)=92`, `verified=true`, peak support 538, and Linux `VmHWM` 2,224,128
bytes. In addition, Clang 18 parsed the CUDA translation unit in both host-only
mode and device-only `sm_89` mode using temporary API stubs; this catches CUDA
C++ syntax problems but is explicitly not an `nvcc` build or device test.

The RTX 4060 WSL host subsequently passed the same 17-test CPU suite with Rust
1.97.1. Its first NVHPC `nvcc` build rejected device uses of the host standard
library's `std::numeric_limits<uint64_t>::max()`. Candidate commit `4a2165c`
replaced those constants with the exactly equivalent all-one `uint64_t`
expression and passed the local host/device syntax and CPU gates. Native build
and runtime validation remain pending after that compatibility fix.

The device self-test, once run, checks stable two-word sort ordering, wide-key
equality/run lengths, compact overflow detection, two-limb carry, and two-limb
overflow detection. The feature-gated integration test then compares both GPU
schemes against every CPU layer and known counts through N=10.

## Benchmark and decision status

**PENDING GPU VALIDATION — no KEEP/REJECT decision.** The experiment is not
complete until the RTX 4060 runs the native build, device self-test, integration
tests, and preregistered benchmark. The collector is
`scripts/benchmark_gpu_wsl.sh`; it records environment/build details and writes
raw files under `benchmarks/e11_gpu_sort_reduce_rtx4060_raw/` plus the combined
GPU CSV at `benchmarks/e11_gpu_sort_reduce_rtx4060.csv`. No benchmark values
from another algorithm or machine have been copied into this report.

Host RSS is Linux process-lifetime `VmHWM`; repeated samples therefore share a
cumulative high-water mark. Device memory is the peak of allocations owned and
tracked by this backend. It excludes CUDA driver/context allocations and other
processes, so the two measurements must remain separate in the final decision.

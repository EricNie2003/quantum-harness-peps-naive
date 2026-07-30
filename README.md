# Exact naive contraction of the N-Queens PEPS

This Rust crate explicitly implements the local tensors in Sec. VI of
Liu--Liao--Wang, *Statistical mechanics of the N-queens problem*.

- `B` is rank 9: eight binary virtual legs and one physical occupation leg.
- `B` has exactly 17 non-zero entries.
- `C = sum_alpha B` is rank 8 and also has exactly 17 non-zero entries.
- The solver applies sparse entries of `C` site by site, contracts each row,
  and uses the `v0`, `v1`, and `v2` boundary conditions from the paper.
- Coefficients use checked `u128` integer arithmetic. There is no floating
  point arithmetic, SVD, truncation, or DFS in the solver.
- The default solver contracts exact first-row orbits of the cut-preserving
  vertical-reflection subgroup. It handles odd-N fixed points explicitly and
  never applies a blanket D4 multiplicity.

## Reports

The current Chinese submission is [`第四稿.html`](第四稿.html). The report and its
figures are frozen at commit
`be679e46c19cf12ac4c3922d610749cad5e16023`. It separates local measurements,
the same-node SCNet comparison, the independent Q(22) production run, and the
Q(28) resource projection.

The detailed evidence reports remain available at
[`docs/issue34_final_submission_zh.html`](docs/issue34_final_submission_zh.html)
and [`docs/issue34_research_report.html`](docs/issue34_research_report.html).
The tensor-to-automaton proof, reviewer response, and publication revision plan
are in [`docs/`](docs/).

## Reproducing the reported results

This section distinguishes three different tasks:

1. **Correctness reproduction** checks the explicit Sec. VI tensors and small-N
   counts on any recent machine.
2. **Artifact reproduction** rebuilds the publication CSV files from the raw
   immutable job outputs without using SCNet.
3. **Timing reproduction** reruns the frozen executables under the recorded
   SCNet hardware, thread, and repetition protocol.

The root executable is the readable explicit-`C` reference implementation. The
large-N curves were produced by frozen historical revisions; benchmarking the
current `main` executable and labelling those timings as E50 data would not be
a valid reproduction.

### 1. Software versions

The recorded exact Rust benchmarks used:

- `rustc 1.97.1 (8bab26f4f 2026-07-14)` with LLVM `22.1.6`;
- target `x86_64-unknown-linux-musl`;
- Cargo release profile `codegen-units=1`, `lto=thin`;
- GNU `time -v` for process-level peak RSS on Linux.

TreeSA planning and the truncated boundary-MPS diagnostic used Julia `1.11.5`.
The submission-figure audit used Python `3.12.3`, NumPy `2.4.4`, Matplotlib
`3.10.8`, and the Noto CJK font collection. The Rust crate has only the locked
`rayon` dependency; the APX1 Julia project uses only standard-library
dependencies.

For the exact benchmark compiler and target:

```bash
rustup toolchain install 1.97.1
rustup target add --toolchain 1.97.1 x86_64-unknown-linux-musl
```

A musl C toolchain is also required when building the static SCNet binaries.
Native host builds are sufficient for correctness checks, but their timings
must not be compared directly with the recorded musl/SCNet data.

### 2. Fast correctness reproduction

From the submission snapshot:

```bash
git clone https://github.com/EricNie2003/quantum-harness-peps-naive.git
cd quantum-harness-peps-naive
git checkout be679e46c19cf12ac4c3922d610749cad5e16023
cargo +1.97.1 test --release
cargo +1.97.1 run --release -- solve 8 --layers
cargo +1.97.1 run --release -- bench 11 --min 4 --repeats 3 --csv
```

The recorded snapshot reports `51 passed` in the library test suite, and the
`N=8` count must be `92`. The release tests check the 17 non-zero
entries of rank-9 `B`, the 17 non-zero entries of rank-8 `C`, the empty and
occupied local truth tables, `v0`/`v1`/`v2` boundaries, optimized-versus-
explicit-`C` replay, small-N independent oracles, and known values of `Q(N)`.

The independent conventional DFS comparator can be checked separately:

```bash
cargo +1.97.1 run --release --bin dfs_bitmask -- solve 16 --threads 1
cargo +1.97.1 run --release --bin dfs_bitmask -- \
  bench 17 --min 8 --threads 16 --repeats 9 --warmup 2 --csv
```

DFS is an oracle and performance comparator only; it is not the PEPS solver.
The root crate reports in-process peak RSS through `GetProcessMemoryInfo` on
Windows. Its in-process Linux RSS field is zero, so Linux publication RSS must
come from `/usr/bin/time -v`, as described below.

### 3. Frozen algorithm revisions

| Reported family | Role | Frozen source | Executable / planner | Arithmetic |
|---|---|---|---|---|
| Naive PEPS | explicit-`C` HashMap row contraction | `20b5334f55819ab0b4bdce7aa701527de736c3dc` | `nqueens-peps-naive` | checked `u128` |
| Latest PEPS, no TreeSA | proved-equivalent explicit-`C`, certified last-six | `fc0921b00f1b700b3f6a3930a43cb48806afd3b8` (algorithm `ea5b985753593bf98e9c4684890bd0f19caf1bd9`) | `e50_crt_avx2` | exact three-prime CRT |
| TreeSA PEPS | TreeSA site-tree plan plus exact executor | `c715e36e835a1890055c09fad34ca7c2e854bf0d` (implementation `e9a80a5311fbb3d99c32b9665caaed7fdcd3a959`) | `generate_plan.jl` + `e37_treesa_d4` | exact integer contraction |
| DFS | independent non-tensor comparator | `b89f4f1320bdbe9e0fcce700b9d98b879f012bea` | `dfs_bitmask` | checked integer search |
| Q(22) production | explicit-`C` certified last-six with metrics replay | `258a6cadda71619febaa7d9be176869fd3d045cf` (algorithm `ea5b985753593bf98e9c4684890bd0f19caf1bd9`) | `e50_profile_once` | exact three-prime CRT |
| APX1 | diagnostic truncated boundary MPS, not an exact result | `b89f4f1320bdbe9e0fcce700b9d98b879f012bea` | Julia `TruncatedBoundaryMPS` | `Float64` QR/SVD |

Use detached worktrees so that historical implementations are never mixed:

```bash
git worktree add --detach ../issue34-repro-naive 20b5334f55819ab0b4bdce7aa701527de736c3dc
git worktree add --detach ../issue34-repro-e50 fc0921b00f1b700b3f6a3930a43cb48806afd3b8
git worktree add --detach ../issue34-repro-treesa c715e36e835a1890055c09fad34ca7c2e854bf0d
git worktree add --detach ../issue34-repro-dfs b89f4f1320bdbe9e0fcce700b9d98b879f012bea
git worktree add --detach ../issue34-repro-q22 258a6cadda71619febaa7d9be176869fd3d045cf
```

At each Rust revision, run `cargo +1.97.1 test --release --lib` before building
the named release binary. The exact cluster build shapes are:

```bash
cargo +1.97.1 build --release --target x86_64-unknown-linux-musl --bin nqueens-peps-naive
cargo +1.97.1 build --release --target x86_64-unknown-linux-musl --bin e50_crt_avx2
cargo +1.97.1 build --release --target x86_64-unknown-linux-musl --bin e37_treesa_d4
cargo +1.97.1 build --release --target x86_64-unknown-linux-musl --bin dfs_bitmask
cargo +1.97.1 build --release --target x86_64-unknown-linux-musl --bin e50_profile_once
```

Each command must be run in its corresponding worktree, not all in one source
directory. Raw metadata records the executable SHA-256 used by each job.
Before running the TreeSA planner, instantiate its frozen Julia environment:

```bash
cd ../issue34-repro-treesa
julia --startup-file=no --project=experiments/e37_treesa_d4 \
  -e 'using Pkg; Pkg.instantiate()'
```

### 4. Rebuild the same-node comparison from raw files

The comparison archive is
[`experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/`](experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/).
It contains every aggregate and per-N CSV, GNU-time record, metadata file,
binary hash, Slurm log, TreeSA plan, and the exact submitted batch scripts.

The following command was checked to reproduce the committed normalized CSV
byte for byte:

```bash
mkdir -p /tmp/issue34-mpl
MPLCONFIGDIR=/tmp/issue34-mpl python3 \
  experiments/e51_truncated_boundary_mps/scripts/analyze_algorithm_comparison.py \
  --dfs experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/dfs/results \
  --naive experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/naive/results \
  --peps experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/latest_peps/results \
  --treesa experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/treesa/results \
  --output-csv /tmp/issue34-comparison-rebuilt.csv \
  --output-time-svg /tmp/issue34-time-rebuilt.svg \
  --output-rss-svg /tmp/issue34-rss-rebuilt.svg
cmp /tmp/issue34-comparison-rebuilt.csv \
  benchmarks/issue34_same_node_algorithm_comparison_scnet.csv
```

`cmp` must exit with status zero. The analyzer independently verifies known
counts, frozen revisions, node identity, requested threads, exit status, and
duplicate/missing points before deriving ratios.

The normalized table retains `N`, exact count, verification status, internal
and process wall times, peak RSS, peak sparse support, local tensor-entry
examinations/acceptances, thread count, repetition policy, revision, node,
command, compiler, raw path, and memory method. `NA` means a metric does not
exist for that algorithm class; it is not silently replaced by another
algorithm's measurement.

The submission snapshot contains 68 exact rows plus the header:

- DFS: `N=1..21`;
- latest exact PEPS without TreeSA: `N=1..21`;
- naive PEPS: `N=1..16`;
- TreeSA PEPS: `N=2..11`.

The same-node DFS/latest-PEPS `N=22` pair is not present in the publication
CSV and must not be inferred from the separate Q(22) production job. Therefore
do not add `--require-complete` when reproducing this report snapshot.

### 5. Rerun the same-node SCNet timing protocol

All comparison jobs were serialized on exclusive node `b10r4n19`: two AMD
EPYC 7742 sockets, 128 physical cores, one hardware thread per core. Every N
ran in a fresh process. The protocol was:

| Family | N range | CPU allocation | Samples / warmups |
|---|---:|---:|---:|
| TreeSA plan + exact executor | 2--11 | 1 core | one deterministic plan and one execution; seed `20260729`, 10 trials, 50 iterations |
| Naive PEPS | 1--14 | 1 core | 3 / no separate warmup |
| Naive PEPS | 15--16 | 1 core | 1 / no separate warmup |
| DFS comparator | 1--19, 20, 21 | 128 cores | 7/2, 3/1, 1/0 |
| Latest PEPS | 1--19, 20, 21 | 128 cores | 7/2, 3/1, 1/0 |

The corresponding command shapes were:

```text
nqueens-peps-naive bench N --min N --repeats R --csv
dfs_bitmask bench N --min N --threads 128 --repeats R --warmup W --csv
e50_crt_avx2 scalar 3 N N R W 512 0
generate_plan.jl PLAN N 20260729 10 50
e37_treesa_d4 PLAN d4
```

The exact `sbatch` submission lines, including memory/time requests and the
serialized `afterany` chain, are preserved in the
[`raw comparison README`](experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/README.md).
The drivers expect the corresponding frozen binary under `./bin/`. SCNet users
must update only site-specific `#SBATCH --account`, `--qos`, and partition
values; changing the node type, exclusivity, CPU count, binding, command,
revision, or repetition policy creates a new benchmark rather than a
reproduction.

Wall times can vary with system state. Exact counts, verification flags, and
work counters must agree; performance should be reported as a new measurement
if the hardware, compiler, thread count, or binary hash differs.

### 6. Reproduce the Q(22) production result

Q(22) is a separate production run on exclusive node `b11r2n02`, with the same
dual-EPYC-7742/128-core topology but not the same physical node as the four-way
comparison. Build `e50_profile_once` at revision `258a6cad...`, then run inside
a 128-core allocation:

```bash
export RAYON_NUM_THREADS=128
/usr/bin/time -v ./e50_profile_once 22 512
```

The expected values in the CSV row are:

```text
Q(22) = 2691008701644
count-only wall = 10112.287307712 s
```

This was one exact three-prime CRT count followed by one instrumented
explicit-`C` metrics replay in the same process. The whole process took
`8:21:00`; GNU `time -v` reported `57892 KiB` maximum RSS. The count-only time
excludes the replay, while the RSS covers both phases. Full command, compiler,
CPU, thread, hash, CSV, stderr, and GNU-time provenance is in
[`benchmarks/raw/scnet_e50_n22/`](benchmarks/raw/scnet_e50_n22/).

### 7. Reproduce the APX1 truncation diagnostic

APX1 is deliberately approximate and must never be rounded or reported as an
exact N-Queens result. At revision `b89f4f1...`:

```bash
JULIA_NUM_THREADS=1 julia --startup-file=no \
  --project=experiments/e51_truncated_boundary_mps \
  experiments/e51_truncated_boundary_mps/test/runtests.jl

JULIA_NUM_THREADS=1 OPENBLAS_NUM_THREADS=128 julia --startup-file=no \
  --project=experiments/e51_truncated_boundary_mps \
  experiments/e51_truncated_boundary_mps/scripts/benchmark_point.jl \
  14 128 1 1 0
```

The Julia test command must pass 616 assertions: 72 explicit-`B`/`C` checks,
504 boundary truth-table checks, 32 uncapped geometry/oracle checks, seven
finite-cap checks, and one BLAS-thread-invariance check.

The reported SCNet sweep used exclusive node `b10r4n17`, Julia one thread,
OpenBLAS 128 threads, and fresh Julia processes. At N=14, chi=4,8,16,32,64
used three samples after one warmup; chi=128 used one sample after one warmup.
The exact submission driver is
[`scnet_e51_truncation.sbatch`](experiments/e51_truncated_boundary_mps/scripts/scnet_e51_truncation.sbatch).

Rebuild both committed APX1 CSVs from the raw jobs with:

```bash
MPLCONFIGDIR=/tmp/issue34-mpl python3 \
  experiments/e51_truncated_boundary_mps/scripts/analyze_tradeoff.py \
  experiments/e51_truncated_boundary_mps/raw/final_b89f4f1/results/*.csv \
  --output-raw-csv /tmp/issue34-truncation-raw-rebuilt.csv \
  --output-csv /tmp/issue34-truncation-tradeoff-rebuilt.csv \
  --output-svg /tmp/issue34-truncation-rebuilt.svg \
  --output-scaling-svg /tmp/issue34-truncation-scaling-rebuilt.svg \
  --plot-n 8,14
cmp /tmp/issue34-truncation-raw-rebuilt.csv \
  benchmarks/e51_truncated_boundary_mps_scnet_release.csv
cmp /tmp/issue34-truncation-tradeoff-rebuilt.csv \
  benchmarks/e51_truncated_boundary_mps_tradeoff.csv
```

Both `cmp` commands must exit with status zero. The key N=14, chi=128 result is
an estimate of `13.4035591212` versus exact `365596`, relative error
`0.9999633378`, and median time `198.414 s`.

### 8. Timing and memory definitions

- `execution_time_s` is the algorithm's measured kernel statistic. Warmup,
  startup, planning, or metrics replay is included only when the corresponding
  column says so.
- `execution_process_wall_s` is fresh-process wall time from the archived
  per-point run; it is not interchangeable with the internal median.
- Linux peak RSS is the per-process `Maximum resident set size` from GNU
  `time -v`. It includes runtime and allocator retention, but excludes Slurm
  controller/cgroup overhead and other processes on the node.
- TreeSA planner and exact executor ran as separate processes. End-to-end RSS
  is the maximum of their two peaks, not their sum.
- APX1 RSS includes Julia, OpenBLAS, LAPACK workspaces, and allocator retention.
- Peak sparse support and local `C` entries examined/accepted are algorithmic
  counters and are reported separately from process RSS.

### 9. Machine-readable evidence index

| Evidence | Path |
|---|---|
| Same-node four-family normalized table | [`benchmarks/issue34_same_node_algorithm_comparison_scnet.csv`](benchmarks/issue34_same_node_algorithm_comparison_scnet.csv) |
| Same-node raw archive and exact Slurm commands | [`experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/`](experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/) |
| E50 N=1..19 scaling | [`benchmarks/scnet_e50_scaling_n1_n19_release.csv`](benchmarks/scnet_e50_scaling_n1_n19_release.csv) |
| E50 calibration points | [`benchmarks/scnet_e50_calibration_release.csv`](benchmarks/scnet_e50_calibration_release.csv) |
| Q(22) raw production bundle | [`benchmarks/raw/scnet_e50_n22/`](benchmarks/raw/scnet_e50_n22/) |
| APX1 normalized measurements | [`benchmarks/e51_truncated_boundary_mps_scnet_release.csv`](benchmarks/e51_truncated_boundary_mps_scnet_release.csv) |
| APX1 accuracy/cost table | [`benchmarks/e51_truncated_boundary_mps_tradeoff.csv`](benchmarks/e51_truncated_boundary_mps_tradeoff.csv) |
| Q(28) sensitivity projection, not an exact count | [`benchmarks/issue34_submission_q28_projection.csv`](benchmarks/issue34_submission_q28_projection.csv) |
| Submission figure data audit/generator | [`scripts/generate_issue34_submission_figures.py`](scripts/generate_issue34_submission_figures.py) |

Q(27) and Q(28) were not computed. The Q(28) CSV and figure are resource
sensitivity projections anchored to exact measurements, not counts, confidence
intervals, or complexity theorems.

See [`REPORT.md`](REPORT.md) for the tensor construction, boundary convention,
and baseline benchmark discussion. Each optimization has an isolated report in
[`experiments/`](experiments/), including its hypothesis, revision, commands,
correctness gate, measurements, and keep/reject decision.

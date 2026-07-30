# APX1: truncated finite-PEPS boundary-MPS diagnostic

## Preregistration and classification

- Branch: `codex/exp-truncated-boundary-mps`.
- Worktree: `.worktrees/e51-truncated-boundary-mps`.
- Baseline: `bd928cd`.
- Mandatory E46--E50 review commit: `8759325`.
- Decision vocabulary: `DIAGNOSTIC_ONLY` or `REJECT`; never exact production.

This branch forked before the coworker's later `origin/main` assigned E51--E60
to a different sequence of exact CPU/cache experiments. The canonical ID of
this approximation study is therefore **APX1**. Legacy directory, script, job,
CSV, and figure names retain `e51` only to preserve raw provenance and hashes.
APX1 is not a numbered exact-PEPS optimization and does not alter the mandatory
five-direction review counter.

This experiment answers a separate scientific question from the exact Issue
#34 acceptance target: if a conventional finite-PEPS boundary contraction is
compressed to maximum MPS bond dimension `chi`, how rapidly does its estimate
converge, and what wall-time/RSS reduction is obtained? Every capped result is
floating-point and approximate. Agreement after rounding is not exactness and
will not be reported as an exact N-queens count.

## Network and contraction

The code constructs the rank-9 `B` explicitly from Eq. (16): all sixteen
independent alpha=0 four-channel pass-through entries plus the unique alpha=1
four-channel 0-to-1 entry. It forms rank-8 `C=sum_alpha B` explicitly; both
tensors have 17 stored nonzeros.

For each board row, `C.entries()` generates a bond-dimension-two row MPO. Its
horizontal left endpoint is `v0=(1,0)` and right endpoint is `v1=(0,1)`. The
boundary MPS has physical dimension eight per column, exactly the open
`(column, down-right, down-left)` virtual bits. After applying a row MPO, the
code splits those bits into labeled qubit sites, contracts the two diagonal
signals that leave the board with `v2=(1,1)`, inserts new `v0` diagonal signals,
and realizes the one-column diagonal translation with adjacent MPS SWAPs. It
then regroups the three bits. At the bottom, every column is contracted with
`v1` and both diagonal families with `v2`.

The diagonal convention matches the exact row-major implementation:
down-right output at column `c` enters column `c+1` on the next row, and
down-left output enters `c-1`; each orientation moves its `v0` and `v2`
endpoints together.

Compression first left-canonicalizes the MPS by QR, then sweeps right-to-left
with LAPACK SVD. Each diagonal-wire SWAP first places the mixed-canonical
orthogonality center on that bond, so its two-site SVD truncates actual Schmidt
directions rather than a gauge-dependent local factor. Splitting one
physical-dimension-eight site into its three labeled qubits is exact (up to
floating numerical-rank removal); `chi` is applied only at canonical MPS cuts.
`chi=0` means no user cap and exists only for small-N geometry checks.
Reported discarded Frobenius fractions are local diagnostics, not a rigorous
global error bound. `peak_working_bond` includes the temporary exact site split,
while `peak_retained_bond` records capped canonical checkpoints.

## Preregistered validation and sweep

1. Test the 17-entry B/C construction, empty/occupied local truth tables, and
   all four channel families under `v0...v1` and `v0...v2`.
2. Require uncapped floating contraction for N=0--7 to match known exact values
   with `rtol=5e-10`, `atol=5e-9`, with no user-cap truncation.
3. Calibrate locally before SCNet submission.
4. Sweep `chi=4,8,16,32,64,128`; use a smaller and a larger N range only after
   measured calibration establishes safe wall/RSS bounds.
5. Record the unrounded estimate and exact reference separately, absolute and
   relative error, median/min/max wall, per-point RSS, retained/pre-truncation
   bonds, SVD and truncation counts, local discarded-weight diagnostics, dense
   MPS elements, and explicit-C work.

The SCNet driver launches each `(N,chi)` point in a fresh Julia process within
one allocation. Internal timings exclude Julia startup; GNU `time -v` captures
per-point process peak RSS including the runtime and LAPACK workspace. Slurm
MaxRSS is an allocation-level cross-check and cannot attribute memory to an
individual point. The dense boundary MPS has no sparse support, so the required
`peak_sparse_support` field is explicitly `NA`; `peak_dense_mps_elements` and
bond dimensions are the applicable representation-size metrics.

## Results

### Rejected non-canonical pilot

Revision `41133e1` applied the bond cap while splitting local physical sites
and during adjacent SWAP factorizations without first placing the global MPS
orthogonality center on the affected bond. Its uncapped N=0--7 tests all
passed, but its capped estimates were gauge dependent. On the local Ryzen,
N=8 at chi=4/8/16 produced approximately 0.342/0.996/1.182; on SCNet EPYC with
the same source they were approximately 0.0000099/-0.00882/0.00261. The exact
reference is 92. These are not ordinary timing fluctuations: truncating a
non-canonical local factor is not a Schmidt truncation, so different valid SVD
gauges can retain different global subspaces.

SCNet job `41521702` completed the N=5--7 pilot and job `41521877` was cancelled
after 1m59s when this flaw was identified. Those rows are retained as rejected
diagnostic evidence and are not used in the final error--speed curves.

The corrected implementation makes the physical-dimension-eight to three-
qubit split exactly, canonicalizes both environments around every SWAP bond,
and only then applies the chi-capped two-site SVD. Local results are invariant
between one and eight OpenBLAS threads for the tested capped points, while the
uncapped N=0--7 validation remains within the preregistered tolerance.

Final canonical SCNet measurement is complete; the validated results and
decision are reported below.

### LAPACK robustness correction

On canonical revision `48be9f2`, SCNet job `41522520` completed N=8 through
chi=64, then LAPACK divide-and-conquer SVD (`gesdd`) returned `info=1` at
chi=128 during an exact site split. The failed process used only about 437 MiB,
so this was numerical non-convergence rather than memory exhaustion. The code
now rejects non-finite inputs explicitly and retries only `LAPACKException`
with LAPACK QR-iteration SVD (`gesvd`). `svd_qr_fallbacks` is reported for every
point; other exceptions still fail closed. The successful chi<=64 rows remain
valid because each point ran in a separate process.

## Validation status

The final benchmarked contraction source is revision `b89f4f1`; every final
job records the same source, driver, and project SHA-256 values. SCNet job
`41523448` passed 608 assertions before measuring: 72 explicit-B/C checks,
504 line-boundary truth-table checks, 24 uncapped N=0--7 geometry checks,
seven finite-cap checks, and one BLAS-thread invariance check. The uncapped
floating estimates at N=5--8 differ from the exact integers by only
`1.2e-15` to `8.7e-14` relatively and report zero cap-induced truncations.

The final local test harness also compares N=0--7 with a conventional
backtracking implementation that is confined to `test/runtests.jl` and never
called by the PEPS code. That independently validates both the known-count
table and the uncapped contraction. The resulting 616-assertion suite passes.
This post-benchmark addition changes only the test harness; the benchmarked
`src/TruncatedBoundaryMPS.jl` SHA-256 remains
`6bdc9c4098dc44b4cbf59c0daf7cb0fbb46c522e8c3de7e0adf44e53a2dfe929`.

## Benchmark protocol

- Official Julia 1.11.5 release on SCNet node `b10r4n17`, two AMD EPYC 7742
  sockets, 128 physical cores, one hardware thread per core.
- `JULIA_NUM_THREADS=1`, `OPENBLAS_NUM_THREADS=128`, one Slurm task bound to
  128 cores. Every final point used this same node and thread configuration.
- Each ordinary point ran in a fresh Julia process, performed one unmeasured
  warmup contraction, then three measured contractions. The CSV reports the
  median/min/max internal kernel time, excluding Julia startup and JIT.
  Expensive calibration points explicitly record their smaller repetition
  count rather than being mixed with the three-repeat data.
- `/proc/self/status` `VmHWM` is sampled in-process and GNU `time -v` records
  per-process maximum RSS, including Julia, OpenBLAS, and LAPACK workspaces.
  Slurm step `MaxRSS` is retained as a cross-check. These high-water marks do
  not isolate tensor storage, allocator retention, or NUMA placement, so the
  representation-level `peak_dense_mps_elements` is reported separately.
- The final allocations requested either 8 GiB or 32 GiB but all remained on
  the same node and well below either limit. The original driver printed a
  stale literal `slurm_memory_request=32G` in metadata even for 8-GiB command-
  line overrides; scheduler accounting is authoritative. The driver now
  records `SLURM_MEM_PER_NODE` directly.
- The dense boundary representation has no sparse-support count;
  `peak_sparse_support=NA` is intentional. Every row still records exactly
  `17*N^2` local-C entry examinations and the accepted-entry count.

The benchmark command shape was:

```text
sbatch --nodelist=b10r4n17 --mem=<8G-or-32G> \
  --export=ALL,SOURCE_REVISION=b89f4f1,E51_RUN_TESTS=<0-or-1> \
  scripts/scnet_e51_truncation.sbatch MIN_N MAX_N CHI_CSV REPEATS WARMUP
```

Raw job CSV, per-point CSV and GNU-time files, logs, hashes, metadata, and
scheduler records are under `raw/final_b89f4f1/`. The merged unmodified rows
are in `benchmarks/e51_truncated_boundary_mps_scnet_release.csv`; the derived
speedup table and two SVG plots are generated by `scripts/analyze_tradeoff.py`.

```text
python3 experiments/e51_truncated_boundary_mps/scripts/analyze_tradeoff.py \
  experiments/e51_truncated_boundary_mps/raw/final_b89f4f1/results/*.csv \
  --output-raw-csv benchmarks/e51_truncated_boundary_mps_scnet_release.csv \
  --output-csv benchmarks/e51_truncated_boundary_mps_tradeoff.csv \
  --output-svg experiments/e51_truncated_boundary_mps/figures/e51_chi_tradeoff.svg \
  --output-scaling-svg experiments/e51_truncated_boundary_mps/figures/e51_fixed_chi_scaling.svg \
  --plot-n 8,14
```

The normalizer independently audits every exact reference count, recomputes the
absolute and relative errors, checks finite timings and RSS, validates tensor
work-counter inequalities and status/truncation consistency, and rejects
duplicate N/chi points before writing either plot.

Final APX1 output SHA-256 values are:

```text
8c729334ab71702551b6fdfe46bd015cfba881493f7f083e8e24edf50706b023  benchmarks/e51_truncated_boundary_mps_scnet_release.csv
96457d42b48525e617f036acad1724a06e5d923238f2d2a87804da677d06dfd1  benchmarks/e51_truncated_boundary_mps_tradeoff.csv
68fad24b62e03209e40ca2dde62229d2f673ad65225904636703d6fb70ea2bb4  figures/e51_chi_tradeoff.svg
e4c66005d818f39a45547f76dcfb1e8ba337c43c4f9e7a777017af89c366b4d3  figures/e51_fixed_chi_scaling.svg
```

## Cost model and scope of the implementation

For N>0 the current geometry performs exactly `(N-1)*(11*N-9)` SVD calls:
row-MPO compression, exact dimension-8-to-qubit splitting, `7*N-8` labeled
adjacent SWAPs on every nonfinal row, and regrouping compression. This gives
553 calls at N=8 and 1,885 at N=14, matching the recorded counters. With a
fixed cap and fixed physical dimensions, dense MPS storage is `O(N*chi^2)`
and the local SVD work is nominally `O(N^2*chi^3)`.

The correctness-first SWAP implementation reconstructs a mixed-canonical
center from both ends before each two-site truncation. That makes every cut a
real Schmidt truncation but adds redundant QR sweeps, giving this baseline an
`O(N^3*chi^3)` upper-bound component. A library implementation that carries
the orthogonality center through the SWAP schedule can reduce this overhead.
Accordingly, fixed-chi times here are a reproducible conventional baseline,
not a claim of state-of-the-art boundary-MPS engineering. This limitation
does not explain away the accuracy result: it can change runtime, but not the
recorded estimates produced by the specified canonical truncations.

## Accuracy--cost result

The requested deeper N=14 sweep gives the clearest answer. Increasing `chi`
does move the estimate monotonically toward the exact count, but not remotely
fast enough to make the contraction useful:

| chi | estimate | relative error | median time | peak RSS | time / exact E50 |
|---:|---:|---:|---:|---:|---:|
| 4 | -1.70e-17 | 1.000000 | 0.480 s | 371.8 MiB | 31.7x |
| 8 | 1.31356 | 0.999996407 | 1.002 s | 381.1 MiB | 66.1x |
| 16 | 1.40069 | 0.999996169 | 2.130 s | 371.1 MiB | 140.6x |
| 32 | 2.10073 | 0.999994254 | 9.437 s | 509.1 MiB | 623.1x |
| 64 | 5.50888 | 0.999984932 | 49.041 s | 710.2 MiB | 3,237.9x |
| 128 | 13.40356 | 0.999963338 | 198.414 s | 692.2 MiB | 13,100.0x |

The exact reference is `Q(14)=365596`. The same-node exact E50 explicit-C
contraction, forced to its three-prime CRT backend, took 0.015146118 s median
over seven measured samples after two warmups (job `41529105`). Thus this
particular truncation baseline is not only inaccurate: even chi=4 is slower
than the exact structure-aware contraction. The comparison is intentionally
against an actual Sec. VI tensor contraction, not DFS.

The fixed-cap scaling sweep reinforces that conclusion. At N=13--20 every
chi=4 result has relative error effectively one. Chi=16 remains at relative
error 0.999999928 for N=16 and 0.999999999946 for N=20, while median time rises
from 3.49 s to 6.81 s. Fixed chi does control dense storage (roughly 367--464
MiB whole-process RSS in this range) and makes runtime growth mild, but it
does so by discarding essentially all of the count-carrying amplitude.

At N=8 the approach to the uncapped result is visible but still poor: chi=128
estimates 54.4344 instead of 92, for 40.8% relative error, and takes 13.86 s.
The uncapped floating geometry check gives 92.000000000008 with no cap-induced
truncation in 12.03 s. The capped run can be slower because the cap does not
remove the exact site-split and canonicalization work, and its different bond
sequence changes the LAPACK kernels; this is why runtime must be measured
rather than inferred from chi alone.

## Interpretation and decision

**REJECT APX1 as a counting algorithm; retain it as a diagnostic and negative
result.** No capped row is an exact Issue #34 result, and none is suitable for
rounding into one. There is no observed chi regime that trades a modest count
error for useful speed: the count is already badly attenuated at N=8, the
relative error approaches one as N grows, and the specialized exact sparse
PEPS contraction is much faster at the controlled N=14 point.

This result still has algorithmic value. It shows that the useful structure
in the N-queens PEPS is combinatorial sparsity and deterministic signal
propagation, not low floating-point Schmidt rank along this boundary path.
The accepted exact algorithm exploits those 17-entry local truth tables,
boundary constraints, D4 slicing, and a compact row frontier without changing
the network. Generic hard-chi compression destroys the small surviving sector
that encodes the count. A future approximate study would need an error-aware
method targeted at the counting sector (for example multiprecision and
symmetry/block structure), plus an independently demonstrated speed advantage;
simply increasing chi is not supported by these measurements.

The publication figures are `figures/e51_chi_tradeoff.svg` and
`figures/e51_fixed_chi_scaling.svg`. All 71 final raw points, including N=14
chi=32/64/128, remain in the machine-readable benchmark CSVs; rejected pilot
and numerical-incident artifacts are stored separately and never mixed into
the final curves.

## Same-node exact-algorithm comparison addendum

The truncation question also motivated a controlled comparison of the exact
implementations used throughout the research story. This is a benchmark
addendum, not a reclassification of APX1: the three PEPS families below remain
exact, while DFS remains a non-tensor comparator.

| Plot family | Frozen source | Mathematical/algorithmic role | Requested range | Threads |
|---|---|---|---:|---:|
| DFS bitmask | `b89f4f1`, `src/bin/dfs_bitmask.rs` | independent conventional comparator; never called PEPS | 1--22 | 128 |
| naive PEPS | `20b5334`, “Index C entries by incoming virtual signature” | explicit-C HashMap row contraction before later frontier/tail optimizations | 1--16 | 1 |
| latest PEPS, no TreeSA | source `fc0921b`, algorithm `ea5b985` | C-certified last-six D4 contraction with forced three-prime CRT | 1--22 | 128 |
| TreeSA PEPS | source `c715e36`, implementation `e9a80a5` | explicit-C, checked-u128 D4 site-tree executor using an actual TreeSA plan | 2--11 exact execution | 1 |

The TreeSA line needs a precise qualification. It is not E50 with a Boolean
planner switch: E50's row macro contraction and streamed last-six terminal do
not expose the site tensors expected by the E37 tree executor. It is a
separate, fidelity-tested explicit-C contraction family. The plot therefore
compares complete implementations, while the two TreeSA time series isolate
the executor and then add `optimization_seconds` from the Julia planner.
Calling the second curve “E50 plus TreeSA” would overstate what was measured.

All comparison jobs are pinned to the same exclusive SCNet node `b10r4n19`:
two AMD EPYC 7742 sockets, 128 physical cores, one hardware thread per core,
255,551 MiB configured RAM. Rust binaries use release/thin-LTO and were built
with rustc 1.97.1 / LLVM 22.1.6 for `x86_64-unknown-linux-musl`; TreeSA uses
Julia 1.11.5 and OMEinsumContractionOrders 1.3.0, seed 20260729, 10 trials, and
50 iterations. A Slurm `afterany` chain prevents overlap. Queue promotion only
changes eligibility order; it cannot affect in-job timings.

Every N is a fresh process. Repeated small points report internal kernel
medians; larger points explicitly report one sample rather than disguising it
as a stable median. GNU `time -v` measures each process's maximum RSS. For
TreeSA the Julia planner and Rust executor have separate time/RSS files:
“executor only” excludes path search, “plan + executor” adds measured TreeSA
optimization time, and end-to-end peak RSS is the maximum of the two sequential
process peaks rather than their sum. The historical executable's internal
Linux RSS field is always zero and is deliberately ignored.

The “latest PEPS” line deliberately forces the same three-prime scalar CRT
backend at every N so that an arithmetic/backend switch is not hidden inside
the scaling curve. It is the scalable N>21 implementation, but not a claim of
the fastest possible dispatch for every small N: checked scalar-u64 is valid
and usually cheaper through N=20. Consequently this curve answers how one
fixed exact production backend scales; it is not a per-N cherry-picked lower
envelope.

The same node and build family do not make this a formulation-level matched
baseline. The DFS comparator directly counts the final row but does not have a
last-six terminal expansion; it targets 64 rather than 512 tasks per thread,
uses different atomic chunking, and accumulates checked integer subtrees rather
than three forced CRT lanes. These curves compare frozen complete
implementations. The publication-revision R2 experiment must equalize terminal
depth, seeding/chunking, and arithmetic in both directions before any speed
difference is attributed to tensor-network provenance.

The exact TreeSA executor stops at N=11 because its preregistered sparse-support
kill gate failed at N=8--11. A TreeSA N=20 plan exists, but executing it after
that failure would turn the benchmark into an unbounded OOM/timeout attempt;
the missing N=12--22 TreeSA line is therefore an experimental decision, not
missing data silently interpolated by the plot.

### Controlled results available before the final N=22 data lock

The N=14--20 frozen-implementation comparison is already complete. Ratios are
reported as `DFS time / latest-PEPS time`; values above one mean the latest PEPS
executable is faster. This is deliberately not called a formulation advantage.

| N | DFS time (s) | latest PEPS time (s) | DFS / PEPS | Observation |
|---:|---:|---:|---:|---|
| 14 | 0.010902 | 0.015420 | 0.7070 | DFS faster; launch/backend overhead dominates |
| 15 | 0.025699 | 0.031871 | 0.8063 | DFS faster; still a small kernel |
| 16 | 0.075594 | 0.069050 | 1.0948 | latest PEPS crossover |
| 17 | 0.414400 | 0.365453 | 1.1339 | latest PEPS faster |
| 18 | 2.369722 | 2.220825 | 1.0670 | latest PEPS faster |
| 19 | 18.747813 | 17.040845 | 1.1002 | latest PEPS faster |
| 20 | 141.132524 | 131.450921 | 1.0737 | latest PEPS faster |
| 21 | 1,270.576339 | 1,188.372938 | 1.0692 | latest PEPS faster; one exact sample |

Across N=16--21 the ratio remains between 1.067 and 1.134. The correct statement
is that this C-derived production implementation is competitive with, and in
these frozen runs modestly faster than, the repository DFS comparator. The
unmatched last-six, task, chunking, and arithmetic policies above prevent a
claim that tensor-network provenance caused the difference.

The historical N=16 point makes the representation transition much clearer
than the late-N ratio: naive explicit-C contraction took 1,197.111 s, examined
19,240,119,845 indexed local entries, materialized peak support 194,209,640, and
used 30.625 GiB GNU-RSS. Latest PEPS took 0.069050 s and 10.074 MiB on the same
node: 17,337x lower wall time and 3,113x lower RSS. DFS used 3.137 MiB. This is
an implementation-history comparison, not an equal-thread speedup: naive is
serial, whereas latest PEPS and DFS request 128 physical cores.

TreeSA provides a different negative result. At N=8--11 its optimizer alone
took 15.125--33.346 s and about 392--397 MiB peak RSS; executor support failed
the preregistered row-baseline gate at every checkpoint. The non-monotone N=10
and N=11 executor times are retained rather than smoothed, because contraction
tree quality—not only N—controls these small exact site-tree costs.

The normalizer independently checks all counts through an audited table, frozen
source/algorithm revisions, requested thread counts, exit status, and node
identity. `--require-complete` additionally refuses publication output until
DFS/latest-PEPS N=22 exist. Partial SVGs carry an explicit
`PROVISIONAL` watermark.

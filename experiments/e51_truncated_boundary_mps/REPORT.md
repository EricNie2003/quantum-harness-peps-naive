# E51: truncated finite-PEPS boundary-MPS diagnostic

## Preregistration and classification

- Branch: `codex/exp-truncated-boundary-mps`.
- Worktree: `.worktrees/e51-truncated-boundary-mps`.
- Baseline: `bd928cd`.
- Mandatory E46--E50 review commit: `8759325`.
- Decision vocabulary: `DIAGNOSTIC_ONLY` or `REJECT`; never exact production.

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

Final canonical SCNet measurement is pending.

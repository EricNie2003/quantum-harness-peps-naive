# Repository hard constraints

These requirements are mandatory for every implementation and report in this
repository. They come from Liu, Liao, and Wang, *Statistical mechanics of the
N-queens problem*, arXiv:2605.10326v2, Sec. VI.

## Tensor-network fidelity

1. The primary method must be an actual contraction of the Sec. VI tensor
   network. A conventional N-Queens DFS or bitmask frontier algorithm may be
   included only as an independent oracle or comparator and must never be
   labelled as the PEPS implementation.
2. Define the rank-9 site tensor `B` explicitly. It has:
   - one physical occupation index `alpha in {0,1}`;
   - eight dimension-2 virtual indices grouped into four directed constraint
     channels: column, row, down-right diagonal, and down-left diagonal;
   - exactly 17 non-zero entries.
3. The 17 entries of `B` must be generated from Eq. (16):
   - for `alpha=0`, every channel independently passes either signal 0 or
     signal 1, producing `2^4 = 16` entries;
   - for `alpha=1`, all four incoming signals are 0 and all four outgoing
     signals are 1, producing one entry.
4. Define the rank-8 counting tensor `C` explicitly as
   `C = sum_alpha B`. It must also have exactly 17 non-zero entries.
5. Contract the local `C` entries, rather than replacing them with a
   handwritten queen-placement recurrence. Safe sparse indexing or
   coarse-graining is allowed only when it is mechanically derived from `C`
   and tested against the explicit tensor.
6. Apply the Sec. VI boundary vectors:
   - `v0 = (1,0)` at the start of every constraint line;
   - `v1 = (0,1)` at the end of rows and columns, enforcing exactly one queen;
   - `v2 = (1,1)` at the end of both diagonal families, enforcing at most one.
7. The chosen direction of a diagonal channel may be reversed because the
   full line constraint is orientation-independent, but its `v0` and `v2`
   endpoints must be reversed together and the convention must be documented.

## Exactness and validation

1. Final counts must use exact integer or certified finite-field/CRT
   arithmetic. Floating point, SVD thresholds, bond truncation, and
   integer-rounding are forbidden for exact results.
2. Integer accumulation must detect overflow or use arbitrary precision.
3. Tests must verify:
   - `B` has exactly 17 non-zero entries;
   - `C` has exactly 17 non-zero entries;
   - the empty and occupied local truth tables;
   - the `v0`, `v1`, and `v2` boundary behavior;
   - small-N results against an implementation-independent oracle;
   - known values of `Q(N)`.
4. A report must distinguish clearly between:
   - explicit local-tensor contraction;
   - a proved equivalent optimized contraction;
   - classic DFS/backtracking comparisons.
5. No result may be described as satisfying Issue #34 merely because its
   count agrees with OEIS. Tensor construction and contraction fidelity are
   separate acceptance conditions.

## Benchmark requirements

1. Benchmark release builds, and record the exact command, compiler, CPU,
   thread count, and repetition policy.
2. Record at least:
   - `N` and exact count;
   - verification status;
   - wall time;
   - peak resident memory/RSS;
   - peak sparse support;
   - local tensor-entry examinations and accepted entries.
3. Preserve raw machine-readable CSV results in `benchmarks/`.
4. Reports must state the memory measurement method and its limitations.
5. Never reuse benchmark data from a different algorithm after changing the
   contraction implementation.

## Auto-research direction

The file `nqueens_issue34_autoresearch_plan.md` contains the auto-research
trial directions, experiment gates, benchmark protocol, and scaling/reporting
requirements for Issue #34. Future work should prioritize a genuine PEPS
implementation: explicitly construct the Sec. VI local tensors and contract
their virtual bonds (with a mechanically derived sparse representation allowed
after tensor-level tests). Traditional DFS/backtracking or bitmask pruning is
useful only as an independent oracle or comparator; it must not be presented as
the PEPS method or used to replace the tensor contraction.

Every attempted optimization of the naive contraction must be isolated in its
own Git worktree (and normally its own experiment branch). The baseline
worktree must remain unchanged so that results can be reproduced and compared
fairly. An experiment is incomplete until it has a self-contained report that
records its hypothesis, exact code revision, worktree/branch, PEPS contraction
convention, arithmetic backend, hardware and build configuration, commands,
correctness checks, runtime/memory/support measurements, raw result paths, and
a decision to keep or reject the change. Benchmark data from one worktree or
algorithm must never be silently reused for another.

The optimization objective is a strictly exact PEPS contraction algorithm for
the N-Queens count: it must preserve the local-tensor construction and all
boundary constraints, pass independent correctness checks, and ultimately aim
for throughput substantially beyond the same-hardware DFS/bitmask baselines.
DFS/bitmask speed is a comparison target, not an implementation shortcut. When
the resource projection and correctness gates permit, the research should also
attempt to compute the currently unavailable `Q(28)` value; if it is not
reached, the report must give the largest verified `N`, measured scaling, and
the precise limiting bottleneck.

## Five-direction review gate

After every five distinct optimization directions have been attempted, stop
starting new experiments and perform a mandatory research review before
continuing. The review must summarize the preceding results, identify which
mechanisms produced each speed, memory, support, or scalability improvement,
and explain why changes without measurable performance benefit failed (for
example, hash overhead, memory traffic, insufficient support reduction,
compiler effects, or an invalid cost model). It must compare the observations
with the original hypotheses and update the experiment priorities, benchmarks,
and resource projections.

The review is allowed—and when the evidence requires it, expected—to reject
or completely overturn the previously specified optimization plan. A revised
plan must state the new hypothesis, its PEPS/exactness obligations, the next
research directions, the kill criteria, and the experiment needed to
distinguish the new plan from the old one. Do not begin the sixth direction
until this review and revised direction list have been recorded in the
research-plan/report artifacts.

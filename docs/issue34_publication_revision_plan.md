# Issue #34 publication-revision experiment and report plan

## Decision

The reviewer assessment in `审稿意见.md` is directionally correct. The strongest
publishable result is not that a PEPS formulation intrinsically beats N-Queens
DFS. It is that an explicit, exact sparse tensor network can be mechanically
compiled and reassociated into a competitive search-shaped contraction, with a
testable provenance chain from the local tensor to the hot kernel.

The review predates two material results: the exact production path now reaches
verified `Q(22)`, and APX1 provides a conventional finite-PEPS truncation study.
Those updates improve completeness but do not remove the two central objections:
the published DFS comparison is not yet optimization-matched, and the method has
not yet been demonstrated on another constraint family. `Q(27)` and `Q(28)`
also remain unavailable.

This plan is a publication/reporting track. The comparator work below must never
be called PEPS, and it does not silently consume an E61 PEPS-optimization ID.
Any new production optimization still needs its own worktree and the post-E60
direction choice required by the auto-research plan.

## R1: theorem and executable equivalence certificate

Review and complete `docs/sec_vi_row_automaton_equivalence.md`, which organizes
the formal statement and proof around four objects that the paper must not conflate:

1. the explicit rank-9 `B`, rank-8 `C`, and boundary vectors;
2. row-wise elimination of horizontal virtual indices;
3. the resulting three-mask transfer automaton;
4. breadth-first, hybrid merge/tail, and conventional DFS execution schedules.

The main theorem should state that, for the Sec. VI tensor with row `v0...v1`
boundaries, contracting one row maps an incoming `(columns, diag_dr, diag_dl)`
boundary to exactly the legal one-bit occupations

```text
positions = board_mask & !(columns | diag_dr | diag_dl)
```

with the documented column and diagonal updates. Induction over rows gives a
weight-preserving bijection between nonzero tensor-contraction terms and legal
N-Queens search paths. Associativity and distributivity then justify the hybrid
prefix/tail and certified last-k trees as contraction reassociations, not a
change of network.

The proof is the primary artifact. Tests support it but do not replace it. Add:

- exhaustive explicit-C versus compiled-row checks on every reachable boundary
  through the existing small-N gate;
- canonical per-depth trace hashes comparing explicit-C, compiled-row, and an
  independently implemented DFS for small N;
- forced failures when any C entry, coefficient, leg orientation, or boundary
  vector changes;
- a machine-readable certificate recording tensor hash, compiler classification,
  trace hashes, and checked arithmetic backend.

## R2: fully matched DFS comparison

Keep `dfs_bitmask.rs` as an independent, explicitly non-tensor comparator. Add a
new isolated `review_matched_dfs` worktree and implement the reviewer-requested
optimizations independently rather than calling the PEPS code.

### Required ablation matrix

| Variant | Last-k | First-row reflection | Task target / chunking | Arithmetic | Purpose |
|---|---:|:---:|---|---|---|
| D0 | 1 | on | current DFS policy | checked integer | preserve the original independent oracle |
| D1 | 4/5/6 | on | current DFS policy | checked integer | isolate terminal unrolling |
| D2 | best matched k | on | same grid as PEPS | checked integer | isolate seeding and scheduling |
| D3 | best matched k | on | same selected policy | three-prime CRT | arithmetic parity with the scalable PEPS curve |
| P-u64 | certified last-6 | on | selected matched policy | checked u64 | same-backend PEPS control where its proof bound permits |
| P-CRT | certified last-6 | on | selected matched policy | three-prime CRT | scalable PEPS control |

The current DFS already has first-row reflection, checked accumulation, dynamic
workers, and prefix splitting. The important unresolved asymmetries are terminal
last-six expansion, `64` versus `512` target tasks per thread, task chunking, and
integer versus forced-CRT arithmetic.

Add one attribution-only shared-kernel experiment in which a handwritten DFS
front end and the explicit-C compiler feed the same terminal kernel and task
executor. This variant is not an independent oracle and must not be used for a
headline speed claim. Its purpose is to measure how much time remains in tensor
construction/certification, prefix generation, and scheduling once execution is
literally identical.

### Benchmark protocol

- one exclusive, pinned SCNet node; identical rustc, target, release/thin-LTO,
  codegen units, CPU binding, thread count, and process environment;
- alternating method order within a serialized allocation to reduce temporal
  bias, while keeping every measured N in a fresh process for RSS;
- N=14--18: at least nine samples after two warmups; N=19--20: at least three
  samples after one warmup; N=21--22: exact single samples unless the preregistered
  budget permits repeats;
- report median, min, p10, p90, paired ratios, process wall, GNU-time RSS,
  task count, split depth, recursive nodes, candidate placements, accepted
  C-derived transitions, and arithmetic lanes;
- preserve all raw rows, including unfavorable and bimodal samples.

Run this as a gated funnel rather than carrying every ablation to N=22. Execute
the full D0--D3/P matrix at N=14--18; freeze the best last-k and scheduling policy
before N=19; retain only D0, the selected matched DFS, and the arithmetic-matched
PEPS controls at N=19--20. Authorize paired N=21--22 single samples only if the
N=20 wall projection fits the declared allocation, and do not reuse an older
curve as though it were a temporally paired sample. This retains attribution
while avoiding several redundant multi-hour N=22 profiles.

For the selected matched DFS/PEPS pair, add a small strong-scaling control at
N=18 and N=19 over 1, 16, 32, 64, and 128 physical cores. Both front ends must
produce the same canonically sorted prefix-task manifest and record its hash;
reuse that fixed manifest across thread counts so changing the task generator
does not masquerade as parallel speedup. Report speedup, efficiency, task-tail
imbalance, and socket/NUMA binding as well as wall time.

Do not write “PEPS beats DFS” unless the matched PEPS/DFS ratio remains below one
with a preregistered uncertainty rule on at least three adjacent late-N sizes.
If matched DFS wins, the result is still publishable: the correct conclusion is
that tensor compilation recovers a competitive search kernel rather than a
stronger algorithmic exponent.

## R3: generalization beyond the uniform 17-entry tensor

The minimum practical additional family is **blocked and integer-weighted
N-Queens**. It changes the local tensors across the lattice while retaining a
clear independent oracle and the line-constraint structure needed to test the
compiler.

- A blocked site deletes its occupied local transition.
- A weighted site assigns a small exact nonnegative integer coefficient to the
  occupied transition.
- The compiler must accept only the mechanically recognized identity passes plus
  the optional weighted occupied transition; it must fail closed on every other
  tensor.
- The result is an exact weighted partition sum, accumulated with checked integer
  or certified CRT arithmetic. No floating point or rounding is allowed.

Preregister N=8,10,12,14, blocked densities 0, 0.10, 0.25, and 0.50, and at least
20 deterministic instance seeds per nonzero density. Include deterministic
small integer weight families. Validate literal tensor contraction against the
compiled contraction and an independently implemented oracle for every instance
through N=10; at N=12 and N=14, run all compiled/oracle pairs but reserve the
expensive literal contraction for a preregistered three-seed audit subset unless
the N=10 resource gate justifies more. Compare literal sparse transfer where
feasible, compiled flat transfer, adaptive merge/tail, and independent DFS using
wall time, RSS, peak support, and accepted work. This sampling rule avoids
turning tensor-level validation into hundreds of uncontrolled naive N=14 runs.

The experiment succeeds scientifically if exactness holds and it explains when
the compilation/reassociation pipeline transfers or fails; it does not need to
produce a speed win. If a venue judges blocked/weighted queens too close to the
original tensor, the stronger follow-up should be a grid independent-set or
exact-cover tensor with the same explicit-tensor -> compiler -> literal oracle ->
optimized association evidence chain.

## R4: separate Issue #34 frontier track

Publication readiness and Issue #34 acceptance are different gates. The report
must retain the verified Q(22) milestone while stating that Q(27), Q(28), and the
harness PR remain incomplete.

The live issue has two layers that must be quoted accurately. Its body requests
an exact Sec. VI contraction and explicitly requires reproduction of Q(27). A
later maintainer comment broadens the challenge to any problem-oriented method
that advances the frontier. This repository intentionally keeps the stricter
tensor-fidelity contract: a DFS frontier run can be a separate challenge track,
but cannot be relabeled as the PEPS result. The issue's known-values table also
contains a malformed Q(25): the audited values are
`Q(24)=227,514,171,973,736` and `Q(25)=2,207,893,435,808,352`.

Before spending a full large-N allocation, add a recoverable sector manifest:

- immutable task IDs and tensor/compiler/binary hashes;
- exact coverage and no-duplicate checks;
- checkpoint/retry and deterministic reduction;
- per-task wall distributions, not only an average;
- independent replay of selected heavy sectors and final CRT uniqueness proof.

Use an N=23 task-distribution pilot to measure median, p90, p99, maximum, and
load imbalance before authorizing N=23/24 full runs. A one-node extrapolation to
Q(28) remains a no-go analysis, not a computation attempt. Reaching Q(27/28)
requires either measured node-count reduction or a materially different exact
parallel-throughput platform; constant-factor CPU tuning is not enough.

## Issue/task design

Split the issue checklist into independently auditable gates:

1. **Tensor fidelity:** explicit B/C, 17 entries, boundaries, orientation, and
   literal small-N contraction.
2. **Equivalence:** theorem, compiler failure modes, reachable-parent replay,
   trace certificate, and arithmetic proof.
3. **Matched baselines:** original independent oracle, optimized matched DFS,
   shared-kernel attribution, exact commands, and paired statistics.
4. **Generality:** at least one nonuniform/additional constraint family with its
   own explicit tensors and oracle.
5. **Scaling/frontier:** verified largest N, measured work/RSS scaling, task-tail
   distribution, checkpointability, and an honest Q(28) resource decision.
6. **Deliverables:** raw CSVs, reports, code revisions, worktrees, plots, response
   matrix, and PR status.

Every claim should link to one gate and one artifact. Agreement with a known
count satisfies validation, not tensor fidelity or equivalence by itself.

Use review-response IDs rather than consuming `E61`: the exact optimization
ledger has already completed its E56--E60 mandatory review. A practical task
board is:

| Task | Depends on | Required artifact | Done criterion |
|---|---|---|---|
| R1A theorem | existing tensor definitions | reviewed proof note | all leg conventions and boundaries appear in the formal statement |
| R1B certificate | R1A | JSON/CSV trace certificate + mutation tests | literal C, compiled row, and optimized schedules agree; deliberate mutations fail |
| R2A matched DFS | existing independent oracle | isolated worktree + raw same-node CSV | D0--D3/P controls completed under the preregistered protocol |
| R2B attribution | R2A | shared-kernel timing breakdown | construction, prefix, executor, and reduction costs are separately measured |
| R3 generality | R1B | blocked/weighted tensors, oracle, raw sweep | exact agreement on preregistered instances and transfer/failure analysis |
| R4A frontier pilot | current Q(22) solver | N=23 sector-tail CSV + manifest | coverage/retry/reduction audit passes and p99/max load is measured |
| R4B frontier decision | R4A | signed resource projection | explicit go/no-go for Q(23/24), with Q(27/28 kept as unmet unless computed |
| R5 manuscript/PR | R1--R4 evidence | claim-evidence matrix + thematic report | no headline claim lacks a theorem, test, raw table, or literature source |

Do not open all tasks as “run N=...”. R1 and R2 are the blocking publication
tasks; R3 is the generality gate; R4 is a separately budgeted frontier program.
APX1 and TreeSA remain informative negative/diagnostic evidence and should not
block the matched-baseline paper revision.

Keep the repository layout equally explicit:

```text
docs/sec_vi_row_automaton_equivalence.md       # R1 mathematical statement
certificates/issue34_equivalence/              # R1 hashes, traces, mutations
experiments/review_r2_matched_dfs/             # isolated comparator worktree report
experiments/review_r3_blocked_weighted/        # second-family tensors and sweep
experiments/review_r4_frontier_manifest/       # sector/retry/reduction pilot
benchmarks/review_r2_*.csv                     # normalized and raw matched runs
benchmarks/review_r3_*.csv
benchmarks/review_r4_*.csv
docs/issue34_reviewer_response_matrix.md        # concern -> evidence -> claim change
```

Each experiment directory needs its own report, revision/worktree, exact
commands, raw-input paths, correctness gates, hardware, and keep/reject decision,
just like the numbered optimization studies. The response matrix should link to
those immutable artifacts rather than duplicate their measurements.

## Report design

The main report/paper should be thematic rather than an E1--E60 diary:

1. claim, scope, and non-claims;
2. explicit Sec. VI network and boundaries;
3. row-automaton equivalence theorem and compiler certificate;
4. contraction schedules and the merge-to-recursive-tail transition;
5. matched DFS experiment and cost attribution;
6. generalization experiment;
7. selected negative-result mechanisms;
8. exact scaling, Q(22), and the Q(27)/Q(28) limitation;
9. reproducibility and claim-evidence matrix.

Move the full chronological experiment ledger, cache/layout micro-ablations, and
complete raw tables to supplementary reports. Keep five main visuals:

1. a commuting diagram from B/C to row automaton to alternate schedules;
2. frontier materialization versus streamed contraction;
3. matched time and RSS curves plus a paired-ratio panel;
4. generalization accuracy/work/performance;
5. a mechanism map for informative negative results (TreeSA, DD, low rank, full
   D4, and memoization).

A defensible title and central claim are:

> **When exact sparse tensor contractions compile to search: a
> fidelity-certified N-Queens case study**

> Exact sparse tensor networks can, through local partial evaluation and
> contraction reassociation, mechanically become search-shaped algorithms. We
> provide a verifiable compilation chain, matched performance attribution, and
> evidence for where standard tensor-network optimizations succeed or fail.

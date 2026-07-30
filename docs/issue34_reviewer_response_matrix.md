# Issue #34 reviewer-response and claim-evidence matrix

This matrix responds to `审稿意见.md` without editing or weakening the review.
It is a living publication artifact, not evidence that an open experiment has
already succeeded. Statuses are evaluated against the repository state at the
2026-07-30 report cut.

## Status vocabulary

- **Resolved:** the required evidence exists and is linked.
- **Partial:** material evidence exists, but the reviewer's gate is not closed.
- **Open:** the required experiment or deliverable has not been completed.
- **Not a goal:** the claim is explicitly removed rather than defended.

## Response matrix

| Reviewer concern | Status | Evidence already present | Required closing artifact | Permitted manuscript claim |
|---|---|---|---|---|
| Is the optimized method still a Sec. VI contraction? | Partial | explicit 17-entry B/C tests, boundary tests, fail-closed C compiler, reachable-parent and terminal replay; theorem draft in `docs/sec_vi_row_automaton_equivalence.md` | R1 trace certificate, mutation suite, and reviewed proof | “proved-equivalent optimized contraction” only after R1 closes |
| Is the three-mask recurrence merely handwritten DFS? | Partial | compiler derives legal local transitions from C and rejects unrecognized tensors | weight-preserving row-transfer/path isomorphism plus literal-C/compiled/optimized trace hashes | distinguish tensor definition, compiler, contraction tree, and execution schedule |
| Does PEPS intrinsically beat DFS? | Not a goal | frozen complete implementations share one node; current controls expose terminal, task, and arithmetic asymmetries | R2 D0--D3/P matched matrix and paired statistics | no formulation-level speed claim from the current curve |
| Is the local DFS still a useful comparator? | Resolved | independent `dfs_bitmask.rs`, separate code path, known-count validation, same-node raw measurements | retain it unchanged as the oracle/control when adding matched variants | “independent conventional DFS comparator,” never PEPS |
| Did the project advance the enumeration frontier? | Open | exact Q(22), measured work/RSS growth, held-out scaling check | exact Q(27) for the issue's minimum benchmark; Q(28) only if actually computed | “method and scaling study,” not a record computation |
| Is Q(28) presently feasible on one CPU node? | Resolved as a no-go for that setup | N=18--22 measured curve and N=22 full profile show exponential accepted-transition work | update only if a new method reduces work or a distributed platform is measured | resource projection, never a claimed computation |
| Does the compilation/reassociation idea generalize? | Open | no second exact constraint family; APX1 changes numerical treatment, not the problem family | R3 blocked/weighted sweep, with literal tensors and independent oracle | no general #CSP claim before R3 |
| Is TreeSA an optimized switch in the production solver? | Resolved | separate TreeSA plan and explicit-C executor, exact N=2--11 data, failed support gate | none; retain implementation distinction in captions | site-tree diagnostic, not “E50 with/without TreeSA” |
| Does finite-PEPS truncation offer an accuracy/speed tradeoff here? | Resolved for the tested N=14 regime | APX1 chi sweep, exact reference, same-node E50 control, canonical SVD tests | broader N/chi work only if a new truncation has a credible accuracy signal | negative diagnostic; never an exact Issue #34 result |
| Are all historical E1--E60 directions main-text contributions? | Not a goal | complete chronological ledger and five-direction reviews remain reproducible | thematic main report and selected mechanism figures | chronology belongs in supplement; mechanisms belong in main text |
| Are source benchmark values reliable? | Partial | audited repository table through Q(27) and runtime verification through Q(22) | keep independent table checks in every normalizer/certificate | note that the live issue's Q(25) table entry is malformed |
| Is the challenge complete? | Open | tensor fidelity, exactness, Q(8/16/20), scaling and reports are present | Q(27), final frontier decision/Q(28), and harness PR | explicitly list passed and unmet acceptance items |

## Claim changes for the abstract and conclusion

Remove or avoid:

- “PEPS beats DFS.”
- “The N-Queens computational frontier was advanced.”
- “TreeSA optimization was enabled in the production solver.”
- “Truncated PEPS is a viable exact counting method.”
- “Agreement with OEIS proves tensor-network fidelity.”

Use instead, subject to the status gates above:

- “The explicit Sec. VI network was compiled through a fail-closed local-tensor
  transformation into an exact sparse row automaton.”
- “Contraction reassociation changes a materialized frontier into a streamed,
  search-shaped tail without changing the finite tensor sum.”
- “On one frozen same-node comparison, the resulting implementation is
  competitive with the repository DFS control; optimization-matched attribution
  remains an open experiment.”
- “The most informative negative results identify when topology-only order
  search, post-hoc state compression, symmetry, memoization, and numerical bond
  truncation fail to reduce the count-carrying work.”
- “The largest project count is Q(22); Q(27), Q(28), and the challenge PR remain
  incomplete.”

## Source-data note

The live Issue #34 body lists `Q(25)=2,207,893,435,360`. OEIS A000170 and the
repository's audited table give `Q(25)=2,207,893,435,808,352`; the neighboring
value `227,514,171,973,736` is Q(24). The reviewer document's broad conclusion
does not depend on this typo, so the review file is preserved verbatim and the
correction is made only in project-authored reports.

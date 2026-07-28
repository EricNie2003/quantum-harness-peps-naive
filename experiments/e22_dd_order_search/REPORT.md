# E22: actual-node DD variable-order search

## Decision

**KEEP the order-search mechanism, but do not use the DD as production.**
Among 54 exact candidates, a fixed `FamilyPaired-Reverse-201` order reduces
peak boundary nodes by 21.3% at N=8 and 22.2% at N=9 versus E21, satisfying the
two-consecutive-N node gate. At N=10 the reduction falls to 13.0%, and hash/
apply work makes wall time 35–87x slower than direct D4. The result validates
actual-node path search, not the current symbolic solver's competitiveness.

## Search space and fidelity

- Branch/worktree: `codex/exp-dd-order-search`,
  `.worktrees/e22-dd-order-search`; base E21 revision `2de86a7`.
- The 54 candidates are six permutations of `(column, dr, dl)`, three column
  traversals `(forward, reverse, center-out)`, and three layouts
  `(site-blocked, input/output paired, family-major paired)`.
- Every candidate uses the same explicit 17-entry C relation, exact checked
  `u128` terminals, D4 first-row projection, diagonal shifts, v0/v1/v2 and
  canonical unique table. Only variable rank changes.
- Candidate score is `(peak_boundary_nodes, peak_live_nodes)`. No dense width
  or estimated FLOP score participates.
- All 27 release tests and Clippy `-D warnings` pass; every one of the 108
  N=8/N=9 candidate runs produced the known count.

The selected order is `FamilyPaired-Reverse-201`: variables are grouped by
family in the order `(dl, column, dr)`, columns run right-to-left, and each
input bit is adjacent to its corresponding output bit.

## Results

| N | E21 nodes | selected nodes | reduction | selected time (s) | D4 median (s) | DD slowdown |
|---:|---:|---:|---:|---:|---:|---:|
| 7 | 558 | 466 | 16.5% | 0.005870 | 0.0000499 | 117.6x |
| 8 | 1,232 | 969 | 21.3% | 0.012472 | 0.0001261 | 98.9x |
| 9 | 4,421 | 3,441 | 22.2% | 0.043247 | 0.0004115 | 105.1x |
| 10 | 11,683 | 10,167 | 13.0% | 0.134405 | 0.0016764 | 80.2x |

For N=8, the absolute search winner is the reflection-related
`FamilyPaired-Forward-102` at 968 nodes; the selected fixed order has 969. At
N=9 the selected order is the absolute winner at 3,441. This supports a real
geometry mechanism rather than per-N overfitting: keep corresponding diagonal
signals adjacent, and traverse from the D4-selected side.

The selected layout allocates more historical nodes than E21 because relation
construction/apply association changes, so node reduction alone does not
reduce time. `PeakWorkingSetSize` in the N=7–10 sequential run is a
process-lifetime peak and cannot be compared pointwise after an earlier N.

## Consequence

E22 confirms the user's D4/tree-search intuition in a precise restricted sense:
search can improve a D4-compatible symbolic contraction further. It does not
guarantee end-to-end speedup. E23 must reduce terminal/factor work per live
node; otherwise E24's direct sparse-kernel work has a much better chance of
closing the DFS gap.

Commands:

```text
cargo run --release --bin e22_dd_order_search -- 8
cargo run --release --bin e22_dd_order_search -- 9
cargo run --release --bin e22_dd_selected -- 7 10
cargo run --release --bin e21_weighted_dd -- 10 10
cargo run --release --bin e12_d4_orbits -- d4-serial 1 7 10 5
```

Raw data:

- `benchmarks/e22_dd_selected_release.csv`
- `benchmarks/e22_order_search_summary.csv`
- `benchmarks/e22_d4_baseline_release.csv`

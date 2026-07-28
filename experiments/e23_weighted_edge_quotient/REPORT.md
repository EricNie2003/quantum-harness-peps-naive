# E23: exact proportional edge quotient over two finite fields

## Decision

**REJECT a full weighted-edge production rewrite.** Exact projective
normalization merges scalar-multiple ADD subfunctions, but the peak-node
reduction is only 6.1%, 14.6%, and 5.4% at N=8–10. The diagnostic itself adds
26–46% wall time versus E22. Both primes produce identical node counts, so the
negative result is not a single-field accident.

## Diagnostic

- Branch/worktree: `codex/exp-weighted-edge-quotient`,
  `.worktrees/e23-weighted-edge-quotient`; base E22 `4e098ea`.
- The input is E22's selected exact `FamilyPaired-Reverse-201` boundary ADD.
- Each terminal coefficient is reduced in primes 1,000,000,007 and
  1,000,000,009.
- Recursively, each branch factors the first nonzero child-edge coefficient,
  normalizes it to one with an exact modular inverse, and hash-conses
  `(variable, normalized-low-edge, normalized-high-edge)`.
- This merges proportional residual functions without enumerating assignments
  or the concrete sparse frontier. It is a feasibility diagnostic: the
  normalized quotient is not yet used by relational product.

All release tests and Clippy pass. Tests require two-prime node-count equality
through N=6; release results agree through N=10 and retain the known count.

## Results

| N | E22 nodes | proportional nodes | reduction | E22 time (s) | diagnostic time (s) | overhead |
|---:|---:|---:|---:|---:|---:|---:|
| 7 | 466 | 354 | 24.0% | 0.005870 | 0.007422 | 26.4% |
| 8 | 969 | 910 | 6.1% | 0.012472 | 0.016792 | 34.6% |
| 9 | 3,441 | 2,937 | 14.6% | 0.043247 | 0.063026 | 45.7% |
| 10 | 10,167 | 9,622 | 5.4% | 0.134405 | 0.177068 | 31.7% |

At N=10, quotient construction performs about 1.87 million modular
multiplications and 39,854 inversions per prime. A production weighted-edge
relprod could reduce inversion cost with batch inversion, but cannot turn a
5.4% node reduction into the required 20% time/RSS improvement.

The result narrows the low-rank hypothesis: boundary functions have exact
linear dependencies in E15, but most E22 residual subfunctions are not merely
scalar multiples. Exploiting that rank would require multi-dimensional bases,
which E17 showed can fill in badly. E24 should therefore prioritize the direct
sparse production kernel instead of another post-hoc factor format.

Command:

```text
cargo run --release --bin e23_edge_quotient -- 7 10
```

Raw data: `benchmarks/e23_edge_quotient_release.csv`.

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

Run:

```powershell
cargo test --release
cargo run --release -- solve 8 --layers
cargo run --release -- bench 11 --min 4 --repeats 3 --csv
```

Both commands report peak resident memory on Windows using
`GetProcessMemoryInfo`.

An independent optimized DFS bitmask comparator is also available. It is a
classic search baseline, not part of the PEPS implementation:

```powershell
cargo run --release --bin dfs_bitmask -- solve 16 --threads 1
cargo run --release --bin dfs_bitmask -- bench 17 --min 8 --threads 16 --repeats 9 --warmup 2 --csv
```

See [`REPORT.md`](REPORT.md) for the construction, boundary convention,
correctness checks, and measured benchmarks.

For the Chinese Issue #34 submission, open
[`docs/issue34_final_submission_zh.html`](docs/issue34_final_submission_zh.html).
It follows `报告结构.md`, leads with the main research conclusion, preserves the
SCNet measurements behind every plot, and states explicitly that Q(27)/Q(28)
and the final harness PR remain incomplete.

The longer English report remains available at
[`docs/issue34_research_report.html`](docs/issue34_research_report.html). Its
claim-by-claim revision plan, tensor-to-automaton equivalence argument, and
reviewer-response matrix are preserved alongside it in `docs/`.

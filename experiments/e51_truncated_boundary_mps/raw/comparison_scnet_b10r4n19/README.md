# Same-node exact-algorithm comparison raw archive

This directory mirrors the immutable result, log, and batch-script artifacts
from the four comparison stages under
`scnet:quantum.harness/tracks/peps/runs/`. Every measured N was launched as a
fresh process, and all jobs were serialized on exclusive node `b10r4n19`.

The many pending Slurm entries were one `afterany` chain, not simultaneous
measurements. `scontrol top` was used only to order these jobs ahead of the
same user's unrelated pending jobs; it did not preempt running work or alter
the allocated hardware.

| Job | Family | N | Repeats / warmup | Requested memory | Requested limit |
|---:|---|---:|---:|---:|---:|
| 41534890 | TreeSA plan + exact executor | 2--11 | one plan + one executor | 16 GiB | 4 h |
| 41534895 | historical naive PEPS | 1--14 | 3 / no separate warmup | 16 GiB | 2 h |
| 41534900 | DFS comparator | 1--19 | 7 / 2 | 8 GiB | 2 h |
| 41534902 | latest exact PEPS | 1--19 | 7 / 2 | 8 GiB | 2 h |
| 41534906 | historical naive PEPS | 15 | 1 / no separate warmup | 64 GiB | 2 h |
| 41534910 | DFS comparator | 20 | 3 / 1 | 8 GiB | 2 h |
| 41534912 | latest exact PEPS | 20 | 3 / 1 | 8 GiB | 2 h |
| 41534915 | historical naive PEPS | 16 | 1 / no separate warmup | 180 GiB | 8 h |
| 41534917 | DFS comparator | 21 | 1 / 0 | 8 GiB | 4 h |
| 41534920 | latest exact PEPS | 21 | 1 / 0 | 8 GiB | 4 h |
| 41534923 | DFS comparator | 22 | 1 / 0, plus metrics replay | 8 GiB | 12 h |
| 41534930 | latest exact PEPS | 22 | 1 / 0 | 8 GiB | 12 h |

Slurm accounting preserves the exact submission lines:

```text
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=04:00:00 --mem=16G scripts/scnet_compare_treesa.sbatch 2 11 20260729 10 50
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=02:00:00 --mem=16G --dependency=afterany:41534890 scripts/scnet_compare_naive20b.sbatch 1 14 3
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=02:00:00 --mem=8G --dependency=afterany:41534895 scripts/scnet_compare_dfs.sbatch 1 19 7 2
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=02:00:00 --mem=8G --dependency=afterany:41534900 scripts/scnet_compare_e50.sbatch 1 19 7 2
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=02:00:00 --mem=64G --dependency=afterany:41534902 scripts/scnet_compare_naive20b.sbatch 15 15 1
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=02:00:00 --mem=8G --dependency=afterany:41534906 scripts/scnet_compare_dfs.sbatch 20 20 3 1
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=02:00:00 --mem=8G --dependency=afterany:41534910 scripts/scnet_compare_e50.sbatch 20 20 3 1
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=08:00:00 --mem=180G --dependency=afterany:41534912 scripts/scnet_compare_naive20b.sbatch 16 16 1
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=04:00:00 --mem=8G --dependency=afterany:41534915 scripts/scnet_compare_dfs.sbatch 21 21 1 0
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=04:00:00 --mem=8G --dependency=afterany:41534917 scripts/scnet_compare_e50.sbatch 21 21 1 0
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=12:00:00 --mem=8G --dependency=afterany:41534920 scripts/scnet_compare_dfs.sbatch 22 22 1 0
sbatch --parsable --nodelist=b10r4n19 --exclusive --time=12:00:00 --mem=8G --dependency=afterany:41534923 scripts/scnet_compare_e50.sbatch 22 22 1 0
```

The family subdirectories preserve raw aggregate CSVs, per-N CSVs, per-N GNU
`time -v` records, metadata, binary hashes, logs, and the exact submitted batch
scripts. The normalized publication table is generated outside this raw tree
by `scripts/analyze_algorithm_comparison.py`; raw inputs are never edited to
make the curves agree.

The publication regeneration command is:

```text
python3 experiments/e51_truncated_boundary_mps/scripts/analyze_algorithm_comparison.py \
  --dfs experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/dfs/results \
  --naive experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/naive/results \
  --peps experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/latest_peps/results \
  --treesa experiments/e51_truncated_boundary_mps/raw/comparison_scnet_b10r4n19/treesa/results \
  --output-csv benchmarks/issue34_same_node_algorithm_comparison_scnet.csv \
  --output-time-svg experiments/e51_truncated_boundary_mps/figures/issue34_same_node_time.svg \
  --output-rss-svg experiments/e51_truncated_boundary_mps/figures/issue34_same_node_rss.svg \
  --require-complete
```

The final flag requires exactly DFS N=1--22, naive PEPS N=1--16, latest PEPS
N=1--22, and TreeSA PEPS N=2--11: 70 data rows plus one header. It also verifies
the audited count, frozen revision, requested thread count, exit status, and
`b10r4n19` node provenance for every row. Without that flag an intentionally
partial diagnostic may be generated, but both SVG titles are visibly marked
`PROVISIONAL` and list the missing ranges.

This archive controls node, exclusivity, compiler family, release mode, and
measurement boundaries; it does not claim optimization parity. The DFS path
has no last-six terminal expansion and uses a 64-task/thread target, one-task
atomic scheduling, and checked integer subtrees. The latest PEPS path uses a
certified last-six terminal, 512 tasks/thread, chunk-16 scheduling, and three
forced CRT lanes. A separate reviewer-driven matched-baseline matrix is required
before attributing a wall-time ratio to tensor-network provenance.

The Rust binaries were built locally for `x86_64-unknown-linux-musl` with
rustc 1.97.1 / LLVM 22.1.6, then copied by hash. The compute node's old CentOS
`file` utility describes these PIE executables as shared/dynamically linked;
the build command, target, SHA-256, and the build host's newer `file` output
identify them as static PIE. This display-version difference does not change
the executable bytes used by the jobs.

Frozen-source release validation was rerun after collecting the benchmark
artifacts. `cargo test --release --lib` passes all 8 tests at historical commit
`20b5334`, all 42 tests at TreeSA commit `c715e36`, and all 52 tests in the E50
cluster worktree containing source `fc0921b` / algorithm `ea5b985`. The current
APX1 legacy worktree passes 51 Rust tests plus 616 Julia assertions for the
conventional boundary-MPS diagnostic. These suites separately cover the explicit
17-entry B/C tensors, boundary behavior, optimized-versus-explicit-C replay, independent
small-N oracles, known counts, symmetry, and exact arithmetic as applicable.

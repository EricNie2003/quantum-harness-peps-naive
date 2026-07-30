# SCNet E50 N=22 raw benchmark bundle

This directory preserves the raw output for the completed exact PEPS count
used by the Chinese Issue #34 submission report.

- Slurm job: `41497149`
- Cluster/node: SCNet `b11r2n02`, exclusive 2×64-core AMD EPYC 7742
- Source revision recorded by the job: `258a6cadda71619febaa7d9be176869fd3d045cf`
- Algorithm revision recorded by the job: `ea5b985`
- Exact result: `Q(22) = 2,691,008,701,644`
- Count-only wall time: `10112.287307712 s`
- Whole-process GNU `time -v` maximum RSS: `57892 KiB`
- Arithmetic: checked `u128`; no floating point or truncation

The process performed the timed count first and then an instrumented metrics
replay. Consequently, the GNU maximum-RSS value covers both phases, while the
reported count time excludes the replay. GNU `time -v` samples process-level
resident memory and does not include Slurm/cgroup overhead or memory used by
other processes on the node.

Files:

- `*.csv`: machine-readable result and contraction metrics
- `*.meta.txt`: job, source, build, CPU, thread, and command provenance
- `*.time.txt`: GNU `time -v` output
- `*.err`: Slurm stderr/timing trace

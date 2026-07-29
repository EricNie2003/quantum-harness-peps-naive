# E36：key/coefficient 联合 u64 打包与 exact promotion

## 决策

**KEEP。**

| N | E32 16-byte entry | E36 8-byte entry | speedup | E32 RSS | E36 RSS | RSS reduction |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.1207 s | 0.08335 s | 1.45x | 117.5 MB | 69.0 MB | 41.3% |
| 15 | 0.7674 s | 0.5133 s | 1.50x | 641.8 MB | 328.2 MB | 48.9% |

N=16 也通过已知值验证：Q(16)=14,772,512，单次 4.608 s、
1.841 GB peak RSS、105,530,452 peak support。相对 E32 已记录的
7.054 s / 3.666 GB，candidate/boundary entry 减半同时降低了
generation、sort 和 reduce 的内存流量。

## 假设、实现与 PEPS fidelity

- hypothesis：N<=21 时 3N-bit virtual-boundary key 与有限位宽的
  coefficient 可以共同放进一个 `u64`；8-byte sortable entry 会显著
  减少候选流量和峰值内存；
- code revision：`ce63e57`；
- branch/worktree：`codex/exp-joint-u64` /
  `.worktrees/e36-joint-u64`；
- base：main `61a7153`；
- contraction convention：逐行收缩 Sec. VI 的显式 `C=sum_alpha B`；
  column/row/两族 diagonal 通道方向及 `v0/v1/v2` 边界不变；
- arithmetic：joint fast path、E32 u64 fallback、E31 u128 fallback
  都采用 checked exact integer arithmetic。

对宽度 N，key 使用高 `3N` bits，coefficient 使用低
`64-3N` bits。排序整个 packed word仍以 key 为第一关键字；reduce
通过右移恢复 key、mask 恢复 coefficient，对同 key 做 checked add。
local `C` value、top-row vertical D4 orbit multiplicity 和 duplicate
reduction 的每个乘加都必须不超过 coefficient mask。

若任一 checked 操作失败，公开 API 会从初始 `v0` boundary 完整重跑
E32 u64 contraction；若该层再溢出，则继续重跑 E31 u128 contraction。
测试以 N=8 和人为 1-bit coefficient 强制第一层 promotion，并分别
覆盖 joint→E32 与 joint→E32→E31 两条 exact fallback 路径。这里没有
饱和、回绕、浮点、截断或 DFS 递推。

显式 rank-9 `B` 与 rank-8 `C` 的 17-entry 测试、local truth table、
边界向量测试和 compiled-operator 对显式逐点收缩测试保持通过；
N=0--10 的 joint 路径 count、support、accepted local transitions
与 E32 完全一致。40 个 release tests 和 Clippy 通过。

## Benchmark 与机制分析

同 worktree 公平对照的 N=15：

- entry size：16→8 bytes；
- median wall：0.7674→0.5133 s（33.1%）；
- peak RSS：641.8→328.2 MB（48.9%）；
- generation：最近一次 0.4435→0.2736 s；
- sort：0.3314→0.1599 s；
- reduce：0.05334→0.02961 s；
- aggregate peak thread-local capacity：4,855,808→2,427,904 bytes。

support 和 operator work 没有变化，因此收益不是换算法或漏掉张量
项，而是等价 sparse PEPS contraction 的 record width 与 memory
traffic 减少。N=15 的最大观测 coefficient 仅 797，远低于 19-bit
上限；N=16 为 2024，低于 16-bit 上限。但该观察不能替代动态
overflow 检查，较大 N 也不能据此假设永不 promotion。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release/thin-LTO；
- threads：`RAYON_NUM_THREADS=8`；
- shards：256，Prefix mode；
- commands：
  - `cargo run --release --bin e36_joint_u64 -- 256 14 15 5`
  - `cargo run --release --bin e36_joint_u64 -- 256 16 16 1`
  - `cargo run --release --bin e32_u64_promotion -- 256 14 15 5`
- repetition：N=14--15 五次取中位数和最小值；N=16 因 working set
  较大单次运行；E32 control 在同一 worktree 复测；
- memory：Windows `PeakWorkingSet64`，是包含 allocator、线程栈和
  runtime 的进程高水位，不是精确 live heap；每条 benchmark 命令
  使用新进程，单进程内的后续 repeat 仍可能保留 allocator pages。

Raw data：`benchmarks/e36_joint_u64_release.csv`。

# E42：certified last-k tensor microkernel

## 决策

**KEEP，并入 production。**

| N | E40 control | E42 last-4 | wall reduction | DFS | E42/DFS |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.011827 s | 0.010812 s | 8.6% | 0.010189 s | 1.061x |
| 15 | 0.071739 s | 0.062123 s | 13.4% | 0.066745 s | **0.931x** |
| 16 | 0.418846 s | 0.459842 s | -9.8% | 0.401016 s | 1.147x |

N=14、15 连续两档超过 8% gate。更重要的是，N=15 的 PEPS
中位数比同机 DFS 快 6.9%（DFS/PEPS=1.074x），这是当前协议下第一个
稳定 crossover。N=16 仍有 E40 已记录的系统级双峰：E42 p10=0.380 s
而 median=0.460 s；正式表不以低分位宣称 crossover。

## Hypothesis、实现与反作弊边界

- code revision：`33fefe6`；
- branch/worktree：`codex/exp-last-k` / `.worktrees/e42-last-k`；
- base：main `7184a08`（accepted E40）；
- prefix：E40 actual-support adaptive merged virtual-boundary sectors；
- arithmetic：checked u64，overflow 时从相同 sectors 用 generic-C
  checked u128 完整 replay。

E42 首先从 explicit rank-8 `C` 的 17 个非零元构造
`CompiledRowOperator` 和 `RecursiveTailRelation`。只有
`CertifiedSecViTailPlan::compile` 确认唯一 occupied entry 的四个
constraint channels 都是 0→1、row v0→v1、value=1，last-k fast path
才启用；改变 C 会 fail closed。

microkernel 没有调用 `dfs_bitmask`。它只是把同一个 certified occupied
C transition 的最后 2--4 行循环展开：每一步仍计算合法 incoming
virtual signals、发射 outgoing signals 并移动两个 diagonal channels；
最终 column signals 与 `v1=(0,1)` 收缩，diagonal endpoints 与
`v2=(1,1)` 收缩。generic u128 replay 仍逐层应用 compiled-C relation，
因此展开代码和测量用 reference contraction 是两条实现路径。

测试对 last-2/3/4 的 N=0..10 全部 known counts 做 fast/reference
一致性检查；coefficient limit=1 强制 u64 失败，generic u128 replay
恢复 Q(8)=92。既有测试还逐 reachable parent 比较 recursive successor、
compiled C 与 sitewise explicit C。DFS 只由独立 binary 在 benchmark
阶段调用。

## 消融与机制

7-repeat 探索中位数：

| N | last-2 | last-3 | last-4 |
|---:|---:|---:|---:|
| 14 | 0.011660 s | 0.011740 s | **0.010733 s** |
| 15 | 0.068416 s | 0.066424 s | **0.061407 s** |
| 16 | 0.418457 s | 0.396405 s | **0.379314 s** |

N=15、16 呈清晰单调改善，故 production 选择 last-4。E42 不减少
recursive accepted entries（与 E40 完全相同），收益来自把递归树最宽、
最频繁的四层替换为固定循环，减少函数调用、栈帧、重复 base-case 分支
和 mask 重算。N=15 的 91,865,192 accepted C entries 仍被计数，只是
最后四行用同一 transition 的展开形式执行。

N=16 正式中位数倒退而探索/低分位显著改善，和 E40 已复现的系统双峰
一致。E43 必须把 p90、任务顺序和 chunk size 纳入目标，不能只追低值。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；
- threads：`RAYON_NUM_THREADS=8`，shards=256；DFS `--threads 8`；
- formal repetition：3 warmups + 21 uninstrumented samples；独立一次
  generic u128 profile replay 不进入 wall samples；
- ablation repetition：2 warmups + 7 samples；
- commands：
  - `cargo run --release --bin e42_last_k -- 256 14 16 21 3 4`
  - `cargo run --release --bin e42_last_k -- 256 14 16 7 2 {2,3,4}`
  - `cargo run --release --bin e40_adaptive -- 256 14 16 21`
  - `cargo run --release --bin dfs_bitmask -- bench 16 --min 14 --threads 8 --repeats 21 --warmup 3 --csv`
- memory：Windows `PeakWorkingSet64` 进程高水位，包含 allocator、线程栈、
  runtime 与独立 profile replay，不等于 live heap。

Raw data：

- `benchmarks/e42_last_k_release.csv`
- `benchmarks/e42_last_k_ablation.csv`
- `benchmarks/e42_e40_control_release.csv`
- `benchmarks/e42_dfs_control_release.csv`

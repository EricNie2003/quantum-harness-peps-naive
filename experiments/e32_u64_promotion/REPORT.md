# E32：u64 coefficient 快路径与自动 u128 promotion

## 决策

**KEEP。**

| N | E31 u128 | E32 u64 | speedup | E31 RSS | E32 RSS | RSS reduction |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.177 s | 0.135 s | 1.32x | 167.4 MB | 116.9 MB | 30.2% |
| 15 | 1.228 s | 1.021 s | 1.20x | 951.2 MB | 638.0 MB | 32.9% |

N=16 首次在当前 production 路径上完成：Q(16)=14,772,512，
7.054 s、3.666 GB RSS、105,530,452 peak support，且仍未 promotion。
时间与内存均超过 15% gate；16-byte entries 对 generation、sort 和
reduce 三阶段都有收益。

## Exactness 与 fidelity

- code revision：`3b2ab87`；
- branch/worktree：`codex/exp-u64-promotion` /
  `.worktrees/e32-u64-promotion`；
- base：main `e1c50f5`（accepted E31）；
- fast arithmetic：`u64 key + u64 coefficient`，16 bytes；
- fallback arithmetic：E31 `u64 key + u128 coefficient`，24 bytes。

快路径的每次 local `C` value 乘法、D4 orbit multiplicity 乘法和
sorted duplicate reduction 都使用 checked `u64`。任何 overflow
或超过配置上限都会返回带 row/operation 的 promotion 信号；公开
exact API 随即从初始 `v0` boundary 重新运行 E31 `u128` PEPS，
总 wall time 包含失败尝试和重算。它不是饱和、截断或整数回绕。

测试把 coefficient limit 人为降到 1，在 N=8 第一行强制触发
promotion；fallback 得到 Q(8)=92，且 support 与 E31 相同。正常
limit 下 N=0--10 均验证使用 u64 fast path，并与 E31 的 count、
support、operator work 完全一致。explicit B/C 17-entry、compiled
`C` operator、D4 top-row orbits、prefix shards 和 `v0/v1/v2`
边界均未改变。

36 个 release tests 与 Clippy 通过。Q(14)--Q(16) 均通过 known
count 验证。

## 机制与边界

候选 entry 从 24 bytes 降为 16 bytes，candidate/boundary traffic
理论下降 1/3；N=15 RSS 实测下降 32.9%，与模型吻合。sort 从
0.513 s 降至 0.330 s，generation 从 0.619 s 降至 0.495 s，
reduce 从 0.089 s 降至 0.059 s。线程局部 buffer capacity 从
7.28 MB 降至 4.86 MB。

promotion 的最坏代价是已经完成的 u64 prefix 加完整 u128 replay。
这是 exactness 的必要代价；未来可记录每层最大 coefficient，选择
在预测接近 `u64::MAX` 前直接进入 u128，避免浪费。当前 N<=16
没有发生 promotion，不能据此假定更大 N 永不发生。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release/thin-LTO；
- threads：`RAYON_NUM_THREADS=8`；
- commands：
  - `cargo run --release --bin e32_u64_promotion -- 256 14 15 3`
  - `cargo run --release --bin e32_u64_promotion -- 256 16 16 1`
  - `cargo run --release --bin e31_parallel_generation -- 256 14 15 3`
- repetition：N=14--15 三次取中位数/最小值；N=16 因 3.7 GB
  working set 单次运行。control 在同一 worktree、相同编译选项复测。
- memory：Windows `PeakWorkingSet64`，包含 allocator、线程栈等，
  是进程高水位而非精确 live heap；每个 benchmark 命令为新进程。

Raw data：`benchmarks/e32_u64_promotion_release.csv`。

# E50：3/4-prime residue-lane AVX2 CRT

## 决策

**REJECT explicit AVX2，不并入 production。** E47 last-6 应在后续以
独立、干净的 scalar-CRT direction 应用于 E45；不能把本实验中被拒的
intrinsics 一并合入。

| N | lanes | scalar CRT | AVX2 CRT | AVX2 change |
|---:|---:|---:|---:|---:|
| 16 | 3 | 0.363358 s | 0.358546 s | -1.3% |
| 17 | 3 | 2.584849 s | 2.678727 s | +3.6% |
| 18 | 3 | 18.003857 s | 18.916037 s | +5.1% |
| 16 | 4 | 0.353104 s | 0.357849 s | +1.3% |
| 17 | 4 | 2.625302 s | 2.681498 s | +2.1% |
| 18 | 4 | 19.037093 s | 22.241525 s | +16.8% |

两档 25% keep gate 和 10% kill gate均未达到正向收益；N=17/18 的
3/4-lane AVX2 全部更慢。N=18 只做配对单样本，因为 N=16/17 已触发
kill；它用于满足预注册中等 N 口径，不用于稳定分位数声明。

## 实现、exactness 与 PEPS fidelity

- code revision：`ea5b985`；
- branch/worktree：`codex/exp-crt-avx2` /
  `.worktrees/e50-crt-avx2`；
- base：main `240ab8c`（accepted E47 + E48/E49 reports）；
- primes：E45 的四个经确定性 primality check 的 32-bit primes；
- bound：forced 3/4-prime product 均在启动前 checked，并要求 `M>N!`。

scalar control 与 AVX2 candidate 均使用 E47 C-certified last-6 terminal。
AVX2 在每个递归节点用四个 u64 lanes 同时做 modular addition：
`sum=a+b`，以 `sum>p-1` mask 减 p。所有 residues 始终小于 p，
`2p<2^33`，所以 signed 64-bit compare 不会改变数学结果。top-row
orbit weight 2 用同一 modular vector addition 实现。

测试对 N=0..12、forced 3/4 lanes 比较 AVX2 residues、独立 scalar
residues、CRT reconstruction、generic checked-u128 C replay 和 known
counts。N=17 profile 再记录 4,276,033,044 total accepted C entries；
candidate/control tasks 和 C work 完全相同。非 AVX2 CPU runtime fallback
为 scalar last-6。

## 失败机制与 scalar-control 发现

显式 AVX2 理论上覆盖每个递归节点，比 E49 的单根层 SIMD 更合理，但
仍需让 recursive function 跨层传递/返回 `__m256i`，并在 worker/task
边界保存 YMM values。对于仅 3--4 个固定 lanes，LLVM 对 const-generic
small-array loop 已能展开并进行良好寄存器分配；手写 intrinsics 没有
减少 branch/node work，反而引入 target-feature call ABI 和 spill 压力。
这是从实测推断的机制，未把它冒充硬件计数器结论。

一个重要旁证是 scalar 4-lane 并不比 scalar 3-lane 稳定更慢；N=16/17
甚至略快，说明“lane 数线性乘时间”的旧 Q28 投影过于悲观。与此同时，
E47 last-6 让 forced scalar CRT 在 N=18 约 18--19 s，远低于 E45
last-4 two-lane 的 31.3 s。这个收益属于 **last-6 association 的迁移**，
不是 AVX2；下一检查点应单独实现/消融干净 scalar CRT last-6。

按 E47 N=17→18 的约 8.15x observed ratio，从 19 s 粗略外推到 N=28
仍为约 800--1,000 年数量级。即使删除旧投影的 lane-linear penalty，
也没有改变 exponential tail，Q(28) 仍不可行。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，AVX2；rustc 1.94.0 / LLVM 21.1.8；
- release、thin LTO、codegen-units=1、8 threads；
- N=16/17：1 warmup + 3 samples；N=18：paired single samples；
- N=17 AVX2/4 另做一次 generic C metrics replay；
- commands：
  - `cargo run --release --bin e50_crt_avx2 -- {scalar,avx2} 3 16 17 3 1 2048 0`
  - `cargo run --release --bin e50_crt_avx2 -- {scalar,avx2} 4 16 17 3 1 2048 0`
  - `cargo run --release --bin e50_crt_avx2 -- {scalar,avx2} {3,4} 18 18 1 0 2048 0`
  - `cargo run --release --bin e50_crt_avx2 -- avx2 4 17 17 1 0 2048 1`
- memory：Windows `PeakWorkingSet64` high-water mark，不等于 live heap。

Raw data：`benchmarks/e50_crt_avx2_release.csv`。

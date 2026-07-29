# E46：certified direct-sector scalar-u64 backend

## 决策

**KEEP。N>=17 时替换 E42 为当前最快、低内存 production path；小 N
仍允许按实测选择 E42/E46。**

| N | E46 scalar | E45 CRT | E46/E45 reduction | DFS | E46/DFS | E46 RSS |
|---:|---:|---:|---:|---:|---:|---:|
| 16 | 0.4660 s | 0.4955 s | 6.0% | 0.3991 s | 1.168x | 12.7 MB |
| 17 | 2.8412 s | 4.3846 s | **35.2%** | 2.9999 s | **0.947x** | 17.0 MB |
| 18 | 19.7989 s | 30.0444 s | **34.1%** | 25.3426 s | **0.781x** | 7.2 MB |

N=17/18 连续两档超过相对 E45 25% 的 gate，且 N=18 比同批 DFS
快 21.9%、RSS 低于 25 MB。相对上一 production E42，N=17 快 18.2%；
N=18 的单样本由 23.809 降到 19.799 s，并把 RSS 从 531.9 MB 降到
7.2 MB。

## 实现、exactness 与 PEPS fidelity

- code revision：`22d15b5`；
- branch/worktree：`codex/exp-direct-scalar` /
  `.worktrees/e46-direct-scalar`；
- base：main `ce7f33a`；
- symmetry：top-row vertical reflection orbit；
- state/prefix：完全复用 E45 的三个独立 u64 virtual-boundary masks 和
  direct C-derived sectors，不做 handwritten queen-prefix seeding。

对 N<=20，启动前计算 checked-u128 `N!` 并要求
`N! <= u64::MAX`。所有 local/tail count、orbit-weight multiplication、
worker subtotal 和最终 reduction 仍逐次 checked，且受显式
`coefficient_limit` 限制。因为每项非负、tasks 不重叠，任一 partial sum
不超过总 contraction count，而 `Q(N)<=N!`；因此该 fast path 不是用已知
Q(N) 猜测不溢出。

若 N! 超过 u64，或任一 checked operation/人工 limit 失败，程序从**同一
组 C-derived direct sectors**用 E45 CRT 完整 replay。limit=1 测试实际
强制 N=8 promotion，重构仍为 92。正常 N=0..12 scalar result 再与
generic checked-u128 C replay/known counts 比较。

prefix 与 tail 都先经
`SiteTensorC::sec_vi() -> CompiledRowOperator ->
RecursiveTailRelation -> CertifiedSecViTailPlan`。只有 explicit C 完整含
16 个 unit pass-through 和唯一四通道 0→1 occupied entry 时才进入
mask hot loop；terminal 仍是 column v1、diagonal v2。没有调用 DFS。

## 粒度消融与失败/收益机制

| N | 512 tasks/thread | 2048 tasks/thread | 选择 |
|---:|---:|---:|:---:|
| 16 | 0.4078 s / cut4 / 9,844 | 0.3810 s / cut5 / 70,906 | 2048 |
| 17 | 2.9315 s / cut4 / 14,272 | 2.8321 s / cut5 / 114,434 | 2048 |

E46 与相同 target 的 E45 具有完全相同的 tasks、recursive nodes 和
accepted C entries；速度来自把每 node 的两条 modular lanes 改为一个
checked native-u64 accumulator，不是 work pruning。N=16 的正式
21-repeat 再次出现系统双峰（p10=0.3858、median=0.4660），所以不以
探索低值宣称 crossover。N=17 的 E46/DFS 21-repeat median 和 p90 都
更低。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；
- threads：`RAYON_NUM_THREADS=8`，DFS `--threads 8`；
- N=14--17：3 warmups + 21 samples；N=18：1 warmup + 3 samples；
- metrics：另运行 generic checked-u128 C replay，不进入 wall samples；
- commands：
  - `cargo run --release --bin e46_direct_scalar -- 14 15 21 3 512 1`
  - `cargo run --release --bin e46_direct_scalar -- 16 17 21 3 2048 1`
  - `cargo run --release --bin e46_direct_scalar -- 18 18 3 1 2048 1`
  - `cargo run --release --bin e45_wide_crt -- bench 16 17 5 1 2048 0`
  - `cargo run --release --bin dfs_bitmask -- bench N --min N --threads 8 ... --csv`
- memory：Windows `PeakWorkingSet64` process high-water mark；包含
  allocator/runtime/worker stacks/profile replay，不等于 live heap。

Raw data：

- `benchmarks/e46_direct_scalar_release.csv`
- `benchmarks/e46_target_ablation.csv`
- `benchmarks/e46_controls.csv`

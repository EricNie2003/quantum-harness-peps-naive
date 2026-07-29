# E52：cache-capacity-aware prefix cut

## 决策

**REJECT production。** 64 KiB/worker 规则把 task tape 和 peak RSS
显著缩小，但正式 median 在 N=16/17/18 分别慢 0.7%/0.9%/3.0%。
N=18 超过预注册的 2% wall 退化上限；不能用单个 17.826 s min 代替
19.915 s median 来 KEEP。

| N | control median | cache-cut median | wall change | control / candidate RSS | control / candidate tasks |
|---:|---:|---:|---:|---:|---:|
| 16 | 0.310279 s | 0.312501 s | +0.7% | 12.71 / 8.30 MB | 70,906 / 9,844 |
| 17 | 2.744855 s | 2.770041 s | +0.9% | 17.39 / 10.42 MB | 114,434 / 14,272 |
| 18 | 19.331729 s | 19.915283 s | +3.0% | 7.81 / 6.12 MB | 18,132 / 1,710 |

32 KiB/worker 因 cut 太浅在 N=16/17 慢 3.3%/4.4%；64/128/256 KiB
都选择相同 depth-4 cut，探索网格没有另一个隐藏 winner。64 KiB/worker
虽满足 N=16/17 的 memory-only gate，但 N=18 未通过完整 gate。

## 机制与 hotspot 解释

E51 的 `samply` profile 已测得 production fast solve 的 99.979% leaf
samples 位于 `contract_certified_tail_last_k_u64::<6>`。E52 从外围验证
这一判断：N=18 task storage 从 580,224 bytes 降到 54,720 bytes，
tail tasks 从 18,132 降到 1,710，但完整 work 仍是
29,682,922,254 accepted C entries。少 materialize 一层 task 只把该层
重新放回同一个 recursive hot function，没有减少节点，也没有创建可复用
数据。因此 task tape 更适合 cache 并不能消除 99.98% 热点；更浅 cut
还降低了动态 task 粒度，N=18 median 反而退化。

这不是“cache 一定无关”的证明：它只排除了 prefix task working set。
下一步必须直接改变 hot function 内的 cache/coherence、exact suffix reuse
或 hot code footprint。

## 实现与 PEPS exactness

- code revision：`95b0febe66c4b4c54aa61178dae197bc2231a5ad`；
- branch/worktree：`codex/exp-cache-cut` /
  `.worktrees/e52-cache-cut`；
- base：main `82c5649`，即 E51 REJECT 后未改 production 的 E47 last-6；
- selector 只在完整 C-derived prefix rows 之间停止；下一层若超过
  `threads * cache_kib_per_worker` 且已有 `threads * 64` tasks，则保留
  当前层；
- 每个 prefix successor 仍由 explicit 17-entry C 编译出的
  `RecursiveTailRelation` 生成；column v1、diagonal v2、vertical orbit
  weights、checked-u64 和 certified CRT fallback 均不变；
- N=0..10、四个 cache budgets 都与 generic-C replay/known Q(N)
  一致；正式 control/candidate 的 count 与 total accepted entries 完全
  相同。

完整 release suite 为 52 passed；`cargo fmt --check` 和
`cargo clippy --release --all-targets -- -D warnings` 通过。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16C/32T；
- compiler：rustc 1.94.0 / LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；8 threads；
- grid：N=16/17，1 warmup + 3 repeats，无 metrics replay；
- formal：N=16/17 为 1 warmup + 5 repeats，N=18 为
  1 warmup + 3 repeats，另做一次 generic-C metrics replay；
- commands：
  - `e52_cache_cut control 2048 16 17 5 1 1`
  - `e52_cache_cut cache 64 16 17 5 1 1`
  - 对 N=18 使用相同参数和 3 repeats；
- RSS：Windows `PeakWorkingSet64` process high-water mark，含 runtime、
  allocator、worker stacks 和独立 replay，不等于 live task bytes；
- raw CSV：
  - `benchmarks/e52_cache_budget_grid.csv`
  - `benchmarks/e52_cache_cut_control.csv`
  - `benchmarks/e52_cache_cut_candidate.csv`

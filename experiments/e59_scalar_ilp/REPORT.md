# E59：2/4-sector scalar ILP interleave

## 决策

**REJECT，N=18 early kill。** batch2 在 N=16/17 慢 17.9%/15.9%；
batch4 慢 19.6%/24.5%。递归 active-lane utilization 也没有达到 75%
gate：batch2 为 69.9%/66.8%，batch4 仅 50.9%/46.6%。

| N | scalar | batch2 | change | util | batch4 | change | util |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 16 | 0.320392 s | 0.377652 s | +17.9% | 69.9% | 0.383211 s | +19.6% | 50.9% |
| 17 | 2.761202 s | 3.200497 s | +15.9% | 66.8% | 3.437827 s | +24.5% | 46.6% |

batch recursion 每层从每个 active boundary各取一个 legal C child，再共同
递归；计时 kernel以 const `MEASURE=false` 编译掉统计。独立 measured
replay 显示不同 boundary 的 legal branching 很快分叉：N=17 batch4 的
379,129,272 个 batch calls 中，只有 45,710,608 次四 lane全活，而
194,846,087 次只剩一个 lane。

因此纯 scalar 也重现 E49 的 divergence 机制。数组状态复制、per-lane
active mask、inactive-lane control 和较大的 recursive frame超过了少量
independent mask ILP；失败不能归因于 AVX2 store/extract 本身。

## exactness 与 PEPS fidelity

- code revision：`3fe52240ba520ebb020f23c8db00149fed2bb63f`；
- branch/worktree：`codex/exp-scalar-ilp` /
  `.worktrees/e59-scalar-ilp`；
- base：main `641d906`；E56--E58 rejected code 均不在 baseline；
- 每个 lane 对应完整且不重叠的 `WideCrtTask`；每层使用相同
  `certified_tail_successor`、checked coefficient与 orbit weight；
- terminal 仍调用 certified last-6，column v1 / diagonal v2不变；
- N=0..10 lanes1/2/4 与 generic C replay/known counts一致；invalid
  lanes和 N>20 scalar bound fail closed；
- formal count/tasks/nodes/accepted entries/support 完全相同；
- full release suite 52 passed；format/clippy通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- scalar/batch2：N=16/17，1 warmup + 5 repeats；
- batch4 由 batch2 early kill，使用 1 warmup + 3 repeats；
- generic-C replay 和 batch utilization replay 均不计入 median；
- command：
  `cargo run --release --bin e59_scalar_ilp -- 16 17 <repeats> 1 2048 <lanes>`；
- RSS：Windows `PeakWorkingSet64` process high-water；
- raw CSV：`benchmarks/e59_scalar_ilp_formal.csv`。

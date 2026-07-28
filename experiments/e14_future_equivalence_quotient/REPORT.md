# E14：exact future-equivalence quotient

## 预注册与实现

- branch：`codex/exp-future-equivalence-quotient`；
- worktree：`.worktrees/e14-future-equivalence-quotient`；
- baseline：`a45a27f`；
- candidate：`36126fe67d60cb30d01774fcc41685ff7b9a481c`；
- keep gate：N=10、11 canonical classes 不超过 explicit support 的 70%，quotient native
  replay 不慢于 E11 的 2x；
- kill gate：两档 class ratio 都高于 0.85，或证明/构建成本先爆炸。

先用 production D4 row contraction 枚举每层的具体 reachable packed virtual boundaries。然后
从 bottom `v1/v2` acceptance 开始反向最小化：

\[
\mathrm{sig}_k(s)=
\left\{(\mathrm{class}_{k+1}(s'),\;m_{s\to s'})\right\}_{s'}
\]

两个 state 只有在由显式 compiled `C` 生成的完整 successor-class/multiplicity map 完全相同
时才归为一类。top row 的左右反射 orbit multiplicity 2/1 也进入 signature。class value
使用 checked `u128` 从 bottom 反向重放，最终重构 Q(N)；没有用“completion count 相同”
替代 bisimulation，也没有浮点近似。

测试 N=0--9 quotient count 与 known Q(N) 一致；全部 release tests 27 项、Clippy 和格式检查
通过。

## Benchmark

- CPU：AMD Ryzen 9 7945HX；Windows；
- rustc 1.94.0，release/thin-LTO；1 thread；
- quotient：`cargo run --release --bin e14_future_quotient -- 10 13`，每点一次诊断；
- baseline：`cargo run --release --bin e12_d4_orbits -- d4-serial 1 10 13 3`，三次中位；
- RSS：Windows `PeakWorkingSetSize`；quotient N 递增在同一进程，因此包含 allocator retained
  pages；
- tensor-level local operator 仍由显式 17-entry `C` 编译；forward/backward transition 数
  均记录，二者相等是完整 signature 覆盖的校验。

| N | reachable peak | future-class peak | ratio | quotient build (s) | replay (s) | direct D4 (s) | build/direct |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 10 | 4,510 | 735 | 16.30% | 0.007091 | 0.000083 | 0.001663 | 4.26x |
| 11 | 22,253 | 3,462 | 15.56% | 0.044294 | 0.000257 | 0.008808 | 5.03x |
| 12 | 98,939 | 14,570 | 14.73% | 0.215293 | 0.001253 | 0.043569 | 4.94x |
| 13 | 541,745 | 57,215 | 10.56% | 1.759298 | 0.008917 | 0.263717 | 6.67x |

class peak 的相邻增长为 4.71x、4.21x、3.93x；reachable peak 为 4.93x、4.45x、
5.48x。压缩不仅是常数，当前窗口的 class growth 也更慢。

层数据揭示压缩集中在 suffix 半边。例如 N=13：

- row 6：61,886 states / 53,032 classes；
- row 7：174,057 / 57,215；
- row 8：363,456 / 11,610；
- row 9：541,745 / 737；
- row 10：531,864 / 42；
- bottom：10,964 / 1。

## 严格区分 replay 与构建

预先给定 quotient DAG 后，N=13 exact replay 只需 406,216 quotient edges 和 8.9 ms，
远快于 E11，因此通过“native apply”部分 gate。但当前构建算法必须：

1. 先枚举完整 concrete reachable graph；
2. 再对同一批 transition 做一次反向 signature；
3. 保存所有层 state/class map。

因此单次 Q(N) 的总时间仍比 direct D4 慢 4.3--6.7x，不能宣称已经优化 production solver，
更不能拿 replay-only 时间与 DFS 比。

## 决策

**KEEP 作为强结构证据；不合并为默认求解器。**

class ratio 10.6--16.3% 远过 70% gate，说明 production frontier 确实存在大量 exact
future-equivalence，这与 E13 “换几棵 site contraction tree 不够”形成一致结论。但 E14
尚缺一个无需先枚举 concrete graph 的 direct canonical signature/quotient apply。

下一步：

- E15 用有限域 flattening rank 判断这种非线性 bisimulation 压缩是否伴随低 exact linear
  rank；
- five-direction review 中把“寻找可在线计算的 future signature”提升为最重要候选；
- 若无法避免 concrete prebuild，E14 只能用于重复 replay/checkpoint，不会帮助首次 Q(28)。

原始数据：

- `benchmarks/e14_future_quotient_release.csv`
- `benchmarks/e14_future_quotient_layers.csv`
- `benchmarks/e14_d4_baseline_release.csv`

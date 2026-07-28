# E13：Rust 简化 support-aware contraction-path exploration

## 预注册

- 分支：`codex/exp-support-aware-path-search`；
- worktree：`.worktrees/e13-support-aware-path-search`；
- baseline commit：`cb227b0`；
- candidate code commit：`ba272fa16b0c28c5ea0fe787619440f05e78caef`；
- 思路：参考 OMEinsum 将 contraction tree 作为搜索对象、用 time/space cost 排序的设计，
  但只在 Rust 中实现少量确定性候选，不复刻 Julia 库或 TreeSA；
- keep gate：两个连续 N 的 actual peak-support slope 比 production D4 row sweep 低至少
  10%，且 actual support 至少下降 20%；
- kill gate：只改善 generic/dense tree 指标，actual sparse support 连续两档未降 20%。

## Direct-C sparse tensor oracle

原型从显式 rank-8、17-entry `C` 构造每个 site tensor，按 Sec. VI 方向连接四类 dimension-2
virtual bonds，并直接吸收边界：

- 每条 constraint line 的 start 固定 `v0=(1,0)`；
- row/column end 固定 `v1=(0,1)`；
- diagonal end 与 `v2=(1,1)` 收缩，即对应 index 求和；
- tensor pair contraction 只 join shared virtual-index assignment，按剩余 open-index key 用
  checked `u128` 精确归并。

它不是 DFS recurrence，也不调用 production row operator。测试对 row blocks、column
blocks、balanced rectangles 和 support-aware greedy 四条路径在 N=0--4 全部核验 known
Q(N)；benchmark 进一步核验到 greedy N=11。release tests 27 项、Clippy 和格式检查通过。

### Rust 中实现的简单候选

1. `RowBlocks`：每行先从左到右收缩，再依次合并整行 tensor；
2. `ColumnBlocks`：转置版本；
3. `BalancedRectangles`：递归沿较长边二分矩形并合并两棵子树；
4. `SupportAwareGreedy`：只考虑有 shared bond 的 cluster pair，用
   `support_left * support_right / 2^shared * (output_rank+1)` 的近似分数选最小项。

这对应用户要求的“借鉴思路、Rust 简单探索”。Julia launcher 虽存在，但没有已配置 runtime，
且精细 TreeSA 当前优先级低，因此没有安装 Julia 或 OMEinsum。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；Windows；
- compiler：rustc 1.94.0，release/thin-LTO；
- generic path 命令：`cargo run --release --bin e13_path_search -- PATH MIN_N MAX_N`；
- D4 baseline：`cargo run --release --bin e12_d4_orbits -- d4-serial 1 5 11 3`；
- threads：1；
- generic diagnostic 每个 point 1 次；D4 baseline 每点 3 次并取中位数；
- RSS：Windows `PeakWorkingSetSize`。同一 path 的 N 在同一进程递增，故高水位可能包含
  之前 N 的 allocator-retained pages；
- `cartesian_pair_upper_bound` 是每次 pair contraction 的左右 support 乘积之和，不是 hash
  shared-index join 真正遍历数；`matching_entry_pairs` 才是实际相容 tensor-entry pair 数。

## 候选内部比较

| path | N=5 support | N=6 | N=7 | N=7 time (s) | N=7 peak rank |
|:---|---:|---:|---:|---:|---:|
| row blocks | 7,168 | 65,536 | 589,824 | 1.9216 | 38 |
| column blocks | 8,192 | 73,728 | 655,360 | 1.7437 | 38 |
| balanced rectangles | 768 | 3,648 | 34,304 | 0.01257 | 26 |
| support-aware greedy | **44** | **77** | **400** | **0.00485** | **21** |

Greedy 相对 naive generic row/column tree 是巨大改善；若只看这张表会错误地认为路径搜索已经
成功。这正是必须用 production coarse-grained contraction 再评分的原因。

## 与 production D4 row transfer 的 actual-support 比较

| N | greedy support | D4 row support | greedy / D4 | greedy time (s) | D4 time (s) |
|---:|---:|---:|---:|---:|---:|
| 5 | 44 | 8 | 5.50x | 0.000802 | 0.0000056 |
| 6 | 77 | 22 | 3.50x | 0.002003 | 0.0000141 |
| 7 | 400 | 86 | 4.65x | 0.004850 | 0.0000524 |
| 8 | 3,030 | 272 | 11.14x | 0.014241 | 0.0001511 |
| 9 | 32,678 | 1,210 | 27.01x | 0.045155 | 0.0005890 |
| 10 | 111,800 | 4,510 | 24.79x | 0.144404 | 0.0016734 |
| 11 | 470,776 | 22,253 | 21.16x | 0.827681 | 0.0088907 |

N=11 greedy peak rank 41，也高于 row frontier 的 `3N=33` bit layout。balanced rectangle 在
N=8 的 support 为 34,304，同样是 D4 row 的 126x。所有计数均正确，但没有一条候选达到
actual support gate。

## 决策

**REJECT 当前四类 path；不合并为 production contraction。**

失败机制不是 generic sparse join 不正确，而是 production row operator 已把一整行显式
`C` contraction 做了结构性 partial evaluation。逐 site contraction tree 即使经过简单
support-aware greedy，也会暴露更多 open virtual indices，实际 support 比宏 row transfer
大 3.5--27x。

这不证明所有 tree search 永远无效，但证明当前阶段不应投入完整 OMEinsum/TreeSA 或 Julia
环境配置。只有新的候选能够把“由 `C` 机械生成的 row macro tensor”作为搜索原子，或给出
小于 3N 的 certified separator 时，才值得重开 E13。下一步按计划进入 E14
future-equivalence quotient，因为它直接尝试压缩 production frontier 的 actual support。

原始数据：

- `experiments/e13_support_aware_path_search/results.csv`
- `benchmarks/e13_path_search_release.csv`
- `benchmarks/e13_d4_row_baseline_release.csv`

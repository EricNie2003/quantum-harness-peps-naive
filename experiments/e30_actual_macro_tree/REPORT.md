# E30：actual-cost single/two-row macro tree search

## 决策

**REJECT macro tree search；KEEP“actual cost 而非 dense width”作为方法规则。**

穷举所有由 size-1/size-2 row blocks 构成的 compositions：

| N | candidates | search time | best blocks | baseline work | best work | reduction |
|---:|---:|---:|:---|---:|---:|---:|
| 10 | 89 | 0.175 s | 1-1-2-1-1-1-1-1-1 | 17,119 | 17,119 | 0% |
| 11 | 144 | 0.996 s | 1-1-2-1-1-1-1-1-1-1 | 87,160 | 87,160 | 0% |
| 12 | 233 | 6.967 s | 1-1-2-1-1-1-1-1-1-1-1 | 403,508 | 403,508 | 0% |
| 13 | 377 | 54.883 s | all size-1 | 2,334,177 | 2,334,177 | 0% |

没有两档 actual candidates/RSS/time 降 15%，触发 kill gate。

## 实现与 fidelity

- code revision：`beeefd6`；
- branch/worktree：`codex/exp-actual-macro-tree` /
  `.worktrees/e30-actual-macro-tree`；
- base：E29 revision `f0f672f`，其 E28 production ancestor 保持不变。

每个 composition edge 是 E28 exact single-row C apply 或 E29 exact
two-row C macro。所有路径产生同一个完整 Sec. VI PEPS contraction；
只改变在哪些 row cuts 做 exact sort/reduce。搜索评分按
`(actual C transitions, peak macro candidates, measured replay time)`，
不使用 dense width 或估计 FLOPs。搜索总成本完整计时。

35 个 release tests 和 Clippy 通过；每个搜索 candidate 都验证 known
Q(N)。环境：Ryzen 9 7945HX、rustc 1.94.0、release/thin-LTO、
8 threads；每个 association 运行一次。

## 机制

N=10--12 的 row-2 two-row edge 恰好不增加 aggregate transitions，
因此可在第二评分项上偶尔胜出；但没有减少 work，generic replay 仍受
per-parent temporary expansion 影响。到 N=13，actual-cost 最优已回到
全单行。

composition 数按 Fibonacci 增长，search time 从 N=10 的 0.175 s
升到 N=13 的 54.9 s。即使引入 greedy/local moves 降低搜索成本，
可选 edge 本身没有正收益，因而不会改善 production。

这验证了 OMEinsum/treeSA 思路的适用边界：路径搜索可以叠加 D4 与
稀疏表示，但只有候选 macro association 本身存在 actual work 差异时
才有价值；不能靠搜索修复 E29 的“延迟 canonical merge 导致重复
apply”。

Raw data：`benchmarks/e30_macro_tree_search_release.csv`。

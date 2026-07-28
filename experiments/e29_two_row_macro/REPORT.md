# E29：two-row C-derived macro apply

## 决策

**REJECT。** Exact two-row macro 未出现 N² 灾难，但没有降低 work：

| N | E28 transitions | E29 transitions | change | E28 time | E29 time |
|---:|---:|---:|---:|---:|---:|
| 10 | 17,119 | 17,326 | +1.2% | 0.00179 s | 0.00182 s |
| 11 | 87,160 | 88,552 | +1.6% | 0.00319 s | 0.00523 s |
| 12 | 403,508 | 410,774 | +1.8% | 0.01052 s | 0.01829 s |
| 13 | 2,334,177 | 2,389,413 | +2.4% | 0.04773 s | 0.09829 s |

预注册 keep gate 要求 accepted candidates/row-equivalent 降 25%；
实测连续四档反而上升，N=13 时间为 2.06x，故停止。

## Fidelity

- code revision：`c179426`；
- branch/worktree：`codex/exp-two-row-macro` /
  `.worktrees/e29-two-row-macro`；
- base：E28 main `c576a96`。

每个 macro 逐次调用同一显式 17-entry C 编译出的 sparse local
transition：第一行的 output virtual bonds 直接作为第二行 input，
之后才 materialize/sort/reduce。它不是 queen-pair recurrence。
`v0/v1/v2`、diagonal shifts、checked u128、24-byte exact entries、
Prefix/256 和合法的首行 D4 orbit 全部保留。

35 个 release tests 通过；新增测试在 N=0--10 与 E28 比较 exact
Q(N)。Clippy 通过。环境为 Ryzen 9 7945HX、rustc 1.94.0、
release/thin-LTO、8 threads；kill diagnostic 每点 1 次。

## 机制

两行 macro 省去了奇数 row 的全局 materialization，因此部分 N 的
observed macro-cut support 较低；但也跳过了第一行 boundary 中相同
intermediate keys 的合并。第二行会为来自不同 parents 的等价
intermediate 重复执行 C apply，transition overhead 随 N 从 1.2%
升到 2.4%。per-parent 临时小向量进一步放大 wall time。

因此“减少一次 sort”不足以补偿“推迟一次 exact canonical merge”。
E30 path search 应直接使用这些 actual transition/support costs，验证
任何包含 two-row edge 的 association 都不优于全单行 tree；不得用
dense width 误判。

Raw data：`benchmarks/e29_two_row_macro_release.csv`。

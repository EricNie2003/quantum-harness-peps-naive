# E8：通用 PEPS 消元顺序 oracle

## 预注册

- 分支：`codex/exp-ordering-oracle`
- baseline commit：`265059c54ae05101c96848387300ab0555594467`
- candidate code commit：`e7001ac0dcbc54d4dc16df553d842e7c13e270fb`
- 假设：非逐行的二维消元顺序可能缩小 exact sparse boundary support，从根本上改善
  E6 的 support 增长；
- keep gate：至少两个连续 N 的 peak support 比 row-major 降低 20%，或者显示出明确更低的
  support 增长率；
- kill gate：到 N=6 仍无 support 优势即停止，不把通用 oracle 扩展为生产实现。

## 实现与 PEPS 忠实性

本实验实现一个只用于小 N 的通用、精确 sparse tensor-network contraction：

1. 每个棋盘格直接从 `SiteTensorC.entries()` 构造局域 factor，因而每格实际枚举显式
   17-entry `C`，没有用 queen-placement recurrence 替代；
2. 四类 virtual channel 的内部 bond 分配唯一变量；
3. 开放端严格应用 `v0=(1,0)`、行列末端 `v1=(0,1)`、对角线末端
   `v2=(1,1)`；
4. 每处理一个 site，就对共享变量做 exact sparse join，并在变量的最后一个 incident
   factor 已处理后求和消元；
5. 所有 coefficient 使用 `BigUint`，没有浮点、截断或舍入。

比较三种 site 顺序：

- `row_major`：逐行从左到右；
- `snake`：相邻行方向交替；
- `diagonal_wavefront`：按 `row + col` 的波前顺序。

测试验证三种顺序都是 site permutation，并逐个与已知 `Q(N)` 比较至 N=4。整个仓库
release test 共 17 项通过，Clippy `-D warnings` 和格式检查通过。N=5、6 的最终精确计数
分别为 10、4，均通过 known-value gate。

## Benchmark 方法

- CPU：AMD Ryzen 9 7945HX；
- OS：Windows；
- 编译器：Rust 1.94，release/thin-LTO；
- 线程数：1；
- 命令：
  - `cargo run --release --bin ordering_profile -- 5`
  - `cargo run --release --bin ordering_profile -- 6`
- 重复策略：每个 N 单次诊断运行。此实验的判定指标是确定性的 support/frontier 大小，
  微秒级 wall time 不用于生产性能结论；
- 内存：Windows `GetProcessMemoryInfo` 的 `PeakWorkingSetSize`，表示整个短命进程的峰值
  working set，包含运行时和 allocator；如此小的实例上它受进程固定开销和页面粒度支配，
  不能用于细分 tensor 数据结构占用。

## 结果

| N | ordering | count | peak support | peak frontier vars | candidate pairs | matched pairs | wall time (s) | peak RSS (MiB) |
|---:|:---|---:|---:|---:|---:|---:|---:|---:|
| 5 | row-major | 10 | 23 | 14 | 3,096 | 289 | 0.0000418 | 5.14 |
| 5 | snake | 10 | 23 | 14 | 3,096 | 289 | 0.0000364 | 5.16 |
| 5 | diagonal | 10 | 38 | 16 | 4,355 | 422 | 0.0000486 | 5.16 |
| 6 | row-major | 4 | 72 | 17 | 13,122 | 1,088 | 0.0001108 | 5.16 |
| 6 | snake | 4 | 72 | 17 | 13,122 | 1,088 | 0.0000998 | 5.20 |
| 6 | diagonal | 4 | 125 | 20 | 20,691 | 1,782 | 0.0001289 | 5.20 |

相对 row-major：

- snake 在 N=5、6 的 support、frontier 和 pair work 完全相同，仅改变遍历方向；
- diagonal-wavefront 的 peak support 分别增加 65.2% 和 73.6%，frontier 变量也更多；
- diagonal 的 support 增长比为 `125/38 = 3.29`，高于 row-major 的
  `72/23 = 3.13`，没有出现渐近改善信号。

## 决策

**REJECT，不合并 baseline。**

两个候选都未满足连续两档至少降低 20% 的 keep gate。snake 证明简单换行方向无法改变
边界复杂度；当前 diagonal 波前同时切穿更多 row/column/diagonal channel，增加 frontier
宽度和 sparse support。E8 的通用 oracle 仍保留在实验分支，作为以后评估新 order heuristic
的正确性工具，但不应进入生产 contraction。

本实验也说明：若没有先基于四族 constraint-line geometry 优化 cutwidth，通用二维
wavefront 不会自然优于 Sec. VI 的逐行 contraction。下一方向应回到生产 row operator，
针对 materialization/hash cost 做有 gate 的后端实验，或设计明确最小化四族活跃线数的
ordering，而不是继续枚举朴素几何顺序。

原始数据：`experiments/e8_ordering_oracle/results.csv`。

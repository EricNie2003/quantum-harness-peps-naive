# E11：由显式 C 推导的稀疏合法位置迭代器

## 预注册与 revision

- 分支：`codex/exp-sparse-position-iterator`；
- worktree：`.worktrees/e11-sparse-position-iterator`；
- baseline commit：`5f3c36639871d0f49a87164c7a260e4f22ee0284`；
- candidate code commit：`03d15204b59dbed7949cf3d45744478588a08e79`；
- 假设：由显式 17-entry `C` 的唯一 occupied entry 推导位置 bitset，只枚举
  column、down-right 和 down-left 三个 incoming signal 都匹配的位置，可以显著减少局域
  entry/position 检查，并在 sort-reduce 与 parallel 后端上转化成端到端收益；
- keep gate：N=13--15 单线程至少 1.5x，且 support/RSS 不恶化；理想目标为 3x；
- kill gate：检查量明显下降但 runtime 小于 1.5x，表明排序、写流量或归并已占主导。

## PEPS 忠实性与 exactness

这不是手写 N-Queens recurrence。`CompiledRowOperator::compile` 仍逐项扫描显式
rank-8、17-entry counting tensor `C`，并验证 16 个 empty identity pass-through 和唯一
occupied entry。稀疏迭代器从该 occupied entry 的 `column_in`、`diag_dr_in` 和
`diag_dl_in` 值机械生成匹配 bitset；successor 的 outgoing signals 和 coefficient 也来自
同一 tensor entry。

新增验证包括：

- 对 N=1--8 的每个可达 parent state，逐项比较 sitewise 显式收缩、dense compiled
  transfer 和 sparse iterator 的完整 successor multiset；
- N=0--10 逐层比较 dense/sparse hash 与 sort-reduce 的 count、support、weight 和 work；
- N=0--10 比较 1/2/4 线程 sparse parallel 与 serial sparse；
- 检查 sparse `operator_candidates == operator_matched`。

原有 B/C 各 17-entry、局域 truth table、v0/v1/v2、独立 oracle 和 known Q(N) 测试保持
不变。release tests 共 18 项通过；`cargo fmt --check` 与
`cargo clippy --release --all-targets -- -D warnings` 通过。系数继续使用 checked `u128`，
不使用浮点、截断或舍入。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- OS：Windows；
- 编译器：`rustc 1.94.0 (4a4ef493e 2026-03-02)`，MSVC target，LLVM 21.1.8；
- build：Cargo release，thin LTO；
- 命令：
  - `cargo run --release --bin e11_sparse_iterator -- dense-hash 1 12 14 3`
  - `cargo run --release --bin e11_sparse_iterator -- sparse-hash 1 12 14 3`
  - `cargo run --release --bin e11_sparse_iterator -- dense-sort 1 13 15 3`
  - `cargo run --release --bin e11_sparse_iterator -- sparse-sort 1 13 15 5`
  - `cargo run --release --bin e11_sparse_iterator -- dense-parallel 16 13 15 5`
  - `cargo run --release --bin e11_sparse_iterator -- sparse-parallel 16 13 15 5`
- 重复策略：同一 backend/N 重复 3 或 5 次，报告中位数与最小值；每个 backend 独立进程；
- 内存：Windows `GetProcessMemoryInfo` 的 `PeakWorkingSetSize`。这是进程历史峰值，包含
  allocator、运行时和前序 repeat 保留的页面，不等同于 live tensor payload；因此只在相同
  命令结构和独立进程之间比较。

## 结果

### 单线程 sort-reduce（当前最快串行后端）

| N | dense (s) | sparse (s) | runtime speedup | dense checks | sparse checks | check reduction | sparse RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 13 | 0.459743 | 0.323348 | 1.42x | 47,909,758 | 4,201,149 | 11.40x | 101.8 |
| 14 | 2.742460 | 1.935670 | 1.42x | 286,010,088 | 23,859,616 | 11.99x | 443.5 |
| 15 | 16.906128 | 12.286512 | 1.38x | 1,783,273,650 | 143,138,637 | 12.46x | 2,883.5 |

每档 count、peak support 和 accepted entry 数均与 dense 完全一致。N=15 的 peak support
仍是 32,120,057；RSS 基本不变。

### HashMap 消融

| N | dense hash (s) | sparse hash (s) | speedup |
|---:|---:|---:|---:|
| 12 | 0.114895 | 0.086761 | 1.32x |
| 13 | 0.765186 | 0.594950 | 1.29x |
| 14 | 5.643898 | 4.648314 | 1.21x |

收益随 N 增长而下降，说明 HashMap probing、分配和内存流量也淹没了位置扫描成本。

### 16-thread 交互

| N | dense 16t (s) | sparse 16t (s) | speedup | sparse RSS (MiB) |
|---:|---:|---:|---:|---:|
| 13 | 0.113051 | 0.101730 | 1.11x | 91.8 |
| 14 | 0.617891 | 0.552422 | 1.12x | 388.6 |
| 15 | 3.725102 | 3.566500 | 1.04x | 2,167.0 |

并行后端几乎完全受 candidate materialization、排序和 merge 约束；降低局域 predicate
检查没有带来同比例收益。

## 机制判断与决策

**REJECT，E11 不单独合并 baseline；保留为诊断 primitive，允许在 E12 的交互消融中重测。**

检查量降低 11.4--12.5x，却只得到 1.38--1.42x 的最佳串行提升、1.04--1.12x 的并行
提升，未达到预注册 1.5x keep gate。结果证明“稀疏性”必须降低 materialized support、
候选写入或 merge 工作，而不能只稀疏化已经很便宜的局域匹配。

这也解释了为何不能因为理论 operation count 大幅下降就宣称方向成功。E12 应优先用
D4 切面稳定子的轨道代表真正减少进入 sort/reduce 的 state 数，并做四项消融：
dense、E11-only、D4-only、E11+D4。若 D4 将候选流量约减半，E11 的剩余扫描占比可能
变化，因此允许重测交互，但不得据此倒改本实验的拒绝结论。

原始数据：

- `experiments/e11_sparse_position_iterator/results.csv`
- `benchmarks/e11_sparse_position_iterator_release.csv`

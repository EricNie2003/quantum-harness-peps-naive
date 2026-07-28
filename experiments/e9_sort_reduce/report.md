# E9：exact sort-reduce layer materialization

## 预注册

- 分支：`codex/exp-sort-reduce`
- baseline commit：`265059cc5fb7ee1657cc3795b5df3a397418ebdb`
- candidate code commit：`2985bfc2cf74fd489b8e0985615969fd2194924b`；
- 假设：即使重层 merge ratio 仅约 1.08–1.21，连续 `Vec` 写入、`u128` key 排序和
  原地 checked-add reduce 仍可能比标准 `HashMap` 的随机访问更快、更紧凑；
- 小规模 gate：N=10–13 若连续两档没有至少 10% runtime 改善即停止；
- keep：N=13、14 均至少快 20%，RSS 不恶化，且 exact support/operator work 完全一致。

## PEPS / exactness 约束

候选不改变 tensor contraction 的转移：

1. `CompiledRowOperator::compile` 仍扫描显式 17-entry `C`，验证 16 个 identity
   pass-through 和唯一 occupied entry；
2. 每个 candidate 仍由 `contract_one_row_compiled` 生成；
3. 唯一变化是层输出从 `HashMap<PackedBoundary,u128>` 改成
   `Vec<(PackedBoundary,u128)>`，按完整 packed virtual-boundary key 排序，再对相邻同 key
   coefficient 做 `checked_add`；
4. bottom column `v1` 和 diagonal `v2` 的最终 contraction 不变；
5. 使用 `u128` exact coefficient，并检查乘法、归并和总和溢出。

新增测试在 N=0–10 逐层核对两个后端的 count、peak support、input/output support、
completed terms、output weight、row candidates 和 accepted terms。原有 B/C 17-entry、
local truth table、boundary、sitewise equivalence、独立 brute force 和 known Q gates 全部保留。
release tests 16 项、Clippy `-D warnings` 和格式检查均通过。

## Benchmark

- CPU：AMD Ryzen 9 7945HX；
- OS：Windows；
- 编译器：Rust 1.94，release/thin-LTO；
- 线程数：1；
- 命令：
  - `cargo run --release --bin sort_reduce -- hash 10 13 5`
  - `cargo run --release --bin sort_reduce -- sort-reduce 10 13 5`
  - `cargo run --release --bin sort_reduce -- hash 14 14 5`
  - `cargo run --release --bin sort_reduce -- sort-reduce 14 14 5`
- 重复策略：每个 backend/N 五次，报告中位数和最小值；同一进程内按 N 递增；
- 内存：Windows `GetProcessMemoryInfo` 的 `PeakWorkingSetSize`。它是整个进程至采样点的
  历史峰值，包含 allocator、运行时和前序 repeat 的 retained pages，不等同于容器 payload；
  hash/sort 分别在独立进程运行以避免两后端互相污染峰值。

| N | hash median (s) | sort median (s) | speedup | hash RSS (MiB) | sort RSS (MiB) | peak support |
|---:|---:|---:|---:|---:|---:|---:|
| 10 | 0.004926 | 0.003407 | 1.45x | 6.68 | 6.32 | 8,838 |
| 11 | 0.021051 | 0.016333 | 1.29x | 11.37 | 9.72 | 39,307 |
| 12 | 0.113696 | 0.081879 | 1.39x | 26.82 | 21.02 | 188,100 |
| 13 | 0.868104 | 0.457664 | 1.90x | 138.23 | 102.48 | 978,362 |
| 14 | 6.038000 | 3.085015 | 1.96x | 666.17 | 444.28 | 5,479,934 |

每个 N 的 count、support、17 次 tensor compile examinations/accepts、row candidate 和
accepted 数完全相同。N=14 两后端均为 286,010,088 candidates、23,859,616 accepted。

低 merge ratio 没有阻止收益，因为主要机制并不是减少 state 数，而是：

- candidate append 和 reduce 是连续内存访问；
- packed key 排序避免每个近乎唯一 state 都做 HashMap probing/桶管理；
- vector entry 比标准 HashMap bucket 更紧凑。

## 与 DFS 的差距

同硬件 N=14：

- E9 sort-reduce PEPS：3.085015 s；
- single-thread DFS：0.0703587 s；
- 16-thread DFS：0.0057339 s。

候选仍分别慢约 43.8x 和 538.0x。E9 将 E6 的 PEPS/DFS gap 再缩小约一半，但没有降低
5,479,934 的 support，所以仍不能解决渐近扩展问题或声称超过 DFS。

## 决策

**KEEP，合并 baseline。**

N=13、14 均远超 20% runtime gate，N=14 RSS 下降约 33.3%，所有 exact/tensor-level
等价检查通过。下一方向 E10 可以在这个连续 candidate representation 上评估 slicing/
parallel expansion，但必须同时报告单线程 work efficiency；不能用更多线程掩盖仍存在的
算法差距。

原始数据：`experiments/e9_sort_reduce/results.csv`。

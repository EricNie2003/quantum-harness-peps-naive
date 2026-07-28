# E6：从显式 C 机械编译 exact row operator

## 预注册

- 分支：`codex/exp-compiled-row-operator`
- review commit：`dc793ce6940d160cb1b2f190fdab2351c1c9f5a9`
- baseline code commit：`96535e63a3feb69a6db2c129100134addd6c288a`
- candidate code commit：`97dab3e6add204d6f2054b08777d7f8c70f8572c`
- 假设：把逐格点 \(C\) contraction partial-evaluate 成整行 operator，可去掉重复的
  horizontal partial automaton；
- keep：N=12、13 至少 2x，或 N=14 局域 runtime work 至少降低 10x；
- 限制：support 不变时只算常数收益，不宣称解决 DFS gap。

## PEPS / exactness 约束

候选没有手写一个无来源的 `available_columns` recurrence。`CompiledRowOperator::compile`
遍历实际 `SiteTensorC.entries()`，并且只有在以下结构全部成立时才成功：

1. 16 个 incoming signatures 各有唯一的 identity pass-through `CEntry`；
2. 唯一另一个 `CEntry` 是四通道同时 \(0\to1\)；
3. 全部 coefficient 为 exact integer 1；
4. 缺项、重复项或其他 transition 都使编译失败。

runtime 从编译得到的 occupied `CEntry` 读取 incoming requirements、outgoing signals 和
coefficient，再按棋盘 geometry 连接到下一行。原 `contract_one_row_sitewise` 被完整保留为
reference backend。

新增 correctness gates：

- N=1–8 的每一个可达 parent boundary，compiled 与 sitewise 的完整 outgoing
  `BoundaryState -> coefficient` map 逐项一致；
- N=0–10 的完整 compiled/sitewise contractions 一致；
- 原有 B/C truth table、独立 brute-force、known Q(N)、packed-key 测试全部通过。

总计 15 个 release tests 和 Clippy 通过。

## 指标语义

compiled backend 把两类工作分开报告：

- `tensor_entries_examined/matched=17`：构造 operator 时一次性验证的全部 C entries；
- `row_operator_candidates/matched`：runtime 检查的整行候选 term 和合法 term。

sitewise baseline 的 row-operator 字段为 0，tensor-entry 字段保持原语义。

## Benchmark

环境与前五个实验相同：AMD Ryzen 9 7945HX、单线程、Rust 1.94 release/thin-LTO、
Windows `PeakWorkingSetSize`。候选 N=10–14 各 5 repeats。

| N | sitewise (s) | compiled (s) | speedup | compiled RSS (MiB) | peak support |
|---:|---:|---:|---:|---:|---:|
| 10 | 0.008197 | 0.004330 | 1.89x | 6.73 | 8,838 |
| 11 | 0.043204 | 0.020335 | 2.12x | 11.36 | 39,307 |
| 12 | 0.245592 | 0.112681 | 2.18x | 26.80 | 188,100 |
| 13 | 1.741573 | 0.823089 | 2.12x | 138.24 | 978,362 |
| 14 | 11.089654 | 5.853217 | 1.89x | 666.16 | 5,479,934 |

所有 count、peak support 和最终 boundary coefficients 不变。RSS 不变，因为两个版本仍物化
相同的 input/output hash maps。

N=14 runtime work：

- sitewise matched C steps：464,957,208；
- compiled row candidates：286,010,088；
- compiled legal row terms：23,859,616。

## 与 DFS 的新差距

同硬件 N=14：

- compiled PEPS：5.853217 s；
- single-thread DFS：0.0703587 s；
- 16-thread DFS：0.0057339 s。

compiled PEPS 仍比单线程 DFS 慢约 83.2x，比 16-thread DFS 慢约 1020.9x。E6 大致把
PEPS/DFS gap 减半，但验证了 five-direction review 的判断：只消除局域 operator 开销仍不足，
主要问题是显式 boundary support 与 HashMap materialization。

## 决策

**KEEP。**

- N=11–13 超过 2x，N=14 为 1.89x；
- 全部 tensor-level 等价测试通过；
- exactness、support 和 RSS 不变；
- 对下一结构方向提供更快、更清晰的 row-transfer baseline。

下一方向应按 review 执行 exact decision-diagram/support compression 原型，而不是继续微调
row operator。

原始数据：`experiments/e6_compiled_row_operator/results.csv`。

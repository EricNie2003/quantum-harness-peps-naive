# E5a：复用逐行局域 contraction 的 partial buffers

## 预注册

- 分支：`codex/exp-partial-buffer-reuse`
- baseline code commit：`dde47b7fad35e6d2cbd94422d2f70559ca80d883`
- candidate code commit：`96535e63a3feb69a6db2c129100134addd6c288a`
- 单变量：`contract_one_row` 从每个格点新建 `Vec<PartialRow>`，改成两个预分配 Vec
  在格点间 `clear + swap`；
- 假设：消除每个父 boundary、每个 site 的 heap allocation；
- keep：两个连续 N 的中位时间改善至少 15%；
- kill：N=11、12 均未达到 15%。

## PEPS 合规性

输入、输出和局域计算均未改变。每个 site 仍：

1. 从显式 \(C\) 自动生成的 incoming-signature 桶取得 `CEntry`；
2. 读取完整 outgoing virtual legs 和 exact coefficient；
3. 把每个匹配项写入下一 partial buffer。

唯一变化是 next buffer 的存储被复用。`drain(..)` 逐项消费当前 buffer，
`std::mem::swap` 交换两个 Vec。没有合并、删除或手写任何 queen transition。

全部 9 个 release 测试和 Clippy 通过；N=10–14 的 count、peak support 和 tensor work
与 E3 baseline 完全相同。

## Benchmark

| N | baseline median (s) | buffer reuse (s) | speedup | baseline RSS (MiB) | candidate RSS (MiB) |
|---:|---:|---:|---:|---:|---:|
| 10 | 0.019144 | 0.008197 | 2.34x | 6.71 | 6.45 |
| 11 | 0.077946 | 0.043204 | 1.80x | 11.14 | 11.26 |
| 12 | 0.483586 | 0.245592 | 1.97x | 26.86 | 26.84 |
| 13 | 3.103225 | 1.741573 | 1.78x | 137.97 | 138.15 |
| 14 | 17.611201 | 11.089654 | 1.59x | 666.17 | 666.17 |

收益随 N 增大略有下降，因为大规模时 boundary HashMap 占比提高，但 N=14 仍快 1.59 倍。
RSS 没有实质变化；partial buffers 只有 \(O(N)\) 个小元素，峰值内存由 boundary maps 决定。

## 决策

**KEEP。**

- 五个规模均超过 15% 时间改善；
- N=10–12 达到约 1.8–2.3x；
- N=14 仍达到 1.59x；
- exact count、support 和 tensor work 完全不变；
- 改动只有 7 insertions / 5 deletions，归因明确。

该 candidate 合入新 baseline。完整 row-operator fusion 仍需单独实验，不能把本结果解释为已经
验证手写 queen recurrence。

原始数据：`experiments/e5a_partial_buffer_reuse/results.csv`。

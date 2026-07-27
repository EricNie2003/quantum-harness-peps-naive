# E2：用 input support 预留下一层 HashMap 容量

## 预注册

- 分支：`codex/exp-hash-capacity`
- baseline：E1 KEEP 后端，code commit
  `20b5334f55819ab0b4bdce7aa701527de736c3dc`
- candidate code commit：`c3768424bfcf3f7cbcf9b4f6ad04277a9e68de73`
- 单变量：`HashMap::new()` 改为 `HashMap::with_capacity(input_states)`；
- 假设：减少从零开始的 rehash；
- 风险：下降阶段过度预留，增加 RSS；
- keep：时间稳定改善至少 10%，或两个连续 N 的 RSS 降低至少 15%；
- kill：时间改善不足且 RSS 收益不能连续保持。

## 合规性与正确性

该改动只改变保存 outgoing boundary tensor 的标准库容器初始容量。局域 \(B/C\)、输入索引、
virtual boundary key、系数、边界向量和 ordering 均不变。

全部 8 个 release 测试和 Clippy 通过；N=10–13 的 exact count、peak support、
`tensor_entries_examined` 和 `tensor_entries_matched` 与 baseline 完全相同。

## Benchmark

环境、编译器、单线程设置和 RSS 测量方式与 E1 相同。N=10–12 各 3 次；N=13 为控制系统
抖动，在两个 worktree 中同期各跑 5 次。

| N | baseline (s) | candidate (s) | 时间改善 | baseline RSS (MiB) | candidate RSS (MiB) | RSS 改善 |
|---:|---:|---:|---:|---:|---:|---:|
| 10 | 0.016569 | 0.015397 | 7.1% | 7.46 | 6.70 | 10.2% |
| 11 | 0.082916 | 0.080700 | 2.7% | 14.37 | 12.86 | 10.5% |
| 12 | 0.489594 | 0.478415 | 2.3% | 37.35 | 31.23 | 16.4% |
| 13 | 3.221072 | 3.113165 | 3.4% | 202.52 | 202.74 | -0.1% |

N=12 的 RSS 收益没有在 N=13 延续；大规模下最终 HashMap 的自然容量与预留版本接近，
主内存仍由同时存活的 input/output maps 决定。时间收益始终小于 10%。

## 决策

**REJECT。**

该策略没有在两个连续规模上达到时间或 RSS threshold。根据 kill condition 不运行 N=14，
不把候选合入主 baseline。分支和原始结果保留，避免之后重复尝试同一容量启发式。

原始数据：`experiments/e2_hash_capacity/results.csv`。

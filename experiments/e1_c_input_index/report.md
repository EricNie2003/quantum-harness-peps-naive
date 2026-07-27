# E1：按 incoming virtual signature 索引局域张量 C

## 预注册

- 实验分支：`codex/exp-c-input-index`
- baseline commit：`f51f3a3`
- candidate code commit：`20b5334f55819ab0b4bdce7aa701527de736c3dc`
- 单变量：把每次对 `C.entries()` 的 17 项线性过滤，改为从显式 `C` 自动生成的
  16 个 incoming-signature 桶；
- 不改变：局域 \(B/C\) 定义、边界方向、边界状态、HashMap、算术、ordering、线程数；
- 假设：减少局域非零元检查数，使 N=11、12 中位时间至少下降 15%；
- kill condition：N=11、12 的改善均小于 15%；
- correctness oracle：局域桶与 `C.entries()` 逐项集合比较、独立 brute-force oracle、
  A000170 已知值。

## PEPS 合规性

`entries_by_input` 在 `SiteTensorC::from_b` 内创建；它遍历由
\(C=\sum_\alpha B^\alpha\) 得到的全部显式 `CEntry`，按四个 incoming virtual bits
机械分桶。运行 contraction 时仍消费原始 `CEntry` 的全部 outgoing legs 和 coefficient。

新增测试枚举全部 16 个 incoming signatures，把索引桶与对 `C.entries()` 线性过滤的结果
逐项比较。测试通过，因而本实验只是稀疏张量 lookup layout 的变化，不是 queen-placement
recurrence。

## 环境与命令

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- 内存：34,024,747,008 bytes；
- Rust：1.94.0，`x86_64-pc-windows-msvc`，release/thin-LTO；
- 线程数：1；
- RSS：Windows `GetProcessMemoryInfo/PeakWorkingSetSize`；
- N=10–12、14：3 repeats；N=13 因首轮系统抖动，baseline/candidate 都改用 5 repeats。

```powershell
cargo test --release
cargo clippy --release --all-targets -- -D warnings
cargo run --release -- bench 12 --min 10 --repeats 3 --csv
cargo run --release -- bench 13 --min 13 --repeats 5 --csv
cargo run --release -- bench 14 --min 14 --repeats 3 --csv
```

## 结果

| N | baseline median (s) | indexed median (s) | speedup | baseline RSS (MiB) | indexed RSS (MiB) |
|---:|---:|---:|---:|---:|---:|
| 10 | 0.021062 | 0.016569 | 1.27x | 7.47 | 7.46 |
| 11 | 0.110500 | 0.082916 | 1.33x | 14.37 | 14.37 |
| 12 | 0.575259 | 0.489594 | 1.17x | 37.30 | 37.35 |
| 13 | 4.264959 | 3.408420 | 1.25x | 202.71 | 202.52 |
| 14 | 25.191505 | 19.308394 | 1.30x | 986.34 | 986.67 |

所有 count 都与已知值一致；peak support 在两个版本间逐 N 完全相同。

N=14 的 `tensor_entries_examined` 从 7,498,659,064 降为 464,957,208，减少
93.80%。`tensor_entries_matched` 保持 464,957,208 不变。候选版本的 examined 等于
matched，是因为 signature lookup 返回的桶内每项都已经匹配 incoming virtual legs。

RSS 没有实质变化，符合预期：每个 `SiteTensorC` 只多保存 17 个小条目的分桶副本，主内存仍由
数百万 boundary hash entries 主导。

## 决策

**KEEP。**

- N=10–14 全部精确；
- N=10、11、13、14 的中位时间改善均超过 15%；
- N=14 加速 1.30x；
- support 和 RSS 基本不变；
- 变换由显式 \(C\) 自动生成并有全 signature 等价测试。

该收益是常数收益，不改变 support 的指数增长。下一实验 E2 应以本 candidate 为新 baseline，
单独测试 HashMap 容量策略。

原始数据：`experiments/e1_c_input_index/results.csv`。

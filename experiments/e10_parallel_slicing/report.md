# E10：exact boundary slicing + parallel sort-reduce

## 预注册

- 分支：`codex/exp-parallel-slicing`
- baseline commit：`5b12cbda3982db8935d5a6b8159a6a4d07a5c3df`
- candidate code commit：`4e3fa7649356201ef8b51c21d74196c29a7e0779`
- 假设：E9 的连续 candidate representation 可按 parent-boundary slices 并行 expansion，
  再用并行 sort 提升吞吐；
- keep：N=13、14 的 8/16-thread 至少 2x，RSS 不超过串行 1.5x，exact work/support 不变；
- 限制：并行收益不能解释为算法复杂度或 support 的改善，必须与同线程 DFS 分开比较。

## PEPS / exactness

每层执行：

1. 将 E9 的 exact、unique `PackedBoundary` vector 切成约 `4 * threads` 个 slices；
2. 每个 worker 对 slice 中每个 parent 调用相同的 `contract_one_row_compiled`。该 operator
   仍由显式 17-entry `C` 机械编译并 fail-closed；
3. 合并 worker-local candidate vectors；
4. Rayon `par_sort_unstable_by_key` 按完整 virtual-boundary key 排序；
5. 单线程、确定顺序用 `checked_add` exact reduce；
6. 最终 column `v1`、diagonal `v2` contraction 不变。

新增 correctness gate 在 threads=1、2、4，N=0–10 上逐层比较 parallel 与 serial
sort-reduce 的 count、support、completed terms、output weight、candidate 和 accepted
work。全仓库 17 个 release tests、Clippy `-D warnings`、格式检查通过。coefficient 继续使用
checked `u128`；没有浮点、SVD、truncation 或 rounding。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical threads；
- OS：Windows；
- 编译器：Rust 1.94，release/thin-LTO；Rayon 1.12.0；
- PEPS 命令：
  - `cargo run --release --bin parallel_slicing -- 1 12 14 5`
  - 同命令分别以 `8`、`16`、`32` threads 运行；
- DFS comparator：`dfs_bitmask bench 14 --min 14 --threads T --repeats 9 --warmup 1 --csv`，
  T=16、32，顺序无竞争运行；
- 重复：PEPS 每点 5 次取中位数/min；DFS warmup 1 次后 9 次取中位数/min；
- RSS：Windows `PeakWorkingSetSize`，包含进程、allocator、Rayon worker stacks 和 retained
  pages；每个线程配置独立进程，适合比较整进程峰值，但不等于 tensor payload。

## PEPS 结果

| N | threads | median (s) | min (s) | speedup vs 1t | RSS (MiB) | peak support |
|---:|---:|---:|---:|---:|---:|---:|
| 12 | 1 | 0.090255 | 0.088089 | 1.00x | 20.71 | 188,100 |
| 12 | 8 | 0.027684 | 0.026098 | 3.26x | 27.34 | 188,100 |
| 12 | 16 | 0.026349 | 0.024647 | 3.43x | 29.93 | 188,100 |
| 12 | 32 | 0.026530 | 0.025790 | 3.40x | 32.09 | 188,100 |
| 13 | 1 | 0.468509 | 0.463875 | 1.00x | 102.17 | 978,362 |
| 13 | 8 | 0.127353 | 0.125342 | 3.68x | 82.40 | 978,362 |
| 13 | 16 | 0.114947 | 0.111997 | 4.08x | 94.01 | 978,362 |
| 13 | 32 | 0.116107 | 0.113085 | 4.04x | 101.70 | 978,362 |
| 14 | 1 | 3.155874 | 3.009632 | 1.00x | 444.36 | 5,479,934 |
| 14 | 8 | 0.717977 | 0.677335 | 4.40x | 386.89 | 5,479,934 |
| 14 | 16 | 0.624911 | 0.597097 | 5.05x | 388.78 | 5,479,934 |
| 14 | 32 | 0.604584 | 0.574414 | 5.22x | 421.90 | 5,479,934 |

每个线程配置的 N=14 count=365,596，17/17 tensor compile examinations/accepts、
286,010,088 row candidates、23,859,616 accepted，以及 peak support 完全一致。

## 与 DFS 的同线程比较

| N | threads | PEPS median (s) | DFS median (s) | PEPS/DFS |
|---:|---:|---:|---:|---:|
| 14 | 16 | 0.624911 | 0.006306 | 99.1x |
| 14 | 32 | 0.604584 | 0.004885 | 123.8x |

32 threads 只比 16 threads 再快 3.4%，显示 memory traffic、候选拼接和串行 reduce 已接近
饱和。并行把 wall time 降低 5.22x，但没有改变 support 或总 candidate work；同线程 DFS
gap 反而比与 single-thread comparator 更能揭示算法效率差异。

## 决策

**KEEP 作为 exact throughput backend。**

N=13、14 超过 2x gate，RSS 始终低于串行的 1.5x，全部 tensor/exactness gates 通过。
默认推荐 16 threads：32 threads 的额外收益只有 3.4% 且 RSS 更高。

但 E10 **没有达到超过 DFS 的目标**。主要限制仍是显式 materialization 的 5,479,934
boundary states 和 286,010,088 row-candidate examinations。根据强制 review gate，到此停止
启动新实验，先完成前十方向复盘并推翻/更新后续优先级。

原始数据：

- `experiments/e10_parallel_slicing/results.csv`
- `experiments/e10_parallel_slicing/dfs_comparator.csv`

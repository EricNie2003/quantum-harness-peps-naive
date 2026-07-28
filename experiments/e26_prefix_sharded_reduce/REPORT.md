# E26：key-sharded exact sparse sort-reduce

## 决策

**KEEP fixed 256-prefix shards。**

相对 E24 的同 revision 全局排序，E26 在 N=14/15：

- 1 thread：0.71181/4.82235 s → 0.51378/3.63108 s，
  快 27.8%/24.7%；
- 8 threads：0.34198/2.75489 s → 0.25820/1.85859 s，
  快 24.5%/32.5%；
- N=15 peak RSS：1,712,611,328 → 1,256,726,528 bytes，
  降 26.6%。

计数、peak support、local tensor entries 和 accepted transitions
完全一致，达到预注册的两档 runtime `>=20%` keep gate。

## 实现与 PEPS fidelity

- code revision：`c8beb0e`；
- branch：`codex/exp-prefix-sharded-reduce`；
- worktree：`.worktrees/e26-prefix-sharded-reduce`；
- base：main `c17fc80`（包含 E24 KEEP kernel）。

每个 successor 仍由 `SiteTensorC::sec_vi()` 的显式 17-entry `C`
编译关系产生。稀疏位置是 column/dr/dl 三个 incoming virtual signals
匹配 occupied `C` entry 的机械位集交；空分支、`v0/v1/v2`、对角线
shift 与首行 vertical-reflection D4 orbit 均未改变。

E26 只把 packed virtual-boundary key 分到互不重叠的 buckets。每个
bucket 独立执行 exact `sort_unstable_by_key` 和 checked `u128`
reduce；所有 buckets 的并集恰为 E24 的完整 candidate multiset。
boundary 在下一行仍保持 sharded，不需要重新拼成一个全局 vector。

`Prefix` 使用 packed key 的高位 prefix；`Mixed` 使用确定性的全 key
mix，仅用于 balance 消融，不参与等价判断。两者都不是概率去重：
同 key 必定进入同 bucket，最终仍做完整 key equality。

## 正确性

`cargo test --release`：33 passed。新增测试在 N=0--10、Prefix/Mixed、
1/8 shards 上逐项比较既有 C-derived sparse contraction 的：

- exact Q(N)；
- peak sparse support；
- row-operator candidates；
- row-operator matched。

原有测试继续验证 B/C 各 17 nnz、truth tables、`v0/v1/v2`、D4、
sitewise-vs-compiled C contraction、独立 oracle 和已知 counts。
`cargo clippy --all-targets -- -D warnings` 通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，Windows MSVC；
- `rustc 1.94.0`，release/thin-LTO；
- `RAYON_NUM_THREADS=1` 或 `8`；
- 每点 3 次，无预热，报告中位数和最小值；
- RSS 为 Windows `GetProcessMemoryInfo/PeakWorkingSetSize` 的进程
  high-water mark；包含 allocator 保留页和 Rayon stacks，不等于
  单个 row 的精确 live tensor bytes；
- E24 comparator 来自同一代码 revision 中已归档的
  `benchmarks/e24_parallel_sort_8t_release.csv` 和
  `benchmarks/e24_sparse_release.csv`，没有跨算法复用为 E26 数据。

命令：

```powershell
$env:RAYON_NUM_THREADS='8'
cargo run --release --bin e26_sharded -- prefix 8 14 14 3
cargo run --release --bin e26_sharded -- prefix 32 14 14 3
cargo run --release --bin e26_sharded -- prefix 128 14 15 3
cargo run --release --bin e26_sharded -- prefix 256 14 15 3
cargo run --release --bin e26_sharded -- mixed 8 14 15 3
cargo run --release --bin e26_sharded -- mixed 16 14 15 3
cargo run --release --bin e26_sharded -- mixed 32 14 14 3
cargo run --release --bin e26_sharded -- mixed 128 14 14 3
$env:RAYON_NUM_THREADS='1'
cargo run --release --bin e26_sharded -- prefix 256 14 15 3
```

## Bucket 消融

N=14、8 threads：

| mode/shards | wall s | peak RSS |
|:---|---:|---:|
| Prefix/8 | 0.56344 | 252,260,352 |
| Prefix/32 | 0.33686 | 205,422,592 |
| Prefix/128 | 0.27424 | 201,797,632 |
| Prefix/256 | **0.25820** | 206,209,024 |
| Mixed/8 | 0.28436 | **198,324,224** |
| Mixed/16 | 0.26830 | 199,475,200 |
| Mixed/32 | 0.28465 | 200,265,728 |
| Mixed/128 | 0.31525 | 207,777,792 |

Prefix/8 很慢，说明少量高位 diagonal prefix 严重不平衡；增加到
128/256 后 bucket 变小，局部 sort/reduce 的 cache locality 才超过
imbalance。Mixed 在 8/16 buckets 已平衡，但 hashing 与写入分散成本使
它在 N=15 仍慢于 Prefix/256。

## 选中配置与 DFS 差距

| N | E24 PEPS 1t | E26 PEPS 1t | E24 PEPS 8t | E26 PEPS 8t | DFS 8t |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.71181 | 0.51378 | 0.34198 | **0.25820** | 0.009912 |
| 15 | 4.82235 | 3.63108 | 2.75489 | **1.85859** | 0.06609 |

E26 把 N=15 的 8-thread DFS gap 从 41.68x 缩至 28.12x，但仍远未
超过 DFS。单线程 gap 从 9.66x 缩至 7.27x。

收益机制不是减少 PEPS work：N=15 仍有 18,178,233 peak states 和
80,077,350 accepted C transitions。收益来自：

1. 把一个超大 O(M log M) sort 改为多个较小 sort；
2. bucket reduce 也能并行，而 E24 只有 sort 并行；
3. boundary 保持 sharded，避免 global concatenation；
4. 较小 bucket capacity 降低 allocator over-reservation/high-water RSS。

## 后续

E26 证明 flat frontier 仍有约 25--33% 的结构化内核收益，但没有改变
support/candidate 增长率。下一项 E27 应在这些 shards 内按 parent chunks
生成有界 sorted runs 并 exact k-way merge，目标是进一步限制 peak live
candidates；不能只继续搜索 bucket 常数。

Raw CSV：

- `benchmarks/e26_shard_grid_8t_release.csv`
- `benchmarks/e26_selected_release.csv`

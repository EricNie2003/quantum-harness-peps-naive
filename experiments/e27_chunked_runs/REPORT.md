# E27：parent-chunk sorted runs + exact k-way merge

## 决策

**REJECT。**

选中诊断配置为 256 prefix shards、1,048,576 parents/chunk。相对 E26：

| N | E26 time | E27 time | time change | E26 RSS | E27 RSS | RSS change |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.25820 s | 0.40207 s | +55.7% | 206,209,024 | 199,221,248 | -3.4% |
| 15 | 1.85859 s | 3.24815 s | +74.8% | 1,256,726,528 | 1,193,234,432 | -5.1% |

预注册 keep gate 要求 N=15 RSS 降 40% 且时间回退不超过 20%。
两项均未达到；继续减小 chunk 只增加 run 数和 heap 成本，因此停止。

## 实现与 exact PEPS fidelity

- code revision：`07f7b8d`；
- branch：`codex/exp-chunked-runs`；
- worktree：`.worktrees/e27-chunked-runs`；
- base：main `8bf5879`，即 E26 Prefix/256 KEEP baseline。

算法保留 E26 的显式 17-entry C compiled transition、sparse incoming
signal intersection、`v0/v1/v2`、对角线 shift、D4 首行 orbit 和 checked
`u128`。它不使用 DFS recurrence。

每次只处理固定数量 parent states，把产生的 successor 按 E26 prefix
shards 分区；每个 chunk/bucket 独立 sort 和 exact reduce，成为一个
sorted run。所有 parents 完成后，对每个 shard 用 `BinaryHeap` 做完整
k-way merge；相等 packed virtual keys 用 checked `u128` 相加。没有
丢弃、阈值、浮点或概率等价。

## 正确性

`cargo test --release`：34 passed。新增测试在 N=0--9、chunk size
1/7/1000 下比较 E26 sharded contraction 的：

- exact count；
- peak support；
- row-operator candidates/matched。

原有 B/C 17-nnz、truth table、boundary、D4、sitewise C 和独立 oracle
测试全部继续通过。Clippy `-D warnings` 通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，Windows MSVC，rustc 1.94.0；
- release/thin-LTO；`RAYON_NUM_THREADS=8`；
- 本实验是 preregistered kill diagnostic，每点 1 次、无预热；
- RSS 为 Windows `PeakWorkingSetSize`，包含 allocator high-water、
  Rayon stacks、run capacities 与 output capacities；
- E26 comparator 为同硬件、同线程数的
  `benchmarks/e26_selected_release.csv`。

命令：

```powershell
$env:RAYON_NUM_THREADS='8'
cargo run --release --bin e27_chunked_runs -- 256 16384 13 13 1
cargo run --release --bin e27_chunked_runs -- 256 65536 13 14 1
cargo run --release --bin e27_chunked_runs -- 256 262144 13 14 1
cargo run --release --bin e27_chunked_runs -- 256 1048576 14 15 1
```

## 机制分析

N=15 的 raw accepted candidates 为 80,077,350。E27 将测得的
`peak_live_candidates` 降到 37,072,187，表面上减少 53.7%；但
`peak_run_entries` 仍为 18,893,954，接近最终 peak support
18,178,233。k-way merge 生成 output 时，所有 input runs 仍必须存活，
所以 runs + output + bucket capacities 同时驻留，RSS 最终只降 5.1%。

同时 N=15 执行 148,116,850 次 heap push/pop。它把 E26 的连续局部
sort/reduce 变成大量 `O(log runs)`、分支密集且 cache-unfriendly 的
heap 操作，造成 74.8% 时间回退。

Chunk 消融也支持该判断：

| N | parents/chunk | time | RSS | max runs/shard |
|---:|---:|---:|---:|---:|
| 13 | 16,384 | 0.13135 | 47,779,840 | 12 |
| 13 | 65,536 | 0.10803 | 47,308,800 | 5 |
| 13 | 262,144 | 0.08554 | 46,223,360 | 3 |
| 14 | 65,536 | 0.53946 | 208,842,752 | 15 |
| 14 | 262,144 | 0.46915 | 201,863,168 | 5 |
| 14 | 1,048,576 | 0.40207 | 199,221,248 | 3 |

更小 chunk 没有显著降低 RSS，却单调增加时间。继续搜索 chunk 常数
不会通过 gate。

## 后续影响

E27 证明“in-memory retained runs + final k-way merge”不能解决 E26 的
内存瓶颈。真正有可能显著降低 RSS 的版本必须：

- 把 runs spill 到外存（预计严重损害 throughput）；或
- 在生成期间做 LSM/pairwise merge，避免 runs 与完整 output 同时存活；或
- 改变 state/value 布局，直接降低每个 live entry 的 bytes。

按计划下一项转 E28 row-aware compact key / SoA coefficients，不把
LSM merge 的实现细节伪装成新的 E27 参数搜索。

Raw data：`benchmarks/e27_chunk_grid_release.csv`。

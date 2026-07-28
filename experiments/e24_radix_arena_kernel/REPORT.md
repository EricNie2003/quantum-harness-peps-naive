# E24：sort-reduce 内核、稀疏迭代与并行排序

## 结论

**KEEP** arena 复用、融合批量生成、稀疏合法位置迭代和并行标准排序；
**REJECT** 当前自写 MSD radix sort；**REJECT** 16-byte deferred candidate 作为默认布局。

E24 是 E21--E24 中第一个明显改善生产 PEPS 路径的实验。单线程
`arena_batched_sparse` 在 N=12--15 相对原 D4 PEPS 基线快约
2.27--2.46 倍；固定 8 线程只并行排序时，N=13--15 相对原单线程
PEPS 基线快约 4.0--4.74 倍。

它仍没有超过 DFS。单线程稀疏 PEPS 在 N=12--15 慢 7.99、8.49、
9.23、9.66 倍；8 线程 PEPS 在 N=13--15 慢同为 8 线程的 DFS
30.69、34.50、41.68 倍。现有证据只能否定“当前平直逐行
sort-reduce 已接近 DFS”的判断，不能证明所有精确 PEPS 收缩都不可能超过 DFS。

## 假设与实现

代码 revision：`df05a11`；branch：`codex/exp-radix-arena-kernel`；
worktree：`.worktrees/e24-radix-arena-kernel`。父 revision 为 main
`e0b011e`。

收缩仍从 `SiteTensorC::sec_vi()` 的 17 个非零局域条目编译行关系，
逐行收缩 virtual bonds。空局域分支给出确定性信号传播，occupied
分支由显式 `C` 扫描得到；稀疏位置位集只是三个 incoming virtual
signals 的机械交集，不是 N-Queens DFS 递归。左/上端使用 `v0`，
行列末端使用 `v1`，两族对角线末端使用 `v2`。D4 部分沿用经证明
保持当前 row cut 的纵向镜像首行 orbit；其余六个 D4 作用不错误地
乘入中间 cut。

所有系数为 checked `u128`。本实验没有浮点、SVD、截断或取整。

消融机制如下：

1. `arena`：跨 row 复用两块 `Vec`；
2. `arena_batched`：直接把匹配的 `C`-derived successor 写入整层
   candidate arena，去掉每个 parent 的临时 `Vec`；
3. `arena_batched_sparse`：用 bitset 只枚举三个 incoming signals
   同时匹配 occupied `C` entry 的位置；
4. `arena_batched_radix`：在相同候选上使用原地 MSD byte radix；
5. `deferred_sparse`：candidate 保存 64-bit boundary key、parent
   index 和 D4 multiplicity，延后读取 exact `u128` 权重；
6. `arena_batched_sparse_parallel_sort`：候选生成和归并仍串行，仅用
   Rayon 的 `par_sort_unstable_by_key` 并行排序。

## 正确性

`cargo test --release`：31 passed。测试覆盖：

- rank-9 `B` 与 rank-8 `C` 都恰有 17 个非零条目；
- empty/occupied truth table 和 `v0/v1/v2` 边界；
- 全部 D4 作用保持显式 `B/C`，并验证只有 identity/vertical
  reflection 保持内部 top-down row cut；
- E24 dense 内核与原基线在 N=0--10 的 count、peak support、
  examined/matched work 完全一致；
- sparse、deferred 和 parallel-sort 与既有 C-derived sparse
  contraction 在 N=0--10 的 count、support 和 work 完全一致；
- 独立 brute-force oracle 与已知 Q(N)。

`cargo clippy --all-targets -- -D warnings` 通过。

## Benchmark 环境与方法

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- OS/target：Windows MSVC；
- compiler：`rustc 1.94.0 (4a4ef493e 2026-03-02)`，release/thin-LTO；
- PEPS 串行项 1 thread；parallel-sort 固定
  `RAYON_NUM_THREADS=8`；DFS 分别固定 1/8 threads；
- PEPS 每点 3 次、无预热，报告中位数和最小值；DFS 预热 1 次、
  正式 9 次；
- RSS 使用 Windows `GetProcessMemoryInfo` 的 `PeakWorkingSetSize`。
  这是进程生命周期 high-water mark，包含 allocator 保留页、Rayon
  worker stacks 与前一次 repetition 的保留容量，不能解释为某个
  row 的精确 live tensor bytes；不同 variant 由独立进程运行。

命令：

```powershell
cargo run --release --bin e24_kernel -- baseline 12 14 3
cargo run --release --bin e24_kernel -- arena 12 14 3
cargo run --release --bin e24_kernel -- batched 12 14 3
cargo run --release --bin e24_kernel -- radix 12 14 3
cargo run --release --bin e24_kernel -- batched-sparse 12 15 3
cargo run --release --bin e24_kernel -- deferred-sparse 12 15 3
$env:RAYON_NUM_THREADS='8'
cargo run --release --bin e24_kernel -- batched-sparse-parsort 12 15 3
cargo run --release --bin dfs_bitmask -- bench 15 --min 12 --threads 1 --repeats 9 --warmup 1 --csv
cargo run --release --bin dfs_bitmask -- bench 15 --min 13 --threads 8 --repeats 9 --warmup 1 --csv
```

## 结果

### 同工作量内核消融

| variant | N=12 s | N=13 s | N=14 s | N=14 RSS | support/work |
|---|---:|---:|---:|---:|---|
| baseline | 0.04542 | 0.27314 | 1.62198 | 237,236,224 | reference |
| arena | 0.04204 | 0.23655 | 1.43078 | 197,316,608 | identical |
| arena + batch | 0.02853 | 0.17534 | 0.98339 | 209,145,856 | identical |
| arena + batch + radix | 0.02976 | 0.18041 | 1.11885 | 209,207,296 | identical |

Arena 的收益来自减少 allocation/copy；融合 batch 的额外收益来自去掉
per-parent 临时向量和 iterator/map 层。自写 radix 在所有三个点都慢，
说明递归 bucket partition、差的局部性和小 bucket 回退成本超过比较
排序；该方向按 kill gate 拒绝。

### 稀疏性

| N | baseline s | batched sparse s | speedup | dense checks | sparse accepted | peak support |
|---:|---:|---:|---:|---:|---:|---:|
| 12 | 0.04542 | 0.01845 | 2.46x | 4,444,872 | 403,508 | 98,939 |
| 13 | 0.27314 | 0.12015 | 2.27x | 27,346,943 | 2,334,177 | 541,745 |
| 14 | 1.62198 | 0.71181 | 2.28x | 153,216,826 | 12,359,529 | 2,847,130 |
| 15 | 11.03459 | 4.82235 | 2.29x | 1,031,876,940 | 80,077,350 | 18,178,233 |

稀疏迭代把无效位置检查削减 11.0--12.9 倍，但总时间只降低约
2.3 倍，证明 N>=14 的剩余主成本已是 candidate 写入、排序、聚合
和大边界内存流量，而不是 local-entry predicate。

Deferred 16-byte candidate 在 N=14 为 0.62767 s，略快于普通 sparse，
但 N=15 中位数仅从 4.82235 降至 4.75003 s，同时 peak RSS 从
1,711,894,528 增至 1,846,771,712 bytes。原因是它必须在排序期间
同时保留 parent boundary 供延迟取权；拒绝为默认实现。

### 并行排序与 DFS

| N | PEPS 8t s | DFS 8t s | PEPS/DFS | PEPS RSS | DFS RSS |
|---:|---:|---:|---:|---:|---:|
| 13 | 0.06785 | 0.002211 | 30.69x | 58,331,136 | 5,554,176 |
| 14 | 0.34198 | 0.009912 | 34.50x | 209,657,856 | 5,578,752 |
| 15 | 2.75489 | 0.06609 | 41.68x | 1,712,611,328 | 5,820,416 |

并行排序相对 serial sparse 在 N=15 只有 1.75x，而不是接近 8x，
因为生成、归并仍串行且排序受内存带宽限制。DFS 的 compact bitmask
tree 几乎不保留整层 frontier；当前 PEPS 则在 N=15 保留
18,178,233 个 canonical boundary states，并排序约 80,077,350 个
accepted candidates。这是当前数量级差距的直接机制。

原始数据：

- `benchmarks/e24_kernel_ablation_release.csv`
- `benchmarks/e24_sparse_release.csv`
- `benchmarks/e24_parallel_sort_8t_release.csv`
- `benchmarks/e24_dfs_1t_release.csv`
- `benchmarks/e24_dfs_8t_release.csv`

## 决策与后续约束

E24 达到 keep gate：至少两个 N 的同语义运行时改善远超 20%，并且
count/support/work 由测试确认。后续生产候选应从
`arena_batched_sparse` 出发；吞吐运行可选 parallel sort。

然而，E24 同时给出更强的负面证据：只继续优化 flat frontier 的
排序常数不可能消除 N=15 的 9.66x 单线程差距和约 328x RSS 差距。
下一阶段必须减少 materialized support/candidates，例如分层分片后
即时局部归并、separator-conditioned subnetwork cache，或不展开全层
frontier 的 exact structured representation；不能把 DFS recurrence
冒充 PEPS。

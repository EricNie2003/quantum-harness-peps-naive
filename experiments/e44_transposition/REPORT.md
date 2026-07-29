# E44：bounded exact recursive transposition table

## 决策

**REJECT，不并入 production，并按 kill criterion 不运行 N=18。**

最佳配置为每 worker 16,384 slots、只缓存 remaining=5：

| N | E42 control | E44 cache | slowdown | hit rate | cache bytes |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.010774 s | 0.014702 s | 1.36x | 0.549% | 2.0 MiB |
| 15 | 0.061246 s | 0.089754 s | 1.47x | 0.579% | 2.0 MiB |
| 16 | 0.460113 s | 0.701533 s | 1.52x | 0.506% | 2.0 MiB |
| 17 | 3.421481 s | 5.295516 s | 1.55x | 0.433% | 2.0 MiB |

E44 要求 N=16--18 nodes/wall 至少下降 20%，实际 N=16、17 均退化
超过 50%。N=17 已有 738M lookups 但只命中 3.20M；按实测 scaling，
运行 N=18 只会昂贵地重复一个 hit-rate 已被上界否定的机制。

## 实现与 exactness / PEPS fidelity

- code revision：`0b80d3a`；
- branch/worktree：`codex/exp-transposition` /
  `.worktrees/e44-transposition`；
- base：main `b198821`（accepted E42 + rejected E43 artifacts）；
- arithmetic：checked u64；overflow 从相同 prefix sectors 用 generic-C
  checked u128 完整 replay。

每个 native worker 拥有固定容量、direct-mapped table，无锁且不跨 worker
共享。key 是完整的
`(remaining_rows, columns, diag_dr, diag_dl)`；N<=19 时 3N-bit packed
virtual boundary 左移 5 位再加入 remaining depth，最大 62 bits。slot
只有 key 完全相等才返回 value；hash collision 只替换旧 slot，绝不把
不同 state 合并。因此 cache collision 影响命中率，不影响 exactness。

value 是 E42 `CertifiedSecViTailPlan` 从 explicit 17-entry C 认证的
remaining-subnetwork contraction。remaining<=4 仍走 last-four
microkernel；其余逐次应用 occupied C transition。cache 不调用 DFS，
也不以 known count 填值。

测试验证 N=0..10 known counts、N=12 实际 cache hit、generic u128 replay
一致，并以 coefficient limit=1 强制 replay 得到 Q(8)=92。既有
explicit B/C、boundary vectors、reachable-parent C replay 测试保留。

## 容量/深度窗消融与失败机制

- remaining=5、4K→16K→64K：
  - N=15 hit rate 0.440%→0.575%→0.742%；
  - N=16 hit rate 0.379%→0.501%→0.663%。
- 64K/worker 需要 8 MiB table，但仍比 16K 更慢。
- 扩展到 remaining<=8 把 N=16 lookups 从 96.9M 增到 181.3M，hit rate
  反而降到 0.219%。
- 4K/remaining<=6 的 replacement 接近 insert 数量，但增加 16x 容量后
  hit 仍低于 1%，所以失败不只是 direct-map collision。

row-ordered contraction 中，一个 placement prefix 的后续 virtual
boundary 几乎唯一；不同 sectors 很少重新汇合。cache lookup/hash/write
发生在数千万到数亿个节点，省掉的 subtree 少于 1%，内存流量远大于
复用收益。这个方向的核心假设被否定。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；8 threads；shards=256；
- search：1 warmup + 5 samples；
- formal N=14--16：3 warmups + 21 samples；N=17：1 warmup + 3 samples；
- commands：
  - `cargo run --release --bin e44_transposition -- <slots> <max_remaining> 15 16 5 1`
  - `cargo run --release --bin e44_transposition -- 16384 5 14 16 21 3`
  - `cargo run --release --bin e44_transposition -- 16384 5 17 17 3 1`
  - `cargo run --release --bin e42_last_k -- 256 14 16 21 3 4`
  - `cargo run --release --bin dfs_bitmask -- bench 16 --min 14 --threads 8 --repeats 21 --warmup 3 --csv`
- memory：Windows `PeakWorkingSet64` 进程高水位；`cache_bytes` 另由
  `workers * slots * sizeof(slot)` 精确计算。RSS 包含线程栈、allocator、
  runtime 和 profile replay，不等于 live heap。

Raw data：

- `benchmarks/e44_cache_search.csv`
- `benchmarks/e44_cache_release.csv`
- `benchmarks/e44_e42_control_release.csv`
- `benchmarks/e44_dfs_control_release.csv`
- `benchmarks/e44_n17_trend.csv`

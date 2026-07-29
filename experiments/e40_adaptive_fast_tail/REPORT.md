# E40：actual-support adaptive merge→certified fast tail

## 决策

**KEEP。**

| N | E32 control | E40 adaptive（含选择） | E40 speedup | DFS | PEPS/DFS |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.1207 s | 0.01197 s | 10.1x | 0.01007 s | 1.19x |
| 15 | 0.7674 s | 0.07122 s | 10.8x | 0.06600 s | 1.08x |

两档均比 E32 快超过 2x、RSS 远低于 E32，且两档都进入同机 DFS 的
1.2x 范围，完整通过 E40 gate。Fixed-cut 的 N=15 cut=4 为
0.06859 s、DFS 0.06600 s（1.04x）；尚未在稳定中位数上超过 DFS。

N=16 出现系统级双峰：不同顺序的 21-repeat 中，PEPS median 曾为
0.569 s、低值约 0.405 s；DFS median 约 0.516--0.520 s，但也曾出现
约 0.399 s 的低值。最终同批表为 0.564 vs 0.520 s。低分位和中位数
对 crossover 给出相反结论，因此 **不宣称 N=16 已超过 DFS**。

## Hypothesis 与 exact PEPS 义务

- code revision：`95cb97c`；
- branch/worktree：`codex/exp-adaptive-fast-tail` /
  `.worktrees/e40-adaptive-fast-tail`；
- base：main `2c41e11`（accepted E38）；
- prefix：E37/E38 joint-u64 sparse contraction；
- tail：从 explicit 17-entry `C` 机械认证的 bit-transition plan；
- exact arithmetic：checked u64 fast path；任何 overflow 从同一 prefix
  用 checked u128 generic-C relation 完整 replay。

`CertifiedSecViTailPlan::compile` 只有在 `CompiledRowOperator` 从显式 C
证明 16 个 pass-through、唯一 occupied entry，并且 occupied legs 确为
四通道 0→1、local value=1 后才允许位并行 specialization。热循环的
available bits、三个 successor masks 和 last-row direct contraction
都是这个已认证 entry 与 `v1/v2` 边界的编译结果。若 C 改变，编译失败
closed；它不是调用 `dfs_bitmask`，也没有共享 comparator 代码。

测试用人为 coefficient limit=1 强制 u64 overflow，验证 u128 replay 得到
Q(8)=92；正常 N=1--10 的每个 cut 都把 u64 count 与独立 u128 profile
replay 比较。E38 的 recursive successor 仍逐 reachable parent replay
compiled C，而 compiled C 又逐项对 sitewise explicit C。45 个 release
tests 和 Clippy 通过。

## 性能机制与消融

1. E38 generic u128 tail：N=15 cut=6 为 0.1408 s。
2. certified bit plan + checked u64：约降到 0.077 s。
3. 将 shard sectors 展平，避免只在 256 shards 上窃取工作，最佳 cut
   移到更浅层。
4. last-row 直接施加 column v1 / diagonal v2，避免每个 solution 的最后
   一层递归。
5. 16-sector chunked atomic queue 减少 N=16 的数十万次 atomic fetch。
6. uninstrumented timing 与独立 u128 metrics replay 分离，和 DFS benchmark
   的 uninstrumented/profile 双路径口径一致。

N=15 cut=4 的 profiled recursive nodes=91,864,135，total accepted
C entries=91,865,192；DFS nodes=90,634,738、placements=91,883,698。
两者工作量已经几乎相同。PEPS 剩余约 4--8% 中位数差距主要来自 prefix
sort/reduce、sector unpack、动态任务 join 与 exact replay infrastructure，
不再是 frontier support 的数量级差距。

## Adaptive selector

E38 证明早期 cuts 的 tail nodes 几乎不变，主要差异是 parallel sectors
是否充足和 merge overhead。因此 selector 从 `max(1,N-11)` 开始，实际
收缩 prefix，并在 support 达到 `threads*512` 时停止；已选择的 prefix
直接复用，所有 probe 时间包含在 reported wall time。

初版 `threads*64` 阈值在最终 fixed-cut 消融中连续选择过深，故被证据
推翻。修正后：

- N=14：probe cut3 support=682，不足；cut4=4,811，选择 cut4；
- N=15：cut4 support=7,426，直接选择 cut4。

这与最终 cut-grid 的最佳点一致。D4 mode 已在 E39 先做 none/vertical/full
actual-cost 消融；full 相对 vertical 仅减 1.3% nodes 且更慢，故 E40
只搜索 accepted vertical mode 的 row cuts，不重复付 full-D4 overhead。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release/thin-LTO；`RAYON_NUM_THREADS=8`；shards=256；
- commands：
  - `cargo run --release --bin e40_fast_tail -- 256 14 3 7 7`
  - `cargo run --release --bin e40_fast_tail -- 256 15 4 8 7`
  - `cargo run --release --bin e40_adaptive -- 256 14 16 21`
  - `cargo run --release --bin dfs_bitmask -- bench 16 --min 14 --threads 8 --repeats 21 --warmup 3 --csv`
- adaptive/fixed timing：只计 uninstrumented exact count；一次独立 u128
  profile replay 记录 nodes/accepted entries，其时间单列，不进入 wall
  sample，和 DFS comparator 一致；
- memory：Windows `PeakWorkingSet64`，是包含 allocator、线程栈、runtime
  与 profile replay 的进程高水位，不是精确 live heap。

Raw data：`benchmarks/e40_certified_fast_tail_cut_grid.csv` 与
`benchmarks/e40_adaptive_release.csv`。

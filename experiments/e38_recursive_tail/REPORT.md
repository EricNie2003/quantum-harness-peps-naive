# E38：C-derived recursive tail contraction

## 决策

**KEEP。**

| N | E37 merged | E38 best cut | speedup | E38 RSS | DFS | PEPS/DFS |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.07234 s | cut 5: 0.02224 s | 3.25x | 10.7 MB | 0.01059 s | 2.10x |
| 15 | 0.4740 s | cut 6: 0.1408 s | 3.37x | 30.9 MB | 0.06594 s | 2.14x |

两档都通过“比 merged 快 3x 或 DFS gap ≤3x”的 gate。E38 是第一条把
当前 exact PEPS contraction 稳定推进到 DFS 三倍以内的路径；仍未超过
DFS，因此不能把它描述成最终目标已完成。

## Tensor-network fidelity

- code revision：`c094884`；
- branch/worktree：`codex/exp-recursive-tail` /
  `.worktrees/e38-recursive-tail`；
- base：main `ae15409`（accepted E37）；
- prefix：E37 joint-u64 exact sparse contraction；
- tail arithmetic：checked `u128` count；N<=21 的 node/accepted counters
  由 `sum P(N,k) < e*N! < u128::MAX` 给出静态上界。

尾部不是对 `dfs_bitmask` 的调用或复制。实现先从显式
`SiteTensorC::sec_vi()` 的 17 entries 调用 `CompiledRowOperator::compile`；
只有编译器证明 16 个 identity pass-through 和唯一 occupied entry 完整
存在，才提取 `column/row/dr/dl` 的 incoming/outgoing signals 与 local
coefficient。每个递归 successor 都使用这些运行时提取的 legs 更新 virtual
boundary。终点要求所有 column signals 为 1（`v1`），两族 diagonal 不过滤
（`v2=(1,1)`）；每行唯一 occupied branch 已施加 row `v0→v1`。

专门测试对 N<=8 的每个 reachable parent，将 recursive successor multiset
逐项与 compiled-C row contraction 比较；compiled-C 本身已有对 sitewise
显式 17-entry C 的逐项测试。N=0--10 的每个 cut（0..N）均与 known count
一致。显式 B/C 17-entry、truth table、boundary 和独立 oracle 测试仍通过；
总计 43 个 release tests、Clippy 通过。

## Cut grid 与机制

完整 cut=0..N 搜索见 `benchmarks/e38_recursive_tail_cut_grid.csv`。发现阶段
把多个 cut 放在同一进程，故后续行的 `PeakWorkingSet64` 被前序 cut 污染；
CSV 明确标记该限制。最终表的 pure-recursive、best-cut、pure-merged 均用
独立新进程复测，内存结论只引用最终表。

- cut=0 pure recursive：N=14/15 为 0.275/1.803 s；只有一个 task，8 线程
  无法并行，且未用 top-row vertical orbit；
- 很小 cut：获得 7/8 个 vertical-orbit tasks，但负载不均；
- cut=5/6：分别产生 26,928/243,380 个 prefix sectors，调度充分，同时
  merged frontier 仍很小；
- 继续增大 cut：recursive nodes 虽下降，但 materialize/sort 与 prefix RSS
  快速上升，最终回到 E37 的 full merged 成本。

N=15 best cut 的 recursive nodes=91,553,677、recursive accepted
entries=91,310,297；加 prefix 后 total accepted=91,610,302，接近 DFS 的
91,883,698 candidate placements。剩余约 2.14x 差距因此主要是每 node 的
state unpack/checked-u128/通用 C-derived relation 与任务调度成本，而不是
搜索工作量数量级。E39 应检验 full D4 是否能实质减少 nodes。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release/thin-LTO；
- threads：`RAYON_NUM_THREADS=8`；shards=256；
- commands：
  - discovery：`cargo run --release --bin e38_recursive_tail -- 256 14 15 0 8 3`
  - completion：`cargo run --release --bin e38_recursive_tail -- 256 14 15 9 15 1`
  - isolated best：N=14 cut=5、N=15 cut=6，各 7 repeats；
  - isolated pure recursive：N=14 5 repeats、N=15 3 repeats；
  - isolated pure merged：N=14/15 各 5 repeats；
  - `cargo run --release --bin e37_arena_reuse -- 256 14 15 7`
  - `cargo run --release --bin dfs_bitmask -- bench 15 --min 14 --threads 8 --repeats 11 --warmup 2 --csv`
- memory：Windows `PeakWorkingSet64`，是包含 allocator、线程栈和 runtime
  的进程高水位，不是精确 live heap。

Raw data：`benchmarks/e38_recursive_tail_release.csv` 与
`benchmarks/e38_recursive_tail_cut_grid.csv`。

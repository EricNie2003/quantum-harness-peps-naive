# E49：cross-sector exact AVX2 root batching

## 决策

**REJECT，不并入 production；按 kill gate 不运行 N=18。**

| N | E47 scalar last-6 | E49 AVX2 batch | change | root lane occupancy |
|---:|---:|---:|---:|---:|
| 16 | 0.327347 s | 0.332629 s | +1.6% | 89.1% |
| 17 | 2.667296 s | 2.724125 s | +2.1% | 90.3% |

E49 要求两档快 15%，kill gate 为收益低于 8%。实际两档均变慢，且
RSS、tasks、nodes 和 accepted entries 无改善，因此立即停止。

## 实现、exactness 与 PEPS fidelity

- code revision：`a460931`；
- branch/worktree：`codex/exp-batched-avx2` /
  `.worktrees/e49-batched-avx2`；
- base：main `bdcb4e3`（accepted E47 + rejected E48 report）；
- hardware path：runtime `is_x86_feature_detected!("avx2")`，否则完整
  fallback E47 scalar last-6。

每四个互不重叠的 direct C sectors 形成一批。AVX2 同时计算四 lanes 的
`~(columns|diag_dr|diag_dl)&mask`，并在每个 root round 向量化
columns OR、DR left shift/mask、DL right shift。每 lane 的每个 occupied
C entry 随后调用 E47 certified scalar last-6 recursion；orbit weights
和 checked reductions 独立保留。

候选没有合并不同 boundary、没有丢弃 inactive lane、没有共享 subtree
count。测试对 N=0..12 比较 AVX2 batch、scalar last-6、generic
checked-u128 C replay 和 known counts，并检查 tasks/accepted entries
一致。非 AVX2 target fail-safe 回到 scalar，而不是产生不同算法。

## 失败机制

N=16/17 分别有 17,726/28,608 个 full four-lane batches，只有一个
partial batch。root-round slot occupancy 已达 89--90%，所以失败不是
padding 或极端 lane divergence。

真正问题是 SIMD 只覆盖每个 sector 的第一层三个 availability masks 和
三个 successor masks；随后绝大多数 570M/4.276B C transitions 仍在
scalar recursion。为这不到一个递归层的覆盖，E49 增加：

- arrays→YMM load、YMM→arrays store；
- per-round selected extraction；
- four-lane scalar dispatch；
- batch-index scheduling。

这些成本超过被向量化的少量 bitwise instructions。要让 SIMD 有机会，
必须像 E50 一样让 lanes 在**每个递归节点**执行相同控制流，而不是把
控制流不同的 boundary tasks 塞进 lanes。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，AVX2；rustc 1.94.0 / LLVM 21.1.8；
- release、thin LTO、codegen-units=1、8 threads；
- candidate/control 均 1 warmup + 5 samples，另有 generic C metrics
  replay；
- commands：
  - `cargo run --release --bin e49_batched_avx2 -- 16 17 5 1 2048`
  - `cargo run --release --bin e47_last_k -- 16 17 5 1 2048 6`
- memory：Windows `PeakWorkingSet64` high-water mark，不等于 live heap。

Raw data：`benchmarks/e49_batched_avx2_release.csv`。

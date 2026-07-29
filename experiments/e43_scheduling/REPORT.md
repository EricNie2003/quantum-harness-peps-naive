# E43：actual-cost task ordering 与 chunk/backend search

## 决策

**REJECT，不并入 production。**

正式候选选择探索中 N=15 最快的 natural ordering + Rayon：

| N | E42 control | E43 candidate | change | E43 p90 | DFS | E43/DFS |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.010627 s | 0.010265 s | -3.4% | 0.010734 s | 0.010007 s | 1.026x |
| 15 | 0.061533 s | 0.061466 s | -0.1% | 0.063605 s | 0.064518 s | 0.953x |
| 16 | 0.465674 s | 0.472965 s | +1.6% | 0.485225 s | 0.482288 s | 0.981x |

E43 没有满足 N=14/15 median 至少下降 5%，N=16 p90 也没有下降
15%。同批 N=15、16 PEPS 略快于 DFS 是 accepted E42 microkernel 的
能力，不是 E43 调度带来的增益。

## 实现与 PEPS/exactness 边界

- code revision：`7dc1d18`；
- branch/worktree：`codex/exp-scheduling` /
  `.worktrees/e43-scheduling`；
- base：main `2014844`（accepted E42）；
- arithmetic：checked u64；overflow 仍由同一 sectors 的 generic-C
  checked u128 replay；
- contraction work、support、accepted C entries 与 E42 完全相同。

E43 只重排/分派 E42 已由 explicit 17-entry C 和
`CertifiedSecViTailPlan` 认证的 virtual-boundary sectors。两个 hardness
score 都只读取 `(columns, diag_dr, diag_dl)`：廉价版统计当前可匹配的
occupied C entries；probe3 版重复三行 certified C transition。它们不
读取 DFS、known count 或 subtree 解数。Rayon/atomic 都在 exact checked
reduction 中组合互不重叠 sectors。

测试枚举 natural、available-hard-first、probe3-hard-first，
NativeAtomic/Rayon 和 chunk=1/4/16/64，均与 generic u128 C replay
得到 Q(9)=352。E42 的 N=0..10、forced overflow 和逐 reachable-parent
explicit-C 测试继续通过。

## 搜索结果与失败机制

N=16 的 7-repeat 探索：

| ordering | backend/chunk | median | p90 | ordering cost |
|---|---|---:|---:|---:|
| natural | atomic/1 | 0.3893 s | 0.4558 s | ~0 |
| natural | atomic/4 | 0.3808 s | 0.4654 s | ~0 |
| natural | atomic/16 | **0.3700 s** | **0.4504 s** | ~0 |
| natural | atomic/64 | 0.3801 s | 0.4565 s | ~0 |
| natural | Rayon | 0.3772 s | 0.4561 s | ~0 |
| available hard-first | best median | 0.3843 s | 0.4741 s | 0.6--1.2 ms |
| probe3 hard-first | best median | 0.9956 s | 1.0934 s | 0.40--0.61 s |

结论：

1. chunk=16 原 E42 默认值仍是 N=16 探索的最佳 median/p90；调小 chunk
   没有消除长尾，故 tail-sector load imbalance 不是双峰主因。
2. Rayon 在 N=15 略快且 RSS 更低，但正式 N=16 反而慢 1.6%。
3. available popcount 太弱，无法预测深层 subtree cost；排序本身虽便宜，
   仍没有调度收益。
4. probe3 与 subtree cost 更相关的可能性更高，但为了估价重复约一次完整
   contraction 的工作，N=16 仅 scoring 就花 0.40--0.61 s。这个 cost
   model 在 exact search 中不可行。
5. 7-repeat 的 N=16 low mode 与正式 21-repeat high mode再次复现，且
   DFS 同样随批次移动。证据更支持系统频率/Windows scheduling 双峰，
   而不是 PEPS task queue 的可修负载失衡。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；
- threads：8，shards=256；
- search：2 warmups + 7 samples，10 个 primary configurations，加廉价
  hard-first follow-up；ordering/scoring 全部计入 wall time；
- formal：3 warmups + 21 samples，另一次 generic u128 profile replay；
- commands：
  - `cargo run --release --bin e43_scheduling -- <ordering> <backend> <chunk> 15 16 7 2`
  - `cargo run --release --bin e43_scheduling -- natural rayon 16 14 16 21 3`
  - `cargo run --release --bin e42_last_k -- 256 14 16 21 3 4`
  - `cargo run --release --bin dfs_bitmask -- bench 16 --min 14 --threads 8 --repeats 21 --warmup 3 --csv`
- memory：Windows `PeakWorkingSet64` 进程高水位，包含 allocator、线程栈、
  runtime 与 profile replay，不等于 live heap。

Raw data：

- `benchmarks/e43_schedule_search.csv`
- `benchmarks/e43_natural_rayon_release.csv`
- `benchmarks/e43_e42_control_release.csv`
- `benchmarks/e43_dfs_control_release.csv`

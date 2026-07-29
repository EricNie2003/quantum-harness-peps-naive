# E55：last-6 热代码形状与 I-cache 消融

## 决策

**REJECT，N=18 early kill；保留 production 的 fully-inline 形状。**
把 last-5/last-6 从 `inline(always)` 降为普通 `inline`，整个可执行文件的
`.text` 只减少 864 bytes（0.315%），N=16/17 正式 median 却慢
3.9%/6.4%。进一步把 last-4/5/6 全部 `inline(never)`，`.text` 也只减少
1,088 bytes（0.397%），N=16/17 exploratory median 慢 16.4%/17.9%。

| N | fully-inline | regular-inline | wall change | noinline | wall change |
|---:|---:|---:|---:|---:|---:|
| 16 | 0.330426 s | 0.343191 s | +3.9% | 0.384497 s | +16.4% |
| 17 | 2.893766 s | 3.079283 s | +6.4% | 3.412570 s | +17.9% |

所有形状的 count、cut、tail task、prefix support 和 accepted explicit-C
entries 完全相同。RSS 差异不到 1%，没有支持集或内存收益。按照预注册的
`>2%` slowdown gate，不跑 N=18。

## 与 99.98% hotspot 的关系

E51 的 1 kHz Samply profile 在 N=17 production fast solve 中记录了
38,578 个 EXE leaf samples，其中 38,570 个（99.979262%）落在
`contract_certified_tail_last_k_u64::<6>`；按 sample CPU delta 计为
99.984696%。另一次 generic explicit-C validation replay 的 47,883 个
samples 全部落在 `contract_recursive_tail`，它是验证阶段而不是
production 计时核。

这个结果证明 production 时间高度集中，但不能把函数名所覆盖的全部工作
解释为一个可消除的调用开销。`::<6>` monomorphization 包含两段：

1. `remaining_rows > 6` 时，循环从
   `!(columns | diag_dr | diag_dl) & board_mask` 枚举所有合法 C 分支，
   用 low-bit extraction 选一个 occupation=1 transition，执行三个
   virtual-mask successor update，再递归和 checked accumulation；
2. 最后六行进入 fully-inlined last-6/5/4 嵌套循环，省掉通用递归层、
   match 和一部分函数边界。

E55 对第二段做了 code-shape 消融。如果 I-cache 容量是主因，减少热核代码
应当不慢、甚至变快；实测却相反，而且全程序 `.text` 最多只缩小 0.40%。
因此现有证据支持：强制内联的收益来自消除热循环内的 call/return、分支和
参数/状态搬运，而不是以 I-cache miss 换速度。

## 热点函数的实际瓶颈

结合 E51--E55，瓶颈是**低复用、数据依赖强的 exact C-state expansion
吞吐**：

- 每个被接受的 queen branch 都依赖上一步的 columns/两族 diagonal masks，
  要做 OR/NOT/AND、low-bit extraction、两个 diagonal shift 和循环分支；
- 分支数由合法 occupation transition 决定，绝大多数不同 prefix 不会汇合
  到同一个 suffix state。E54 在 remaining=7 做 lossless exact cache，
  最大 256 KiB/worker 的 N=17 hit rate 只有 0.277%，证实普通 suffix
  temporal locality 极弱；
- E51 把 task record 从 32 bytes 压到 8 bytes、E52 把 task working set
  压到 L1/L2 预算、E53 改 worker ownership，均没有持续 wall-time 收益；
  因而 prefix/task memory traffic 不是 99.98% 时间的主导项；
- E55 又排除了“热代码过大导致 I-cache 容量瓶颈”这一假设。

本机 WPR 因 policy error `0xc5585011` 拒绝 system-performance profile，
xperf PMU 配置以 `0x1069` 失败，所以这里不声称有实测的 L1/L2/I-cache
miss、branch-mispredict 或 IPC 数字。上面的函数级占比是采样器实测；
函数内部瓶颈是由源码控制流和 E51--E55 消融共同支持的归因。

## 怎样消除瓶颈

不能通过“优化掉这个函数”得到接近 100% 的加速，因为它也承载了必须完成
的 exact contraction。可行的下一步必须减少每个 C transition 的动态指令，
或减少 transition 总数：

1. 将已经认证的 fully-inlined terminal kernel 扩展到 last-7/last-8，
   直接测量是否继续减少递归/分支，同时严格监控 code size 与寄存器 spill；
2. 对 terminal kernel 做 source-generated 固定深度版本，比较 nested-loop、
   branchless accumulation 和 compiler branch-hint/PGO 形状；目标是减少
   call、match、checked-add hot-path 和不可预测分支；
3. 用无 profiling counter 的独立构建记录 generic-node work，确保任何收益
   来自每节点成本下降，而不是漏数；若动态 instruction reduction 停滞，
   则转向 exact symmetry/association 以减少节点数；
4. 不再优先扩大普通 memoization table 或调整 task layout，除非新的状态
   canonicalization 能先证明至少几个百分点的复用率。

## exactness、PEPS fidelity 与验证

- code revision：`ad83da132b1d820297d6c4ef3b3e564086918792`；
- branch/worktree：`codex/exp-icache-shape` /
  `.worktrees/e55-icache-shape`；
- base：main `bac3fe2`，E51--E54 rejected code 均不在 production；
- 三种形状只改变 Rust inlining attribute；explicit rank-9 `B`、rank-8
  `C` 的 17 entries、v0/v1/v2 boundaries、checked exact-u64/CRT promotion、
  C-derived transition 和 D4 top-row orbit weights 均不变；
- targeted last-5/6 vs generic-C replay 在三种 feature 下通过；default
  release suite 51 passed；format 和三种 feature 的 clippy 均通过；
- E55 CLI 每个 N 再做一次 known-count verification；N=16/17 均为 true。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1；`RAYON_NUM_THREADS=8`；
- formal control/regular：N=16/17，1 warmup + 5 repeats；
- noinline early kill：N=16/17，1 warmup + 3 repeats；
- command：
  `cargo run --release --target-dir <shape-target> [--features <shape>] --bin
  e55_icache_shape -- 16 17 <repeats> 1 0`；
- section size：
  Rust toolchain bundled `llvm-size.exe`，记录 PE `.text/.data/.bss` 和文件
  metadata length；
- RSS：Windows `PeakWorkingSet64` process high-water；它包括 allocator、
  runtime 与线程栈高水位，不是 cache residency，也不能测 L1/L2；
- raw CSV：
  - `benchmarks/e55_icache_shape_formal.csv`
  - `benchmarks/e55_icache_shape_early_kill.csv`
  - `benchmarks/e55_icache_shape_sections.csv`
  - profile 原始表沿用同一未改变 production hot kernel 的
    `benchmarks/e51_baseline_samply_hotspots.csv`。

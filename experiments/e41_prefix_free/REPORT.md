# E41：prefix-free C-derived sector seeding

## 决策

**REJECT，不并入 production。**

| N | E40 control | E41 prefix-free | E41/E40 speedup | DFS | E41/DFS |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.011667 s | 0.010256 s | 1.138x | 0.009942 s | 1.032x |
| 15 | 0.069842 s | 0.067252 s | 1.039x | 0.065624 s | 1.025x |
| 16 | 0.516994 s | 0.514667 s | 1.005x | 0.483813 s | 1.064x |

E41 只在 N=14 达到 8% speedup；N=15、16 分别只有 3.7% 和
0.45%，且没有一个 N 的稳定中位数超过 DFS。它不满足“连续两档至少
8%”或“稳定超过 DFS 一档”的 gate。

## Hypothesis、实现与 PEPS fidelity

- code revision：`1bc6d97`；
- branch/worktree：`codex/exp-prefix-free` /
  `.worktrees/e41-prefix-free`；
- base：main `7184a08`（accepted E40）；
- arithmetic：checked u64；任何 overflow 从同一组 prefix-free sectors
  用 checked u128 generic-C relation 完整 replay；
- symmetry：只使用已通过 E39 消融的 top-row vertical reflection orbit。

E41 从 explicit 17-entry `C` 构造 `CompiledRowOperator`，再编译
`RecursiveTailRelation` 和 `CertifiedSecViTailPlan`。只有 occupied entry
严格为四个 constraint channels 的 0→1、local value=1 时 fast path 才
启用。prefix 的每一步都调用这个 relation 生成 successor；它保留每条
互不重叠的 placement path，而不把相同 virtual boundary 合并。top-row
左半分支分别携带 orbit weight 2，奇数 N 的中心分支 weight 1。

tail 仍是 certified Sec. VI contraction：available mask 和 successor
shift 是 occupied C entry 的机械编译，终端 columns 与 `v1` 收缩，
diagonals 与 `v2` 收缩。没有调用或共享 `dfs_bitmask` comparator。

测试覆盖 N=0..10 known counts、u64 fast result 与 u128 replay 一致，并用
coefficient limit=1 强制 replay 后得到 Q(8)=92。既有测试逐 reachable
parent 对比 recursive successor、compiled C 和 sitewise explicit C。

## 失败机制

E41 成功删除了 prefix sort/reduce 和 packed sector 解码，并把 peak RSS
从 E40 的 9.2/10.5/22.8 MB 降到 5.9/6.2/6.5 MB。但 measured seed
仅为 0.037/0.079/0.126 ms；E40 同一 selected prefix 的时间也只有
0.816/0.956/1.940 ms。N=15 的 91.9M 和 N=16 的 570.6M recursive
accepted C entries 才是绝对主体，所以 prefix 优化不可能产生要求的
连续 8% wall-time 收益。

不合并的路径只引入很少重复：

- N=14：4,816 tasks vs E40 4,811 merged sectors；
- N=15：7,432 vs 7,426；
- N=16 的 E41 在 cut=4 产生 9,844 tasks；E40 的 adaptive selector
  为了 support threshold 在 merge 后进入 cut=5、产生 70,745 sectors。

后一点说明 prefix-free seeding 能更早获得足够并行度，但 N=16 的
tail work 仍从 E40 的 570.13M 微升到 570.59M，抵消了调度与 prefix
节省。这个实验否定了“merge overhead 是剩余主要差距”的假设，E42
应直接减少/加速最后若干行的 C transition。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；
- threads：`RAYON_NUM_THREADS=8`，DFS `--threads 8`；
- repetition：3 warmups + 21 uninstrumented samples；表中为 median；
  独立一次 u128 profile replay 不计入 wall samples；
- commands：
  - `cargo run --release --bin e41_prefix_free -- 14 16 21 3`
  - `cargo run --release --bin e40_adaptive -- 256 14 16 21`
  - `cargo run --release --bin dfs_bitmask -- bench 16 --min 14 --threads 8 --repeats 21 --warmup 3 --csv`
- memory：Windows `PeakWorkingSet64` 进程高水位；它包含 allocator、
  Rayon/native worker stacks、runtime 和独立 profile replay，不等于
  live heap，因此只适合同机同协议比较。

Raw data：

- `benchmarks/e41_prefix_free_release.csv`
- `benchmarks/e41_e40_control_release.csv`
- `benchmarks/e41_dfs_control_release.csv`

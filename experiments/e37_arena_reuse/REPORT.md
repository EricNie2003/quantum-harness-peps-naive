# E37：跨行 destination bucket capacity 复用

## 决策

**KEEP。**

最终反向执行顺序、独立进程、7-repeat 配对结果：

| N | E36 | E37 | speedup | E36 RSS | E37 RSS |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.08087 s | 0.06933 s | 1.17x | 71.24 MB | 69.44 MB |
| 15 | 0.53246 s | 0.46563 s | 1.14x | 336.80 MB | 333.53 MB |

两档均超过预注册 10% time gate，且最终配对 RSS 未增加。首轮 5-repeat
配对曾测得 N=14/N=15 RSS 分别高 6.3%/0.3%；反转命令顺序并增加重复后
不再出现，故判断为 Windows working-set/allocator 波动而非稳定回归。
该不确定性保留在报告中，不用单次有利结果掩盖。

## 假设、实现与 fidelity

- hypothesis：E36 每行销毁 destination vectors，再让 allocator 为下一行
  提供相似 bucket capacity；显式双缓冲可避免重复 growth/copy；
- code revision：`8b80599`；
- branch/worktree：`codex/exp-arena-reuse` /
  `.worktrees/e37-arena-reuse`；
- base：main `b709f9f`（accepted E36）；
- arithmetic：E36 joint u64 checked arithmetic，两级 exact promotion 不变。

内核维护 current boundary 和 spare destination 两组 shard vectors。第 r
行读取 current、把 candidates 写入上一轮已 `clear()` 的 spare；generation
结束后，旧 current 清空并成为第 r+1 行的 spare。`clear()` 保留 allocation
但移除所有旧 coefficient；排序与 checked reduce 只看到本轮真实写入的
candidate multiset。非复用 E36 仍可由同一参数化内核调用，便于消融。

这一改动不改变显式 17-entry `B/C`、compiled `C` row relation、
`v0/v1/v2`、top-row vertical D4 orbit weighting、prefix partition 或
final v1/v2 contraction。N=0--10 测试逐项比较 E36/E37 count、peak support
与 accepted C transitions；41 个 release tests 与 Clippy 通过。

## Allocation/capacity 结果

N=15 最近一次运行累计向 destination 提供 1,007,750,496 bytes 的已保留
capacity；在已有 capacity 之外仍增长 461,508,048 bytes；单层可供复用的
spare capacity 峰值 244,508,000 bytes。这里“累计复用”是各层 capacity
之和，不是同时 live bytes，不能和 RSS 直接相加。

N=15 的最近一次 generation 0.3231 s；对照为 0.3446 s。总 wall 的改善
还包含减少 allocator book-keeping、reallocation/copy 对随后 sort 的
cache 影响。sort/reduce 算法和 8-byte record 均未变化，support
18,178,233、accepted entries 80,077,350 完全相同。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release/thin-LTO；
- threads：`RAYON_NUM_THREADS=8`；
- shards：256，Prefix mode；
- final commands（独立进程，反向顺序）：
  - `cargo run --release --bin e36_joint_u64 -- 256 14 15 7`
  - `cargo run --release --bin e37_arena_reuse -- 256 14 15 7`
- repetition：七次取中位数和最小值；阶段明细及 capacity counters 来自
  同进程最后一次 repeat，不冒充阶段中位数；
- memory：Windows `PeakWorkingSet64`，包含 allocator retained pages、
  线程栈和 runtime；它是进程高水位而非精确 live heap。

Raw data：`benchmarks/e37_arena_reuse_release.csv`。

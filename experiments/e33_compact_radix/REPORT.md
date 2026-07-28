# E33：compact shard LSD radix

## 决策

**REJECT；production 保持 E32 `sort_unstable_by_key`。**

同一 worktree 的最终 13-bit digit 消融：

| N | standard | radix | delta | standard sort | radix sort | RSS delta |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.130 s | 0.153 s | +17.7% | 0.040 s | 0.059 s | +1.3% |
| 15 | 0.835 s | 1.121 s | +34.3% | 0.290 s | 0.501 s | +0.2% |

没有达到 time gate，且 N=15 显著退化，触发 kill criterion。

## 实现与 fidelity

- code revision：`1c3f59d`；
- branch/worktree：`codex/exp-compact-radix` /
  `.worktrees/e33-compact-radix`；
- base：main `faf5d3a`（accepted E32）；
- entries/arithmetic：E32 16-byte `u64 key + u64 coefficient`，checked
  overflow 后自动 E31 u128 replay；
- contraction：候选生成、explicit-`C` compiled operator、D4、
  `v0/v1/v2`、support 和 reduction 均不变，仅替换每个 prefix shard
  内的 key sort。

radix 对 shard 中相对首 key 不变化的 digit 自动跳过；稳定
out-of-place scatter 在 `entries` 与同长度 scratch 间交替，最后按
需要 copy back。测试对 20,000 个确定性乱序 u64 keys 与标准排序
逐 key 比较，并在 N=0--10 对完整 PEPS contraction 比较 count、
support 和 operator work。37 个 release tests 和 Clippy 通过。

## Digit-width 消融与失败机制

| digits | N=15 total | N=15 sort |
|:---|---:|---:|
| 8-bit | 1.236 s | 0.648 s |
| 13-bit | 1.121 s | 0.501 s |
| standard comparison sort | 0.835 s | 0.290 s |

13-bit 把典型 prefix shard 的 pass 数从约 5 降到约 3，确实比
8-bit 快 22.7%（sort phase），说明 pass/streaming traffic 是主要
问题；但仍要多次完整读写候选和清零 8192-bin histogram。
Rust 标准 pdqsort 对 16-byte records、部分 prefix locality 和中等
shard sizes 已很高效，单次比较移动少于 radix 的三次稳定 scatter。

PeakWorkingSet64 只增加约 1%，不是 scratch 真正免费的证据：
scratch 在 shard 内短命、各 shard 大小有限，Windows allocator
复用/进程高水位掩盖了瞬时 live allocation；运行时结果已足以判退。
未来只有在 entry 更小、shard 更大或能做 in-place MSD partition
并同时减少后续 reduction traffic 时才应重访 radix。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- release/thin-LTO；`RAYON_NUM_THREADS=8`；256 prefix shards；
- commands：
  - `cargo run --release --bin e33_compact_radix -- 256 14 15 3`
  - `cargo run --release --bin e32_u64_promotion -- 256 14 15 3`
- each 3 repeats, report median/min；standard control 在同一最终
  revision 立即复测。8-bit 数据来自同一 worktree 的前置原型，
  13-bit 是提交 revision。
- memory：Windows `PeakWorkingSet64`，是进程高水位，包含
  allocator/线程栈，不能直接解释瞬时 scratch live bytes。

Raw data：`benchmarks/e33_compact_radix_release.csv`。

# E28：24-byte compact virtual key / exact coefficient layout

## 决策

**KEEP。** 固定 Prefix/256，与 E26 完全相同的 states/transitions：

| N | E26 8t | E28 8t | time improvement | E26 RSS | E28 RSS | RSS improvement |
|---:|---:|---:|---:|---:|---:|---:|
| 14 | 0.25820 s | 0.22065 s | 14.5% | 206,209,024 | 157,429,760 | 23.7% |
| 15 | 1.85859 s | 1.61179 s | 13.3% | 1,256,726,528 | 946,487,296 | 24.7% |

达到预注册的 RSS `>=20%` keep gate。单线程 N=14/15 为
0.44980/3.15005 s，相对 E26 0.51378/3.63108 s 也分别快
12.5%/13.2%。

## 实现与 fidelity

- code revision：`2803fec`；
- branch/worktree：`codex/exp-compact-soa` /
  `.worktrees/e28-compact-soa`；
- base：main `664648e`，production baseline 为 E26。

对 N<=21，三族 virtual boundary 共 `3N<=63` bits，key 精确存为
`u64`。exact `u128` coefficient 拆成 low/high 两个 `u64`，形成
`CompactEntry { key, weight_low, weight_high }`，`size_of=24`，避免
Rust 对 `(u128,u128)` 的 16-byte alignment 将 entry 扩为 32 bytes。

每次相加前无损重构 `u128` 并使用 `checked_add`，随后拆回两个 halves。
没有缩小 coefficient、浮点或未对齐 unsafe access。row-aware 条件是
N<=21；更大 N 明确报错而不截断 key。

局域 successor 仍从显式 17-entry C compiled operator 和 sparse incoming
signal intersection 产生；`v0/v1/v2`、diagonal shift、D4 首行 orbit、
256 prefix shards 均与 E26 相同。

## Correctness 与 benchmark

34 个 release tests 通过。新增测试确认 entry 恰为 24 bytes，并在
N=0--10 比较 E26 的 count、peak support、candidate/matched work。
Clippy `-D warnings` 通过。

环境：AMD Ryzen 9 7945HX、Windows MSVC、rustc 1.94.0、
release/thin-LTO；1/8 Rayon threads；每点 3 次、无预热、中位数。
RSS 为 Windows process `PeakWorkingSetSize`，包含 allocator 保留页。

```powershell
$env:RAYON_NUM_THREADS='8'
cargo run --release --bin e28_compact -- 256 14 15 3
$env:RAYON_NUM_THREADS='1'
cargo run --release --bin e28_compact -- 256 14 15 3
```

N=15 仍有 18,178,233 peak states 和 80,077,350 accepted C
transitions；E28 改善 bytes/entry 和 memory traffic，不改变增长率。
8-thread DFS gap 从 E26 的 28.12x 降至 24.39x。

Raw data：`benchmarks/e28_compact_release.csv`。

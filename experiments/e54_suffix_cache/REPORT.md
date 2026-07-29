# E54：per-worker bounded tagged suffix cache

## 决策

**REJECT，N=18 early kill。** 最大的 256 KiB/worker cache 在 N=16/17
也只有 0.255%/0.277% hit rate，远低于预注册的 3% kill。最低开销的
4 KiB candidate 正式 5-sample median 比 control 慢 9.6%/11.7%。

| N | control median | 4 KiB cache | wall change | 256 KiB hit rate |
|---:|---:|---:|---:|---:|
| 16 | 0.312261 s | 0.342143 s | +9.6% | 0.255% |
| 17 | 2.899234 s | 3.237864 s | +11.7% | 0.277% |

4/16/64/256 KiB grid 的 hit rate 随容量增加，但全部小于 0.28%；
candidate 的 count、generic recursive nodes 和 accepted C entries 都与
control 一致。

## 99.98% hotspot 的含义

E51 `samply` profile 把 99.979% production leaf samples定位到
`contract_certified_tail_last_k_u64::<6>`。E54 第一次直接在该函数的
remaining=7 边界尝试消除重复子树：命中才直接返回一个 exact completion。
结果表明热点不是“同一 suffix 在 L1/L2 外反复加载/重算”的 cache
bottleneck。N=17 有 190,357,772 次 lossless lookup，但 256 KiB table
只命中 527,989 次；hash、tag load、branch 和几乎每次 insert 的成本远高于
省下的 0.28% 子树。

因此 99.98% 并不自动意味着存在接近 100% 的可消除时间。它说明所有
不可避免的 work 都集中进了一个 monomorphized kernel；当前证据更支持
瓶颈是低复用、分支依赖的 C-state expansion。要明显提速，需要降低每个
扩展的指令/分支成本，或用新的 exact association 减少扩展数，而不是
再扩大普通 transposition cache。

## exactness 与 PEPS fidelity

- code revision：`4f67b12cd525aa0670b5776dce08a2e0456ebfa4`；
- branch/worktree：`codex/exp-suffix-cache` /
  `.worktrees/e54-suffix-cache`；
- base：main `68f0c2e`，E51--E53 rejected code 均不在 production；
- slot 为 16-byte `(lossless key, exact u64 value)`；cache object 固定
  一个 N，key 无损编码 three N-bit virtual masks 和 remaining rows；
  tag 必须完整相等，collision 只产生 miss；
- 每个 worker 独占 table，无共享 mutation；cache 不跨 N/solve 复用；
- cache 只保存 certified last-6 上方 remaining=7 的 exact count；
  checked overflow、column v1、diagonal v2 和 C-derived prefix不变；
- timed path 以 const `MEASURE=false` 编译掉统计；独立 measured solve
  记录 hits，再由 generic explicit-C replay核对 count/work。

N=0..10、四个容量都与 generic C/known counts 一致；N=14 确认实际
lookup path被覆盖；slot size 和容量有静态测试。完整 release suite
52 passed，format/clippy 通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- grid：N=16/17，1 warmup + 3 repeats，另做 measured cache solve 和
  generic-C replay；
- formal：control/4 KiB，1 warmup + 5 repeats；hit rate引用同 revision
  grid 的 measured solve；
- RSS 为 Windows `PeakWorkingSet64` process high-water；
- raw CSV：
  - `benchmarks/e54_suffix_cache_grid.csv`
  - `benchmarks/e54_suffix_cache_formal.csv`

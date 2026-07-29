# E53：cache-line tiled task ownership

## 决策

**REJECT，不运行 N=18。** 正式 dynamic 1-line candidate 在 N=16/17
分别比同 revision control 慢 1.3%/0.7%，p90 也没有改善 10%。
完整 1/2/4/8/16 cache-line grid 没有一个配置在两档达到 2% early gate。

| N | control median | dynamic 1-line median | change | control p90 | candidate p90 |
|---:|---:|---:|---:|---:|---:|
| 16 | 0.310696 s | 0.314710 s | +1.3% | 0.311181 s | 0.319114 s |
| 17 | 2.734867 s | 2.753447 s | +0.7% | 2.753686 s | 2.754449 s |

一次 cyclic N=17 diagnostic 恰好落在 2.495 s 的低峰，但先前 3-sample
median 为 2.799 s。追加预热后的 9-sample 复测为 2.897 s，故没有挑选
该单样本；N=16 同批 9-sample median 也为 0.332 s。

## worker work 与热点机制

每个 32-byte AoS task 顺序读取，dynamic tile 从 1 到 16 条 64-byte
cache lines 都没有稳定改变 wall。这直接排除了 shared
`AtomicUsize` cache-line bouncing 是主要瓶颈；它每 2--32 tasks 才执行
一次，而 E51 profile 已显示 99.979% samples 在 last-6 kernel。

static contiguous 令 worker task counts 几乎完全相等，但 N=17 的
generic-C worker node counts 从 356,190,023 到 656,459,655，max-min
为 300,269,632（平均的 56%），因此 median 慢到 3.02--3.29 s。
block-cyclic 把 node max-min 降到 9,257,596（平均的 1.7%），但
9-sample wall 仍慢约 5.9%。当前 dynamic scheduler 的少量 coherence
成本换来了必要的硬子树负载均衡；它不是可消除的 99% hotspot。

## exactness 与 PEPS fidelity

- code revision：`678fdd938651594b5d79fbfe4e8d202bfe4eec33`；
- branch/worktree：`codex/exp-cache-tiles` /
  `.worktrees/e53-cache-tiles`；
- base：main `1ee9983`；E51/E52 rejected code 均不在 base；
- 三种 schedule 只分配相同的 C-derived `WideCrtTask`，每个 task
  恰好消费一次；tail 仍是 certified last-6，boundary 与 checked exact
  arithmetic 不变；
- N=0..10 对三种 schedule、1/8/16-line tiles 都与 generic-C replay 和
  known counts 一致；测试还要求 worker task/node reductions 等于全局
  task/node totals。

完整 release suite 52 passed；format 和 `clippy -D warnings` 通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- grid：N=16/17，1 warmup + 3 repeats；
- formal control/dynamic：1 warmup + 5 repeats，加一次 generic-C replay；
- contiguous/cyclic node diagnostic：1 sample + generic-C replay；
- cyclic 冲突复测：2 warmups + 9 repeats；
- N=18 early kill；
- RSS 为 Windows `PeakWorkingSet64` process high-water mark；
- raw CSV：
  - `benchmarks/e53_cache_tile_grid.csv`
  - `benchmarks/e53_cache_tile_formal.csv`
  - `benchmarks/e53_cyclic_resample.csv`

# E3：把三个 virtual-boundary masks 打包为单个 u128 key

## 预注册

- 分支：`codex/exp-packed-u128`
- baseline：E1 KEEP 后端，code commit
  `20b5334f55819ab0b4bdce7aa701527de736c3dc`
- candidate code commit：`dde47b7fad35e6d2cbd94422d2f70559ca80d883`
- 单变量：HashMap key 从三个 `u64` 字段的 `BoundaryState` 改为单个 `u128`；
- 假设：key 从 24 bytes 降到 16 bytes，降低 bytes/state、RSS 和 hash/cache 成本；
- keep：两个连续 N 的 RSS 降低至少 15%，或时间稳定改善至少 15%；
- kill：时间和 RSS 都无稳定改善。

## 编码与 PEPS 合规性

对棋盘宽度 \(N\)，packed key 的 bit layout 是：

```text
[0,N)       columns
[N,2N)      down-right diagonal virtual signals
[2N,3N)     down-left diagonal virtual signals
```

该编码与三个开放 virtual-index masks 一一对应。主 contraction 从 map 取出 key 后立即
`unpack` 为原 `BoundaryState`，继续调用完全相同的逐格点 `CEntry` contraction；每个
successor 只在插入 HashMap 前 `pack`。局域 \(B/C\)、ordering、系数和边界向量没有变化。

新增测试覆盖 \(N=1\ldots42\) 的零、全一和交错模式 pack/unpack round trip。目标
\(N\le28\) 只需 84 bits。当前布局的通用上限从旧版的 63 降为 42，但不影响 Issue #34
范围。

全部 9 个 release 测试和 Clippy 通过；所有 benchmark 的 count、support 和 tensor work
与 baseline 完全一致。

## Benchmark

环境、release 配置、单线程和 Windows peak-working-set RSS 与 E1 相同。

| N | baseline (s) | packed (s) | 时间变化 | baseline RSS (MiB) | packed RSS (MiB) | RSS 降低 |
|---:|---:|---:|---:|---:|---:|---:|
| 10 | 0.016569 | 0.019144 | +15.5% | 7.46 | 6.71 | 10.0% |
| 11 | 0.082916 | 0.077946 | -6.0% | 14.37 | 11.14 | 22.4% |
| 12 | 0.489594 | 0.483586 | -1.2% | 37.35 | 26.86 | 28.1% |
| 13 | 3.221072 | 3.103225 | -3.7% | 202.52 | 137.97 | 31.9% |
| 14 | 19.308394 | 17.611201 | -8.8% | 986.67 | 666.17 | 32.5% |

N=10 时间受微秒级短任务抖动影响，最小时间实际略优于 baseline。随 N 增大，packed key
产生小幅时间收益，但主要贡献是内存：N=14 节省约 320.5 MiB。

## 决策

**KEEP。**

- N=11–14 连续满足 RSS 降低 15% 的条件；
- N=13、14 的 RSS 降低超过 30%；
- N=14 时间改善 8.8%；
- 所有 tensor-level 与 count-level correctness gates 通过；
- support 和局域 tensor work 不变，收益可以明确归因于 hash key layout。

该 candidate 应合入下一轮 baseline。下一方向 E4 为 flat/robin-hood hash；若不引入新依赖，
可先比较 `HashMap` hasher，但必须注意 hash 随机性和碰撞安全。

原始数据：`experiments/e3_packed_u128/results.csv`。

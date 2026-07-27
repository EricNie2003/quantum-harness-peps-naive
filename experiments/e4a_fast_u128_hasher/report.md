# E4a：标准 HashMap 上的确定性 u128 hasher

## 预注册

- 分支：`codex/exp-fast-u128-hasher`
- baseline code commit：`dde47b7fad35e6d2cbd94422d2f70559ca80d883`
- candidate code commit：`31e43a21b14a760741b1be6fea3e2d1690f9e0e8`
- 单变量：标准 `HashMap` 的 `RandomState` 改为专门处理内部 `u128` key 的
  SplitMix64 风格 deterministic hasher；
- 不改变：map layout、packed key、局域 \(C\)、算术、support、ordering；
- keep：两个连续 N 的中位时间改善至少 15%；
- kill：N=13 仍没有稳定收益。

输入 key 完全由内部 exact contraction 生成，不接收不可信输入；HashMap 自身仍通过完整 key
比较处理碰撞，因此 hasher 变化不影响 exactness。

## 验证

全部 9 个 release 测试和 Clippy 通过。N=10–13 的 count、peak support、tensor work 和 RSS
均与 E3 baseline 一致。

| N | baseline median (s) | fast hasher median (s) | 变化 |
|---:|---:|---:|---:|
| 10 | 0.019144 | 0.015860 | -17.2% |
| 11 | 0.077946 | 0.091721 | +17.7% |
| 12 | 0.483586 | 0.482290 | -0.3% |
| 13 | 3.103225 | 3.167849 | +2.1% |

短任务波动较大且方向不一致。N=13 五次中位数没有收益，虽然最小样本较快，但预注册协议以
中位数为准。RSS 没有改变，因为容器 layout 未变。

## 决策

**REJECT。**

不运行 N=14，不合入主 baseline。该结果只否定“在标准 HashMap 上单独替换 hasher”；
它不否定后续真正改变 bucket/layout 的 flat hash 或 robin-hood 后端。

原始数据：`experiments/e4a_fast_u128_hasher/results.csv`。

# E56：C-certified last-7/last-8 terminal expansion

## 决策

**REJECT，N=18 early kill。** k=7 相对同 revision k=6 在 N=16/17
分别快 1.9%/3.8%，没有达到两档 5% keep gate；k=8 又回退到与 k=6
接近。按照预注册规则不运行 N=18。

| N | last-6 | last-7 | change | last-8 | change |
|---:|---:|---:|---:|---:|---:|
| 16 | 0.326326 s | 0.320211 s | -1.9% | 0.327585 s | +0.4% |
| 17 | 2.732082 s | 2.628886 s | -3.8% | 2.728144 s | -0.1% |

candidate `.text` 为 276,079 bytes，旧 last-6 binary 为 269,551 bytes，
增加 2.42%，远低于 30% code-size kill；clean release build 为
16.264 s。因此 E56 不是 code explosion 失败，而是 last-6 之后额外一层
call/base-case 删除只剩很小常数，k=8 的更大内联没有继续获益。

## 实现、PEPS fidelity 与 exactness

- code revision：`20d0533072fa094eaea824b98f03eabf6f37680f`；
- branch/worktree：`codex/exp-last78` / `.worktrees/e56-last78`；
- base：main `0194775`，production last-6 未修改；
- last-7/8 仅组合 `CertifiedSecViTailPlan` 已认证的 occupation=1
  explicit-C successor；每层仍计算完整合法 positions，terminal column
  v1 与 diagonal v2 不变；
- checked-u64 accumulation、coefficient limit 与 forced CRT replay保留；
- k=4..8、N=0..10 都与 generic explicit-C replay/known counts一致；
- full release suite 51 passed；format 和 clippy `-D warnings` 通过；
- formal N=16/17 的 count、tasks、recursive nodes、accepted C entries和
  support 对 k=6/7/8 完全一致。

## Benchmark protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- N=16/17：1 warmup + 5 repeats；每个结果另做 generic-C profile replay，
  replay 时间不计入 median；
- command：
  `cargo run --release --bin e47_last_k -- 16 17 5 1 2048 <k>`；
- RSS：Windows `PeakWorkingSet64` process high-water；包含 runtime、
  allocator 和 worker stacks，不代表 live heap/cache residency；
- section size：Rust toolchain bundled `llvm-size.exe`；
- raw CSV：
  - `benchmarks/e56_last78_ablation.csv`
  - `benchmarks/e56_last78_build.csv`

# E57：factorial-bounded deferred-overflow hot kernel

## 决策

**REJECT，N=18 early kill。** deferred-overflow 相对原 checked-last6 在
N=16 慢 0.3%，N=17 快 1.3%；两档收益都小于 2% kill gate。

| N | checked | deferred detection | wall change |
|---:|---:|---:|---:|
| 16 | 0.328149 s | 0.329216 s | +0.3% |
| 17 | 2.775337 s | 2.739968 s | -1.3% |

原实现的 `checked_add` overflow branch 在正常 count 上极易预测。
candidate 取消 recursive `Option` failure return 和逐 child coefficient-limit
filter，但 `overflowing_add/mul` 仍需生成 overflow bit，并把 sticky flag
沿每条 child dependency chain OR 回父层。实测两种成本基本互换，没有
显著缩短 99.98% 热核。

## exactness 与 PEPS fidelity

- code revision：`cd30c9a83d2e22d9be2928aeae8847eda0c15d09`；
- branch/worktree：`codex/exp-infallible-tail` /
  `.worktrees/e57-infallible`；
- base：main `6058aa0`；E56 rejected code 不在 baseline；
- candidate 没有使用 unchecked arithmetic 或接受 wrapping result：
  每个 add/mul 调用 `overflowing_*` 并累积 sticky flag，task/reduction
  边界发现 flag 就返回 `None`，由既有 exact CRT 完整 replay；
- 人工 `u64::MAX + 1` 明确置 overflow flag；coefficient limit=1 强制走
  原 checked path并完成 CRT Q(8)=92；
- N=0..12 candidate、checked-last6、generic explicit-C replay 与 known
  Q(N) 一致；count/tasks/nodes/accepted entries/support 完全相同；
- explicit B/C 17 entries、v0/v1/v2、C-derived successor 和 vertical
  orbit weights 均未改变；
- full release suite 52 passed；format 与 clippy通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- N=16/17：1 warmup + 5 repeats；generic-C profile replay不计入 median；
- command：
  `cargo run --release --bin e57_detected_tail -- 16 17 5 1 2048 <mode>`；
- RSS：Windows `PeakWorkingSet64` process high-water，包含 runtime、
  allocator 和 worker stacks；
- raw CSV：`benchmarks/e57_detected_tail_formal.csv`。

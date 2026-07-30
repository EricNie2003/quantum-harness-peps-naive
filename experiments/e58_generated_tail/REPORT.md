# E58：source-generated depth-specific scalar tail

## 决策

**REJECT，N=18 early kill。** noinline generated chain 在 N=16 慢 1.0%、
N=17 快 2.5%；fully-inline chain 在 N=16/17 快 0.9%/4.0%，仍没有达到
两档 5% keep gate。

| N | recursive last-6 | generated noinline | change | generated inline | change |
|---:|---:|---:|---:|---:|---:|
| 16 | 0.316316 s | 0.319609 s | +1.0% | 0.313383 s | -0.9% |
| 17 | 2.756190 s | 2.686464 s | -2.5% | 2.646709 s | -4.0% |

source macro 为 remaining depth 7--20 生成静态函数；task 入口只 dispatch
一次，此后每个 upper-tail node 不再重复检查 `remaining_rows==0/1/<=6`
或 decrement depth。N=17 的收益高于 N=16，符合上层动态判断随深度增加的
假设，但到正式 gate 仍不足。fully-inline `.text` 增 11.54%，noinline
增 2.39%，都未触发 40% code-size kill。

机制上，upper tail 只占 last-6 之上的较窄层；多数 accepted work仍发生在
已优化 terminal loops。删除动态 depth 判断只能影响少数上层 nodes，
无法复制 E47 展开最宽最后六层时的两位数收益。

## exactness 与 PEPS fidelity

- code revision：`57e05d1a052b1db0e45854eef5ce2058d09ceddb`；
- branch/worktree：`codex/exp-generated-depth-tail` /
  `.worktrees/e58-generated`；
- base：main `25a85d9`；E56/E57 rejected code 都不在 baseline；
- macro 只生成相同 `certified_tail_successor` 的固定深度调用链；
  positions、checked accumulation、last-6 certificate、column v1 和
  diagonal v2 均不变；
- generated depth 0--20 通过 N=0..12 checked-last6、generic C replay和
  known counts；depth 21 fail closed；
- count/tasks/nodes/accepted entries/support 对所有 shape完全一致；
- full default release suite 52 passed；inline feature targeted exactness、
  format 和两种 feature clippy均通过。

## Benchmark protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- N=16/17：1 warmup + 5 repeats；generic-C profile replay不计 median；
- control/noinline command：
  `cargo run --release --bin e58_generated_tail -- 16 17 5 1 2048 <mode>`；
- inline command 增加
  `--target-dir target/e58-inline --features e58-inline-generated`；
- section size：Rust toolchain bundled `llvm-size.exe`；
- RSS：Windows `PeakWorkingSet64` process high-water；
- raw CSV：
  - `benchmarks/e58_generated_tail_formal.csv`
  - `benchmarks/e58_generated_tail_sections.csv`

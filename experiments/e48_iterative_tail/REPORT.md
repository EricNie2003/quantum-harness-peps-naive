# E48：iterative fixed-stack C-tail machine

## 决策

**REJECT，不并入 production；按 kill gate 不运行 N=18。**

同 revision、同一批次的 1 warmup + 5 samples：

| N | E47 recursive last-6 | E48 fixed stack | reduction | gate |
|---:|---:|---:|---:|:---:|
| 16 | 0.322088 s | 0.319550 s | 0.8% | FAIL |
| 17 | 2.729689 s | 2.693935 s | 1.3% | FAIL |

两档均远低于 5% kill threshold，也没有 10% median / 15% p90 收益。
更早的相邻批次甚至显示 E48 在 N=17 慢约 15%，说明 1% 级差异完全
处于系统批次波动范围。继续运行 N=18 不能挽救被两档否定的机制。

## 实现、exactness 与 fidelity

- code revision：`af9c7cf`；
- branch/worktree：`codex/exp-iterative-tail` /
  `.worktrees/e48-iterative-tail`；
- base：main `9bf936b`（accepted E47）；
- arithmetic：E46 checked scalar-u64 / identical-sector CRT replay；
- terminal：剩余 6 行仍调用 E47 C-certified last-6 supernode。

候选为每个 direct sector 分配固定 `[u64;35]` arrays，保存 columns、
DR、DL 和 remaining positions；显式 depth/backtrack 取代剩余上层的
Rust recursion。每次 successor 仍调用 `certified_tail_successor`，
tasks、recursive nodes、accepted C entries 和最终 count 与 E47 完全
相同。

测试对 N=0..12 从初始 boundary 比较 iterative、recursive last-6、
generic checked-u128 C replay 和 known counts；同时检查 tail tasks 与
accepted entries 相同。explicit B/C、boundary vectors、forced arithmetic
promotion tests 全部保留。没有 DFS 调用或近似。

## 失败机制

E47 已把最宽、调用最频繁的六层变成 inline supernode，剩余 recursion
只在相对窄的上层。固定栈仍需：

- 初始化四组 35-entry arrays；
- 每次 descent 写回三个 masks 和 positions；
- 用动态 depth 索引 reload；
- 显式执行 backtrack state machine。

编译器对普通递归的 state 传参和 return reduction 已能较好保存在寄存器/
紧凑 stack frame；E48 的数组 traffic 抵消了 call-frame 节省。RSS
基本相同，故也没有内存理由保留它。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- release、thin LTO、codegen-units=1、8 threads；
- candidate/control 均 1 warmup + 5 samples，另有 generic C metrics
  replay；
- commands：
  - `cargo run --release --bin e48_iterative_tail -- 16 17 5 1 2048`
  - `cargo run --release --bin e47_last_k -- 16 17 5 1 2048 6`
- memory：Windows `PeakWorkingSet64` high-water mark，包含 runtime、
  worker stacks 和 profile replay，不等于 live heap。

Raw data：`benchmarks/e48_iterative_release.csv`。

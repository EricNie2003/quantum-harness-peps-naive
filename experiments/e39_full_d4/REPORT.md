# E39：recursive sectors 的 full-D4 canonical augmentation

## 决策

**REJECT as production；KEEP diagnostic implementation on experiment branch。**

| N | none nodes | vertical nodes | full nodes | full vs vertical | vertical time | full time |
|---:|---:|---:|---:|---:|---:|---:|
| 12 | 856,189 | 428,095 | 420,428 | -1.79% | 0.000900 s | 0.000893 s |
| 13 | 4,674,890 | 2,337,446 | 2,299,584 | -1.62% | 0.004285 s | 0.004386 s |
| 14 | 27,358,553 | 13,679,277 | 13,492,067 | -1.37% | 0.02377 s | 0.02370 s |
| 15 | 171,129,072 | 85,564,537 | 84,463,165 | -1.29% | 0.1505 s | 0.1553 s |

Full 相对 none 减少约 50.6% nodes，但约 50% 已由 vertical reflection
产生；其余六个非平凡 actions 相对 vertical 只再减少 1.3--1.8%，触发
预注册的 `<15% actual node reduction` kill gate。N=15 wall 反而慢 3.2%。
因此不能把“full 比 none 减半”误报成 full-D4 的增量收益。

## 正确性、D4 与 PEPS fidelity

- code revision：`9c9c892`；
- branch/worktree：`codex/exp-full-d4` / `.worktrees/e39-full-d4`；
- base：main `2c41e11`（accepted E38）；
- successor：仍由 explicit 17-entry `C` 编译出的
  `RecursiveTailRelation` 生成；
- arithmetic：checked `u128` local multiplication、branch reduction 和
  orbit-weight join。

实现显式使用仓库已有、已测试的八个 coordinate actions：
identity、90/180/270 rotations、vertical/horizontal/main-diagonal/
anti-diagonal reflections。对应 channel-family permutation 和 line
endpoint orientation 的 tensor-invariance tests 继续通过。

对深度 k 的 partial placement，某 action 只有在已放置 queens 的变换
恰好覆盖 rows `[0,k)` 时才可比较。实现据此使用：

- vertical：总可比较，但词典序通常由第一行决定；
- rotate90/main-diagonal：仅当已占 columns 恰为 `[0,k)`；
- rotate270/anti-diagonal：仅当已占 columns 恰为 `[N-k,N)`；
- rotate180/horizontal：仅在完整深度。

只有变换前缀严格更小时才剪枝。完整 canonical representative 的 orbit
size 由 8/stabilizer 精确得到 1/2/4/8。N=15 有 310 个 size-4 和
284,743 个 size-8 orbits，重构
`310*4 + 284743*8 = 2,279,184`。

none/vertical/full 对 N=0--10 均验证 exact count；每种模式还验证
`sum(orbit_size * representatives)=Q(N)`。44 个 release tests 与
Clippy 通过。

## 消融解释

最初实现每个 node 都重新扫描 vertical-transformed prefix，N=15 full
为 0.195 s。消融发现第一 queen 非 central 时，vertical lex order 已永久
决定；跳过后续冗余比较后降到 0.155 s。这个优化保留在实验分支，说明
先前的 overhead 确实来自 canonical checking，而非 C transition。

优化后 full 仍做 10,744,138 次 canonical comparisons，剪掉 1,040,825
个 partial branches；与 vertical 相比仅少 1,101,372 nodes，却增加
完整 orbit/stabilizer 和低/高 column-set 检查。额外 D4 actions 的可比较性
通常到很晚才出现，无法抵销开销。这正是 row-prefix 几何与 full-board D4
不匹配的实测结果。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release/thin-LTO；`RAYON_NUM_THREADS=8`；
- split depth：3；N=12--15；
- commands：
  - `cargo run --release --bin e39_recursive_d4 -- 12 15 3 none 7`
  - `cargo run --release --bin e39_recursive_d4 -- 12 15 3 vertical 7`
  - `cargo run --release --bin e39_recursive_d4 -- 12 15 3 full 7`
- repetition：每档七次，取 median/min；
- memory：Windows `PeakWorkingSet64`，进程高水位，包含 allocator、线程栈
  和 runtime，不是精确 live heap。

Raw data：`benchmarks/e39_recursive_d4_release.csv`。

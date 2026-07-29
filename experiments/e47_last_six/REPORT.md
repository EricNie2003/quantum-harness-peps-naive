# E47：C-certified last-5/last-6 terminal supernode

## 决策

**KEEP last-6，作为新的 production tail microkernel。**

| N | E46 last-4 | E47 last-6 | reduction | DFS | E47/DFS |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.009752 s | 0.008303 s | 14.9% | 0.010550 s | **0.787x** |
| 15 | 0.063901 s | 0.051086 s | 20.1% | 0.068454 s | **0.746x** |
| 16 | 0.400718 s | 0.314490 s | 21.5% | 0.399135 s | **0.788x** |
| 17 | 2.878197 s | 2.341897 s | 18.6% | 2.999933 s | **0.781x** |
| 18 | 20.076193 s | 19.027543 s | 5.2% | 25.342626 s | **0.751x** |

N=16/17 连续两档超过 8% keep gate；N=18 仍有 5.2% 增益。当前
N=14--18 每个正式点均快于同检查点 DFS comparator 约 21--25%。

## 实现与 PEPS fidelity

- code revision：`e1c2e5b`；
- branch/worktree：`codex/exp-last-six` / `.worktrees/e47-last-six`；
- base：main `7e8dbd4`（accepted E46）；
- arithmetic/backend：E46 certified scalar-u64，checked failure仍 replay
  identical direct sectors with CRT；
- contraction association：只改变剩余 5/6 rows 的 terminal grouping。

last-5 对每个由 certified occupied C entry 匹配的第一步，调用已认证
last-4；last-6 同理组合 last-5。`#[inline(always)]` 使编译器能把这两层
展开为一个 terminal supernode。所有 successor 仍调用
`certified_tail_successor`，最终 column v1 / diagonal v2 条件不变。
只有 `CertifiedSecViTailPlan` 确认 explicit C 的四通道 0→1、row
v0→v1、value=1 后，上层 API 才到达这些 functions。

测试对 N=0..10、k=4/5/6 从初始 boundary 运行完整 contraction，并与
generic checked-u128 recursive C replay/known counts 对照。无 lookup
table、无 known subtree count、无 DFS 调用。k=7 被 API fail closed。

## k=4/5/6 消融和机制

| N | last-4 | last-5 | last-6 | last-6 vs last-4 |
|---:|---:|---:|---:|---:|
| 16 | 0.400718 s | 0.329904 s | **0.314490 s** | -21.5% |
| 17 | 2.878197 s | 2.414502 s | **2.341897 s** | -18.6% |

三者的 tasks、recursive nodes、accepted C entries 完全相同。收益来自
删除最后两层的 recursive calls、重复 remaining-row dispatch、base
case 与 mask setup；不是剪枝或漏计。last-5 已取得大部分收益，last-6
仍稳定再降 4.7%/3.0%。

新 worktree 的首次完整 release compile（含 dependencies）为 32.51 s；
E46 对应为 33.86 s。最终 binaries 为 E47 366,592 bytes、E46
369,664 bytes；CLI 字段不同，因此只用于排除明显 code-size explosion，
不把 3 KB 差异解释为 library size 收益。未触发预注册的 25% size/compile
回退 gate。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；8 threads；
- ablation：1 warmup + 5 samples；formal N=14--17：3+21；
  N=18：1+3；每档另有 generic C metrics replay；
- commands：
  - `cargo run --release --bin e47_last_k -- 16 17 5 1 2048 {4,5,6}`
  - `cargo run --release --bin e47_last_k -- 14 15 21 3 512 6`
  - `cargo run --release --bin e47_last_k -- 16 17 21 3 2048 6`
  - `cargo run --release --bin e47_last_k -- 18 18 3 1 2048 6`
- memory：Windows `PeakWorkingSet64` process high-water mark；包含
  runtime/worker stacks/profile replay，不等于 live heap。

Raw data：

- `benchmarks/e47_last_k_ablation.csv`
- `benchmarks/e47_last_six_release.csv`
- `benchmarks/e47_controls.csv`

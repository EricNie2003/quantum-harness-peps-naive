# E15：exact finite-field boundary flattening rank

## 预注册与定义

- branch：`codex/exp-finite-field-rank`；
- worktree：`.worktrees/e15-finite-field-rank`；
- baseline：`a45a27f`；
- candidate：`dda0dfaa36bec4ad5aefa9935e7c32867b3370f6`；
- keep gate：至少两个 N 的 rank/support `<=0.5`，且 rank growth slope 比 support 低至少
  15%；
- kill gate：rank 接近 support，立即拒绝 exact MPS 主线。

对 production D4 contraction 的 peak-support layer，把 packed boundary 的三族 bits
`(column, diag_dr, diag_dl)` 各按棋盘左/右半列分组，形成 coefficient matrix
\(M_{L,R}\)。矩阵元素来自 exact PEPS boundary coefficient。

分别在两个素域

\[
\mathbb F_{1\,000\,000\,007},\qquad
\mathbb F_{1\,000\,000\,009}
\]

上做 sparse Gaussian elimination。所有乘法用 `u128` 中间值取模；pivot inverse 用
Fermat exponentiation。两个域 rank 不一致会使 binary 失败。它是 exact modular rank
diagnostic，不使用浮点 SVD、阈值或舍入。

测试含已知 rank-1/rank-2 矩阵和 N=1--7 两素域稳定性；全部 28 项 release tests、Clippy
和格式检查通过。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；Windows；
- rustc 1.94.0；release/thin-LTO；1 thread；
- 命令：
  - `cargo run --release --bin e15_rank_diagnostic -- 5 10`
  - `cargo run --release --bin e15_rank_diagnostic -- 11 12`
  - `cargo run --release --bin e15_rank_diagnostic -- 13 13`
- 每点一次诊断；rank 只依赖 exact matrix，不将 wall-time 当稳定 microbenchmark；
- RSS：Windows `PeakWorkingSetSize`。同一命令的 N 递增，历史高水位含 allocator retained
  pages；
- local tensor compile 逐项检查显式 C 的 17 entries；row candidate/matched work 与
  production D4 layer construction 一并记录。

## 结果

| N | peak row | support | left patterns | right patterns | exact rank | rank/support | elapsed (s) | RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 8 | 5 | 272 | 203 | 191 | 163 | 59.93% | 0.0005 | 5.23 |
| 9 | 6 | 1,210 | 426 | 806 | 396 | 32.73% | 0.0015 | 5.39 |
| 10 | 7 | 4,510 | 1,777 | 1,778 | 1,370 | 30.38% | 0.0048 | 6.30 |
| 11 | 8 | 22,253 | 2,767 | 8,719 | 2,484 | 11.16% | 0.0213 | 8.69 |
| 12 | 9 | 98,939 | 12,713 | 13,905 | 8,334 | 8.42% | 1.3302 | 34.05 |
| 13 | 9 | 541,745 | 22,872 | 97,180 | 20,443 | 3.77% | 11.0167 | 246.87 |

两个素域在所有 N 完全同 rank。N=10--13：

- support growth：4.93x、4.45x、5.48x；
- rank growth：1.81x、3.35x、2.45x；
- 每一步 rank growth 都至少比对应 support growth 低 24%，明显超过 15% gate。

N=13 rank/support 只有 3.77%，与 E14 future-class/support 10.56% 一起说明 boundary tensor
既有非线性 future equivalence，也有更强的 exact linear low-rank structure。

## 性能限制

当前 elimination 是诊断，不是 exact MPS solver：

- 它先构造完整 sparse boundary，不能节省首次 contraction；
- N=13 elimination fill-in 的 peak row nnz 约 9,600，wall time 11.0 s，远慢于 direct D4
  0.264 s；
- 单个 prime 的低 rank 不足以重构整数 Q(N)；exact solver 需要模数乘积界、CRT、冗余
  prime 校验，并在每次 row apply 后维持 certified rank factorization；
- 不能从两个 prime 的一致性推出 characteristic-zero rank 的无条件证明，只能把它视为
  极强候选证据。对求解正确性仍需足够 CRT 模数与独立验证。

## 决策

**KEEP 作为 exact-low-rank 研究方向；不合并当前 Gaussian diagnostic 为默认求解器。**

E15 大幅通过 rank ratio 与 slope gate，推翻了“exact rank 很可能接近 support”的保守假设。
下一轮应优先原型化 row-transfer 后的有限域 rank-factorized boundary apply，并把：

1. rank growth；
2. factorization/update wall time；
3. CRT prime 数和整数界；
4. D4、E11 sparse iterator 的交互

作为联合 gate。若 update 仍需先 materialize full support，则不能声称生产收益。

原始数据：

- `benchmarks/e15_finite_field_rank_release.csv`
- `experiments/e15_finite_field_rank/results.csv`

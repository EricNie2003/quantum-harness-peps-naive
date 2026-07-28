# E35：certified row-frontier distinguishability audit

## 决策

**KEEP 作为确定性结构证书；REJECT 作为 production solver。**

| N | reachable peak | certified class lower bound | ratio | audit time | RSS |
|---:|---:|---:|---:|---:|---:|
| 10 | 4,510 | 735 | 16.30% | 0.0117 s | 7.4 MB |
| 11 | 22,253 | 3,462 | 15.56% | 0.0755 s | 13.5 MB |
| 12 | 98,939 | 14,570 | 14.73% | 0.340 s | 40.3 MB |
| 13 | 541,745 | 57,215 | 10.56% | 2.869 s | 197.4 MB |
| 14 | 2,847,130 | 313,373 | 11.01% | 17.821 s | 959.3 MB |

这认证了：在当前 D4 top-row slicing 后的逐行 exact weighted
transfer 中，任何保持完整未来 weighted behavior 的确定性 quotient，
在某一层至少要有上述数量的 classes。N=10--14 的 log-linear fit：

- reachable support：base 4.997，R²=0.99950；
- certified classes：base 4.444，R²=0.99885。

classes 明显少于 concrete support，但仍在该窗口内指数增长。它说明
“事后 exact quotient 有 6--10x 压缩”是真的，也说明仅靠把所有
concrete states 先生成再合并不能解决规模问题。

## 证书为什么不是概率结论

每个 state 的 exact signature 是按 target future-class 排序的完整
`(class_id, checked-u128 multiplicity)` vector。构建过程先用两个
64-bit 素数

`18446744073709551557`、`18446744073709551533`

计算指纹并分桶：

- 指纹不同，则模剩余不同本身就是 exact inequality witness；
- 两个指纹相同，仍逐 vector 做确定性比较；只有 vector 完全相等才
  合并；
- 若同指纹但 vector 不同，记录 exact collision witness。本次
  N=10--14 为 0，但正确性不依赖它为 0；
- 构建后从 explicit-`C` successor generator 重新生成**每个 concrete
  state** 的 signature，与所属 class 的完整 vector 比较，再用
  checked u128 从 terminal `v1/v2` acceptance 反向 replay Q(N)。

因此 hash collision 不能制造错误合并。N=14 分别做了 12,359,522
forward、signature 和 witness-replay transitions，并完成
10,233,002 次同指纹 exact comparisons；Q(14)=365,596。
两个模数另用 deterministic 64-bit Miller--Rabin bases 验证为素数。

## PEPS fidelity 与下界适用范围

- code revision：`7230d6b`；
- branch/worktree：`codex/exp-frontier-audit` /
  `.worktrees/e35-frontier-audit`；
- base：main `f0a7f9a`；
- successor relation：`CompiledRowOperator` 由 explicit 17-entry
  `C` 机械编译；top-row D4 orbit weights 与 production 相同；
- terminal：完整 column `v1` acceptance，row/diagonal boundary
  已包含在每个 C-derived row apply；
- arithmetic：signature、class value 和 replay 均 checked u128。

这个 lower bound **只约束当前 row ordering 下、以 exact weighted
bisimulation 合并 frontier states 的表示**。它不约束：

- 不同 contraction path；
- 允许线性组合的 exact MPS/finite-field rank 表示；
- 能在 local C apply 同时原生生成 symbolic classes 的新算法。

所以它不是“PEPS 不可能快过 DFS”的普适证明。相反，class fit base
4.44 低于既有 DFS node window 约 5.8，说明若能跳过 concrete
prebuild，理论上仍有改变竞争关系的空间；当前缺失的是在线
canonical symbolic apply，而不是 future equivalence 不存在。

## 为什么不进 production

认证需要先枚举全部 reachable concrete graph，再做 signature pass，
最后做 witness replay。N=14 audit 17.8 s，而 E32 production PEPS
约 0.135 s；证书用于诊断，不用于单次计数。N=14 的 313k classes
也远高于能直接与 DFS 竞争的规模，且没有给出无需 concrete states
即可构造 class 的局域规则。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- release/thin-LTO，单线程 audit；
- commands：
  - `cargo run --release --bin e35_frontier_audit -- 10 13`
  - `cargo run --release --bin e35_frontier_audit -- 14 14`
- one deterministic run per N；work/support 指标不受时序噪声；
- RSS：Windows `PeakWorkingSet64`，N=10--13 在同一进程递增，
  N=14 独立进程；包含 allocator/runtime retained pages。

Raw data：`benchmarks/e35_frontier_audit_release.csv` 与
`benchmarks/e35_frontier_audit_layers.csv`。

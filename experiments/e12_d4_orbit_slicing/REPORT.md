# E12：完整 D4 作用验证与 cut-preserving orbit contraction

## 预注册与 revision

- 分支：`codex/exp-d4-orbit-slicing`；
- worktree：`.worktrees/e12-d4-orbit-slicing`；
- baseline commit：`5f3c36639871d0f49a87164c7a260e4f22ee0284`；
- candidate code commit：`fdebf2097a409f94db24dcd358204c9832ce67f8`；
- 假设：完整棋盘具有 D4 对称性，但 top-down interior row cut 的稳定子只有
  `{identity, vertical reflection}`。按该稳定子的首行 occupied-tensor 轨道收缩，可将后续
  support、候选流量和 RSS 接近减半，而不对 fixed orbit 盲乘；
- keep gate：N=13--15 aggregate work 至少下降 35%，runtime 至少 1.5x；
- kill gate：只在最终答案盲乘、fixed orbit 处理不正确，或收益低于 20%。

## D4、局域张量与边界约定

代码显式实现 D4 的 8 个作用：恒等、90/180/270 度旋转、水平/垂直反射、主/副对角
反射，并实现它们对 row、column、down-right、down-left 四类 constraint channel 的置换。

映射一条 constraint line 时，允许将新线重新定向，使映射后的 start endpoint 仍接 `v0`，
end endpoint 仍接 row/column 的 `v1` 或 diagonal 的 `v2`。因此同一 channel 的 `in/out`
成对移动；没有只翻 virtual direction 而不翻 boundary endpoint。

tensor-level 测试对每个 D4 元素和显式 `B`/`C` 的每个 entry 做 channel-pair permutation，
验证变换后 entry 仍存在且 `alpha`/coefficient 不变。另有测试验证：

- 8 个坐标作用是不同的双射，并保持一个独立 N=8 solution 的 row/column/diagonal 非攻击性；
- 每个 interior top-row cut 的稳定子恰为 `{identity, vertical reflection}`；
- 首行 orbit representative 数为 `ceil(N/2)`；偶数 N 全部 multiplicity 2，奇数 N 有一个
  center fixed point，multiplicity 1；
- N=0--11 对称/非对称 serial contraction exact count 一致且 support 下降；
- N=0--10 的 1/2/4-thread 对称 contraction 与 serial 对称 contraction 一致；
- N=0--10 的 dense、E11-only、D4-only、E11+D4 四项消融 count 一致。

完整 release tests 26 项、Clippy `-D warnings` 和格式检查通过。B/C 17-entry、v0/v1/v2、
sitewise oracle、brute-force oracle、known Q(N) 等既有 gate 全部保留。系数使用 checked
`u128`。

## 收缩设计

首行从显式 `C` 的唯一 occupied entry 生成 N 个 tensor terms。垂直反射将 column `c` 与
`N-1-c` 配对，只保留 `c <= N-1-c`：

- 二元素 orbit 的 tensor coefficient 精确乘 2；
- 奇数 N 的 center fixed point coefficient 乘 1；
- 随后仍逐行收缩同一个 compiled `C` operator，并执行原 bottom column `v1` 与 diagonal
  `v2` contraction。

这是 projected tensor slice 的加权和，不是在最后答案上乘 2。其余六个 D4 元素会把
top-down row task 映到 bottom-up/column/反向 task，不能直接用于同一 interior cut；
本实验用它们验证 kernel/tensor 等价，但不虚报额外 4x。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- OS：Windows；
- 编译器：Rust 1.94.0，release/thin-LTO；
- threads：serial 1；parallel 16；
- 命令：`cargo run --release --bin e12_d4_orbits -- BACKEND THREADS 13 15 REPEATS`；
- backend：`dense-serial`、`d4-serial`、`dense-parallel`、`d4-parallel`、
  `sparse-serial`、`d4-sparse-serial`、`sparse-parallel`、`d4-sparse-parallel`；
- 重复：dense serial 3 次，其余 5 次，报告中位数/最小值；每个 backend 独立进程；
- RSS：Windows `GetProcessMemoryInfo` 的 `PeakWorkingSetSize`，包含 allocator、运行时和
  进程内前序 repeat retained pages，不等同于 live tensor payload。

## D4-only 结果

### Serial

| N | dense (s) | D4 (s) | speedup | support reduction | work reduction | dense RSS (MiB) | D4 RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 13 | 0.465995 | 0.255967 | 1.82x | 44.63% | 42.92% | 101.7 | 55.3 |
| 14 | 2.708757 | 1.406227 | 1.93x | 48.04% | 46.43% | 443.5 | 226.6 |
| 15 | 17.071132 | 9.578511 | 1.78x | 43.40% | 42.14% | 2,884.4 | 1,633.2 |

这里 work reduction 按 `row_operator_candidates` 计算。accepted transitions 的下降分别为
44.45%、48.20%、44.06%，也超过 35% gate。

### 16 threads

| N | dense (s) | D4 (s) | speedup | dense RSS (MiB) | D4 RSS (MiB) |
|---:|---:|---:|---:|---:|---:|
| 13 | 0.116062 | 0.068293 | 1.70x | 89.7 | 61.9 |
| 14 | 0.632885 | 0.320829 | 1.97x | 389.1 | 209.5 |
| 15 | 3.744197 | 2.054081 | 1.82x | 2,168.0 | 1,220.3 |

所有行的 count 均等于 known Q(N)，并由同 revision 的 unsymmetrized contraction 独立核验。

## E11 × E12 消融

### Serial

| N | dense | E11 only | D4 only | E11+D4 | D4 marginal | E11 marginal after D4 |
|---:|---:|---:|---:|---:|---:|---:|
| 13 | 0.465995 | 0.343972 | 0.255967 | 0.179209 | 1.82x | 1.43x |
| 14 | 2.708757 | 1.999303 | 1.406227 | 0.986010 | 1.93x | 1.43x |
| 15 | 17.071132 | 12.543836 | 9.578511 | 6.777653 | 1.78x | 1.41x |

### 16 threads

| N | dense | E11 only | D4 only | E11+D4 | E11 marginal after D4 |
|---:|---:|---:|---:|---:|---:|
| 13 | 0.116062 | 0.112780 | 0.068293 | 0.063385 | 1.08x |
| 14 | 0.632885 | 0.576997 | 0.320829 | 0.303596 | 1.06x |
| 15 | 3.744197 | 3.545659 | 2.054081 | 1.919514 | 1.07x |

E11 在 D4 后仍没有达到它自己的 1.5x keep gate，parallel 边际更只有 6--8%。因此不推翻
E11 的 REJECT：它可保留为实验 backend，但默认/合并候选应是 D4-only。消融同时表明两者
没有严重负交互；主要瓶颈仍是 support materialization/sort/merge。

## 与 DFS 的差距

同硬件现有 scaling benchmark：

- N=15 serial DFS：0.499690 s；D4-only PEPS：9.578511 s，仍慢 19.2x；
- N=15 16-thread DFS：0.036802 s；D4-only PEPS：2.054081 s，仍慢 55.8x。

即使选择实验性的 E11+D4，分别仍慢约 13.6x 和 52.2x。E12 明显降低常数、support 和
内存，但没有改变约 5x/N 的 support slope，尚未超过 DFS。

## 决策

**KEEP D4-only；REJECT E11 作为默认交互项。**

E12 达到三档 runtime `>=1.5x`、work reduction `>=35%`、exact orbit/fixed-point 和
unsymmetrized validation 全部 gate。下一步 E13 必须追求 actual sparse separator/support
slope 的下降；仅得到 dense FLOP/width 更优的 contraction path 不足以 KEEP。

原始数据：

- `experiments/e12_d4_orbit_slicing/results.csv`
- `benchmarks/e12_d4_orbit_slicing_release.csv`

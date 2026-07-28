# E34：corner/diamond explicit-C contraction path

## 决策

**REJECT top-left diamond ordering；KEEP generic explicit-C frontier 仅作
ordering correctness oracle，不进入 production。**

| N | row-major generic support | diamond support | ratio | E32 row-cut support | diamond / E32 |
|---:|---:|---:|---:|---:|---:|
| 10 | 15,392 | 44,489 | 2.89x | 4,510 | 9.86x |
| 11 | 71,868 | 229,812 | 3.20x | 22,253 | 10.33x |
| 12 | 358,964 | 1,309,296 | 3.65x | 98,939 | 13.23x |

N=10--12 连续三档 active support 超过同一 generic engine 的
row-major 2x，并且差距扩大，明确触发 kill gate。diamond 的
explicit-entry examinations 也分别是 row-major 的 2.55x、2.85x、
3.17x；N=12 wall time 8.31 s 对 1.95 s。

## Cut interface 与实现

`TopLeftDiamond` 按 `(row + column, row)` 收缩。完成 shell `k` 后：

```text
contracted: r + c <= k              uncontracted: r + c > k

X X X X | . . .
X X X | . . .         cut 上保存每条 row / column /
X X | . . .           down-right / down-left 内部虚键的 0/1
X | . . .
| . . .
```

每条内部 virtual bond 获得唯一 edge id。frontier key 不是皇后
placement recurrence，而是当前 cut 上所有 edge 的逐 bond 二进制
赋值。收缩一个 site 时，对每个 frontier state **扫描 explicit
`SiteTensorC::sec_vi()` 的全部 17 entries**：

- 已收缩邻居的腿必须匹配 frontier bit；
- 未收缩邻居的腿写入新 frontier bit；
- 不接内部 edge 的 endpoint 直接测试边界向量允许集合：
  `v0={0}`、row/column `v1={1}`、diagonal `v2={0,1}`；
- 相同新 frontier assignment 用 checked u128 相加。

因此它是 rank-8 `C` virtual-bond contraction，不是手写 N-Queens
递推。row-major 与 diamond 使用完全相同的 engine，只替换 site
ordering。

## Fidelity、D4 与验证

- code revision：`4b9aad9`；
- branch/worktree：`codex/exp-corner-frontier` /
  `.worktrees/e34-corner-frontier`；
- base：main `1528cde`（E32 production + E33 report）；
- arithmetic：checked u128；
- support safety cap：5,000,000（本次 N<=12 全部完成，未触发）。

完整的 `r+c<=k` top-left triangle 在主对角反射下不变，所以 shell
边界的 stabilizer 是 `{identity, main-diagonal reflection}`；90°
旋转和其余反射把它送到其他 corner sector，不能在单 sector 内直接
canonicalize。按 `row` 逐 site 的 shell 内 tie-break 暂时只保留
identity，完成整 shell 后才恢复二元 stabilizer。这个复杂性意味着
即使加入合法 D4 orbit bookkeeping，最多只能在 shell cuts 获得接近
2x，而当前 support 相对 production 已差 10--13x，无法逆转结论。

37 个 release tests 和 Clippy 通过。新增测试在 N=0--6 对两个
ordering 验证 known Q(N)；benchmark 又在 N=7--12 全部得到正确
Q(N)。generic engine 的边界方向也由两个 ordering 独立交叉验证。

## 失败机制

diamond cut 同时横切更多 row、column 和两族 diagonal lines：
N=12 peak open bonds 为 44，而 row-major 为 35。更关键的是这些
bond assignments 在精确约束下仍保持大量可区分组合，support
增长快于 row-major。几何上更“圆”没有转化成更低的 exact sparse
rank；它只是把 row/column 的长程状态与 diagonal 状态同时暴露。

这也否定了“对称 cut 天然更适合完整 D4”的简单假设：top-left
triangle 实际只保留 D4 的二阶子群，且得到该对称性之前要经历
shell 内的非对称中间 cuts。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- release/thin-LTO；generic engine 单线程；E32 control 8 threads；
- commands：
  - `cargo run --release --bin e34_corner_frontier -- 7 10 5000000`
  - `cargo run --release --bin e34_corner_frontier -- 11 12 5000000`
  - `RAYON_NUM_THREADS=8 cargo run --release --bin e32_u64_promotion -- 256 10 12 1`
- one run per ordering/N；主要 gate 是 deterministic support/work，
  wall time 仅作辅助。
- RSS：Windows `PeakWorkingSet64`，包含 allocator 与 runtime；
  generic orders 在同一进程顺序执行，因此后运行者的 high-water
  mark 可能继承前者，不能把 RSS ratio 单独当算法 live-memory ratio。

Raw data：`benchmarks/e34_corner_frontier_release.csv` 与
`benchmarks/e34_production_row_control_release.csv`。

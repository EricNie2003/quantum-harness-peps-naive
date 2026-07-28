# E7：exact weighted ADD/ZDD boundary representation 原型

## 预注册

- 分支：`codex/exp-boundary-diagram`
- baseline：E6 compiled row operator，commit `97dab3e6add204d6f2054b08777d7f8c70f8572c`
- prototype commit：`930e658fab34b7e6616f7ba46c4a16c6357a559f`
- 假设：开放 virtual-boundary coefficient function 可通过共享未来相同子图压缩；
- 变量顺序：三个 mask grouped，或同一棋盘列的 column/两 diagonal bits interleaved；
- reduction：weighted ADD 与 zero-suppressed weighted ZDD；
- keep：连续两个 N 的节点数比 explicit support 少至少 30%，且 build/apply 不超过
  当前 contraction 的 2x；
- kill：N=10、11 主要层节点数仍大于 support，或构建成本远超 2x。

## Exactness

diagram 的输入是 E6 exact contraction 每层的完整
`PackedBoundary -> u128 coefficient` map。终端节点保存 exact `u128` coefficient，缺失分支
为精确零。

测试对一个 12-bit 稀疏 weighted function 枚举全部 4096 个 key，验证 grouped/interleaved
和 ADD/ZDD 四种 diagram 的 evaluation 与原 map 完全一致；另验证 profiler 保持
\(Q(8)=92\)。全套 17 个 release tests 及 Clippy 通过。

该原型仅在 explicit boundary 生成后构图，用于回答“是否存在足够结构压缩”；它不是最终
diagram-native contraction，因此没有被标为主 PEPS backend。

## 结果

### Peak layer

| N | peak support | best diagram | best nodes | nodes/support | profile total (s) | E6 contraction (s) |
|---:|---:|---|---:|---:|---:|---:|
| 10 | 8,838 | interleaved ZDD | 13,103 | 1.48 | 0.0664 | 0.00433 |
| 11 | 39,307 | interleaved ZDD | 48,302 | 1.23 | 0.3073 | 0.02033 |

profile total 同时构建四种 diagram，约为 E6 contraction 的 15x。N=11 peak RSS 为
21.06 MiB，而 E6 N=11 为约 11.36 MiB。

weighted ZDD 明显优于 ADD，interleaved order 在中间层通常优于 grouped order，但最佳组合
仍未低于 explicit state count。即便按一个 diagram node 与一个 packed-map entry 等大这一
过度乐观假设，内存也没有收益；实际构建还需要 unique tables。

系数种类很少（peak 层只有 4–5 种），说明失败不是 terminal value 太分散，而是 boundary
bit patterns 的子函数共享不足。ZDD 节点数从 N=10 的 1.48× support 改善到 N=11 的
1.23×，但要在下一个尺寸突然达到 0.70× 才能过 gate，没有证据支持这种跃迁。

## 决策

**DIAGNOSTIC_ONLY / REJECT as backend。**

- 两个连续 N 的最佳节点数都大于 explicit support；
- 构建成本约为 contraction 的 15x，远超 2x gate；
- RSS 也更高；
- 因而不运行 N=12，不实现 diagram-native apply，不合入 main。

这个负结果反驳了“普通固定变量顺序 ZDD/ADD 可直接压缩当前 boundary”的假设。未来若重访
decision diagrams，必须先提出动态变量顺序、按 geometry 分层或完全不同的 quotient，
不能重复本原型。

原始数据：`experiments/e7_boundary_diagram/results.csv`。

# Five-direction review 01：E1–E5 后的强制研究复盘

日期：2026-07-28  
复盘基线：`main`，E1、E3、E5a 已合入  
触发条件：已经完成五个不同优化方向 E1、E2、E3、E4a、E5a

## 1. 当前结论

第一轮优化把 N=14 的严格 Sec. VI PEPS contraction 从 25.1915 s 降到
11.0897 s（2.27x），peak RSS 从 986.34 MiB 降到 666.17 MiB（-32.5%）。
这些是有价值的工程收益，但没有任何一个方向降低 peak support：

\[
S_{\max}(14)=5\,479\,934.
\]

同硬件优化 DFS bitmask comparator 的 N=14 单线程中位数为 0.0703587 s，
16 线程中位数为 0.0057339 s。因此当前 PEPS：

- 比单线程 DFS 慢约 157.6x；
- 比 16 线程 DFS 慢约 1933.9x；
- peak RSS 约为单线程 DFS 的 128x。

这个差距不能通过继续调 hasher、reserve 或小对象分配来消除。原计划对容器级常数优化的
优先级过高；下一阶段必须把研究问题改为：

> 如何从显式 \(C\) 张量机械导出更粗粒度的 exact operator，并最终避免显式物化数百万个
> 几乎不合并的 boundary states？

## 2. 五个方向的机制归因

| 方向 | 决策 | 观察 | 机制解释 |
|---|---|---|---|
| E1：incoming-signature 索引 \(C\) | KEEP | N=14 1.30x；检查项 -93.8% | 删除对 17 个稀疏项的重复线性过滤；纯局域计算收益 |
| E2：按 input support 预留 HashMap | REJECT | N=13 仅快 3.4%，RSS 无稳定下降 | map 最终容量由输出 support 决定；预留只改变扩容时机，不能减少 entry 数 |
| E3：`u128` packed boundary key | KEEP | N=14 RSS -32.5%，时间 +1.10x | key 从 24 B 降到 16 B，改善 bytes/state 和 cache；不改变状态数 |
| E4a：确定性 u128 hasher | REJECT | N=13 慢 2.1%，RSS 不变 | hash 计算不是主瓶颈；额外 mix 不优于标准实现，容器 layout 未变 |
| E5a：复用 partial buffers | KEEP | N=14 1.59x | 删除每个 parent、每个 site 的 Vec 分配；局域路径未变 |

原始假设中，E1、E3、E5a 对“局域扫描、entry 大小、分配次数”的判断成立。E2、E4a
失败也说明当前主要成本不在 rehash 次数或 hasher 本身，而在：

1. 显式 boundary support；
2. 同时存活的 input/output hash tables；
3. 对每个 boundary 重复执行完整行局域 contraction；
4. 低 merge ratio 下的随机内存访问。

## 3. 为什么当前 sparse-map 路线难以超过 DFS

N=14 最重层附近：

- row 10：5,942,914 completed terms → 5,479,934 unique states，merge ratio 1.08；
- row 11：5,709,218 completed terms → 4,715,884 unique states，merge ratio 1.21。

状态合并很弱。HashMap contraction 为每个几乎独立的 partial configuration 支付 hash、
allocation 和大内存流量；DFS 只在紧凑递归栈上遍历同类合法前缀。只要 boundary tensor
仍以显式 HashMap entry 物化，PEPS 后端很难在同线程数下超过 DFS。

这不是 PEPS exactness 的失败，而是当前 boundary representation 的归纳偏置不适合该
support：它为很少发生的 state merge 支付了很高成本。

## 4. 修订后的假设

### H1：整行 operator 可以显著降低局域常数，但不能单独跨越 DFS gap

从显式 \(C\) 和 \(v_0/v_1\) 自动 contraction 出 row operator，避免为每个 parent
重复建立 partial-row automaton。它必须与逐格点 \(C\) contraction 对所有可达小 N
边界逐项一致。

判别目标：

- N=12、13 至少 2x；
- N=14 局域 transition work 至少降低 10x；
- count/support/RSS 语义不变。

即便成功，它仍物化相同 boundary support，所以预期只能成为后续结构实验的更快基线。

### H2：超过 DFS 需要结构性 support 压缩或更低增长率

优先候选是 exact decision diagram/ZDD 或未来行为等价类，而不是更多 HashMap 微调。
节点必须表示开放 virtual boundary tensor 的精确商，不能直接替换成 queen DFS。

判别目标：

- N=10–12 连续两个规模的节点数至少比 explicit support 低 30%；
- build/apply 时间不超过当前 promoted baseline 的 2x；
- 与显式 \(C\) contraction 的边界 coefficient map 在小 N 完全一致。

### H3：新 ordering 只有降低实际 support 才值得

在补齐 geometry-independent direct-TN oracle 后，比较 row、snake、diagonal/min-fill。
稠密 width 下降但 sparse support 不降不算成功。

判别目标：

- 两个连续 N 的 peak support 至少下降 20%；或
- measured growth ratio 明显下降；
- 否则停止 ordering 搜索。

## 5. 修订后的方向优先级

1. **E6：机械生成的 exact row operator（下一实验）。**
2. **E7：开放 virtual boundary 的 exact ZDD/BDD 原型。**
3. **E8：geometry-independent direct-TN oracle + 小规模 ordering 比较。**
4. **E9：只有当 merge ratio 提升或 candidate vector 内存安全时才做 sort-reduce。**
5. **E10：只有 serial PEPS gap 降到 DFS 的 10x 内，才投入 slicing/CPU parallel。**

降级：

- flat hash / allocator：只作为后端配套，不再作为主研究方向；
- CRT：仍是大 N exactness 必做项，但不能解决当前速度/support gap；
- GPU/MPI：在 serial algorithm 没有结构改进前推迟；
- exact finite-field MPS、MITM：保持研究型低优先级。

## 6. 第六方向的预注册要求

不得手写 `available_columns` 或复制 DFS recurrence。E6 必须：

1. 从 `SiteTensorC.entries_by_input` 和 row 的 \(v_0/v_1\) 机械生成 operator；
2. 保留每个 outgoing virtual leg 和 coefficient；
3. 在 N≤8 的全部可达 parent boundaries 上，与原 `contract_one_row` 输出 multiset 完全一致；
4. 保留原逐格点实现作为验证后端；
5. 独立 worktree、独立报告和原始 CSV；
6. 若 N=12、13 均未达到 2x，判为 `DIAGNOSTIC_ONLY` 或 `REJECT`，不继续微调。

## 7. 资源投影

当前 promoted PEPS 从 N=13 到 N=14：

- 时间约增长 6.37x；
- RSS 约增长 4.82x。

简单外推的 N=15 可能约 70 s / 3.1 GiB；N=16 可能进入数百秒和 15 GiB 量级。
这只是风险上界提示，不可用作 Q27/Q28 正式 projection。E6 前不直接启动 N≥15。


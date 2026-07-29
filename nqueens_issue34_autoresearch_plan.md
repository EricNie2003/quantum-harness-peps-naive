# Quantum Harness Issue #34：N 皇后精确张量网络收缩
## 面向 Agent Autoresearch 的完整研究与执行方案

**目标 Issue：** [QuantumBFS/quantum.harness #34](https://github.com/QuantumBFS/quantum.harness/issues/34)  
**方法赛道：** PEPS Based Algorithm  
**核心目标：** 严格、无截断地收缩 Liu–Liao–Wang 在 Sec. VI 构造的 N 皇后张量网络，复现已知计数并尽可能推进到 \(N=28\)。  
**计划定位：** 本文既是研究方案，也是可供 autonomous coding/research agent 逐阶段执行的操作规范。

---

# 2026-07-27 基于现有硬约束的重新评估（当前执行版本）

本节依据仓库根目录 `AGENTS.md` 重新评判原计划。若本节与后文旧的优先级、baseline
描述或“推荐首个任务”冲突，**以本节为准**。后文保留为研究方向的详细背景。

## A. 不可绕过的合规边界

当前主实现必须始终保留以下可执行证据：

1. 显式 rank-9 局域张量 \(B\)，含 8 条二值 virtual legs、1 条 physical leg，
   且恰有 17 个非零元；
2. 显式构造 \(C=\sum_\alpha B^\alpha\)，rank-8，恰有 17 个非零元；
3. 主 contraction 的局域转移必须从 `C.entries()` 机械生成；
4. 使用 \(v_0=(1,0)\)、row/column 终点的 \(v_1=(0,1)\)、diagonal 终点的
   \(v_2=(1,1)\)；
5. 任何 fused transition、packed state、对称切片或新 ordering 都必须与显式
   \(C\) contraction 做逐项或小 \(N\) 等价测试；
6. 普通 DFS/bitmask 只能作为 oracle 或同硬件比较，不能作为 PEPS 主实现；
7. 每个优化实验必须在独立 Git worktree/branch 中完成，baseline worktree 不修改；
8. 每个实验必须独立记录 count、wall time、peak RSS、peak support、局域项工作量和
   keep/reject 决策。

## B. 当前基线状态

| 项目 | 当前状态 | 证据 |
|---|---|---|
| 显式 \(B\) 的 17 个非零元 | 已完成 | `SiteTensorB::sec_vi` + 单元测试 |
| 显式 \(C=\sum_\alpha B\) 的 17 个非零元 | 已完成 | `SiteTensorC::from_b` + 单元测试 |
| 逐格点、逐行 exact contraction | 已完成 | `contract_one_row` / `contract_rows` |
| \(v_0/v_1/v_2\) 边界 | 已完成 | 行首、行末、棋盘底部及对角线边缘逻辑 |
| 独立小 \(N\) oracle | 已完成 | 朴素棋盘枚举，\(N=0\ldots9\) |
| 已知值验证 | 已完成至 \(N=14\) | `benchmarks/naive_release.csv` |
| wall time / RSS / support profiling | 已完成至 \(N=14\) | benchmark 和 layer CSV |
| 通用 dense/direct-TN oracle | 尚未完成 | 在改变 ordering 前补齐 |
| \(Q(16),Q(20),Q(27)\) | 尚未完成 | 当前 naive support/RSS 增长过快 |

初始冻结的 N=14 naive baseline（优化前）三次中位时间约 25.19 s，峰值 RSS 约
986 MiB，检查约 75 亿个局域 \(C\) 非零元。最直接的浪费是：已知 4 个 incoming
virtual bits 后仍线性扫描全部 17 个条目。E1、E3、E5a 完成后，当前 promoted baseline
的 N=14 中位时间为 11.09 s，峰值 RSS 约 666 MiB。

## C. 难度与可行性标尺

- 难度 1/5：局部改动，不改变边界语义，半天内可验证；
- 难度 2/5：改变状态布局或容器，需要专门等价测试和完整 benchmark；
- 难度 3/5：新增算术/并行/批处理后端，需要多个组件和失败恢复；
- 难度 4/5：改变 contraction tree、分布式执行或需要复杂资源投影；
- 难度 5/5：研究型原型，收益和完成概率均高度不确定。

“可行性”综合考虑 PEPS 合规性、当前 Rust 代码、可用单机资源和预期验证成本，而不只是
理论上能否实现。

## D. 第一轮历史研究方向重新排序（已被后文 gates 取代）

本表保留最初规划及编号映射用于审计；它不是当前执行队列。当前 E11–E15 以文末“基于
`Q&A.md` 三轮讨论与 scaling 实测的第三阶段修订”为准。

| 新顺序 | 方向 | PEPS 合规条件 | 可行性 | 难度 | 预期收益 | 当前决策 |
|---:|---|---|:---:|:---:|---|---|
| E1 | 按 4-bit incoming signature 索引 \(C\) | 索引必须由显式 `C.entries()` 自动生成并逐项比对 | 极高 | 1/5 | 去掉每次 17 项线性扫描，support 不变 | **首先尝试** |
| E2 | `HashMap` 容量预估/复用 | 只改变容器，不改变 tensor transition | 极高 | 1/5 | 减少 rehash；可能增加 RSS | E1 后尝试 |
| E3 | 三个 mask 打包为单个 `u128` key | 编解码必须与开放 virtual indices 一一对应 | 很高 | 2/5 | key 从 24 B 降至 16 B，改善 hash/RSS | E2 后尝试 |
| E4 | flat/robin-hood hash 后端 | 输入输出必须与标准 `HashMap` 完全一致 | 高 | 2/5 | 更少指针和更高吞吐 | E3 后尝试 |
| E5 | 从 \(C\) 自动生成整行 sparse operator | 禁止手写 `available_columns`；逐行表必须由局域 \(C\) contraction 生成 | 高 | 2/5 | 融合临时 partial row，降低局域开销 | E4 后尝试 |
| E6 | sort-reduce / radix-reduce | candidate 必须来自同一 tensor operator | 中高 | 3/5 | 顺序内存访问、易并行；临时内存风险 | hash 基线稳定后 |
| E7 | 机器字有限域 + CRT | 模数乘积必须显式证明 \(>N!\)，保留冗余模数 | 高 | 3/5 | 固定字宽、天然并行；不降 support | 在主容器确定后 |
| E8 | prefix tensor slicing | slice 是固定 physical/virtual boundary 的子网络；加权和需核验 | 高 | 3/5 | 可恢复并行、降低单任务 RSS | CRT 前后均可 |
| E9 | CPU 多线程/work stealing | 每个 worker 收缩合规 tensor slice | 高 | 3/5 | 吞吐提升；总内存可能放大 | slicing 后 |
| E10 | row/column/snake/diagonal ordering | 必须先补通用 direct-TN oracle；不能调用 DFS recurrence | 中 | 4/5 | 可能降低 peak support，也可能更差 | 推迟到 E1–E6 后 |
| E11 | support-aware cost model | 只用于选择合法 contraction tree | 中 | 3/5 | 诊断/排序价值，不直接提速 | 有至少 3 种 ordering 数据后 |
| E12 | ZDD/BDD 边界 | 节点语义必须是开放 virtual boundary 的精确商 | 中低 | 5/5 | 可能结构压缩 | 固定 3–4 小时原型预算 |
| E13 | meet-in-the-middle/separator | join signature 必须覆盖所有跨 separator virtual bonds | 低 | 5/5 | 可能改变 exponent，接口可能爆炸 | 仅在 support 数据支持时 |
| E14 | exact finite-field MPS | exact rank factorization，禁止浮点 SVD | 低 | 5/5 | 研究 exact rank；主求解未必更快 | 仅作小规模诊断 |
| E15 | MPI/Slurm job array | worker 必须执行 exact tensor slice，manifest 可验证 | 中高* | 4/5 | 扩展总吞吐 | *仅在有集群时高 |
| E16 | GPU expansion/sort-reduce | kernel transition 必须由 \(C\) 表生成并与 CPU 比对 | 中低 | 5/5 | 大批量吞吐 | CPU 算法与分片稳定后 |
| E17 | 多 GPU 分布式 TN | 保持 exact arithmetic 与完整通信校验 | 低 | 5/5 | 最终规模路线 | 远期 |

## E. 对原 R1–R17 的具体修订

### 原 R1 packed sparse boundary state

基础版本已经使用三个 `u64` mask，因此“从对象 tuple 改为机器字”已完成。剩余研究问题改为
**把三个 mask 编码为单个 `u128` key**。对 \(N\le28\)，共需 \(3N\le84\) bits，可无损
编码。该方向保留，但排在局域 \(C\) 索引和容器容量实验之后。

### 原 R2 flat hash map / allocator

拆成两个实验：

1. 无依赖的容量预估与 map 复用（难度 1）；
2. 第三方 flat/robin-hood hash（难度 2）。

不得同时改变 key encoding，否则无法归因。

### 原 R3 sort-reduce

保留。必须先测量 `completed_row_terms / unique_output_states` 的 merge ratio，并对临时
candidate vector 做内存上界。若预测峰值超过可用内存的 70%，不得直接跑最大 \(N\)。

### 原 R4 fused transition / early propagation

原文“行恰好一枚直接编码进 transition”“预计算可用位置”有退化为经典 bitmask DFS 的风险。
修订为：

> 从显式 \(C\) 自动 contraction 出完整行 operator，再缓存或融合该 operator。

只有生成器与显式逐格点 contraction 在全部可达小 \(N\) 边界上逐项一致时，才可作为 PEPS
后端。手写 queen recurrence 不接受。

### 原 R5 contraction ordering

规则路径在理论上可行，但当前实现的数据布局专为 row order 设计。开始该方向前必须实现一个
与 geometry 解耦的 factor-graph/direct-TN oracle，并在 \(N\le4\) 对每条 ordering 得到完全
相同的 contraction。难度从“高到中”调整为 4/5，优先级下调。

### 原 R6 support-aware cost model

保留为诊断方向，不应早于实际生成至少三种 ordering 的数据。没有训练/留出路径数据时拟合
cost model 没有决策价值。

### 原 R7 symmetry slicing

保留，但统一改称 **tensor slicing**。固定第一行皇后可以理解为固定一组 physical legs，
但实现必须由 \(B/C\) 网络边界条件生成 slice，且不切片 contraction 的加权和必须一致。
完整 \(D_4\) 权重处理推迟到 prefix slicing 已验证之后。

### 原 R8 finite field / CRT

合规且可行，但它不减少 support。当前系数远未成为主要瓶颈，因此排在状态/容器优化之后；
作为 Q20 以上的严格算术和冗余校验仍是必做项。

### 原 R9 exact finite-field MPS

保留为小规模科学诊断，不进入主求解关键路径。若 \(N\le8\) 已比 sparse boundary 慢
100 倍且 exact rank 没有显著压缩，立即停止。

### 原 R10 ZDD/BDD

保留固定时间盒。必须表示开放 virtual boundary 的未来等价类，而不是直接改做 queen-placement
ZDD。若节点数或 apply/reduce 时间在两个连续 \(N\) 上不优于 explicit support，则停止。

### 原 R11 separator / meet-in-the-middle

可行性下调为低。列和两族 diagonal virtual lines 同时跨 separator，signature 很可能抵消
双向收缩收益。只有测得 separator signature 上界明显小于 row-order peak support 才实现。

### 原 R12 CPU parallel

保留，但放在 deterministic tensor slicing 之后，避免多个线程共享一个超大 hash map。
首选 slice-level 并行和独立 map，而不是细粒度加锁。

### 原 R13 MPI/Slurm

算法上可行，但依赖外部基础设施。无可用集群时只实现 manifest/reducer，不假设集群存在。

### 原 R14 GPU expansion + sort-reduce

保留为远期。GPU kernel 必须消费从 \(C\) 导出的 transition table；在 CPU sort-reduce、
固定字宽算术和切片 manifest 稳定之前不启动。

### 原 R15 multi-GPU distributed contraction

远期方向。通信量和设备资源都尚无数据，当前不能给出“中等可行”的结论。

### 原 R16 hybrid TN + classic search

重新分类为 **对照/验证方向，不是默认主方法**。若 tensor contraction 只生成 DFS prefix，
剩余部分完全由经典回溯求解，则不能作为纯 PEPS 主结果。只有整个 hybrid 流程仍可表述为
合法 tensor slicing 与子网络 exact contraction 时才可能合规。

### 原 R17 approximate-guided exact

只允许近似结果用于排序合法 exact tensor slices 或 contraction trees。近似量不能删除 slice、
决定 exact coefficient 为零或参与最终计数。

## F. 逐个实验的统一协议

每个 E1–E17 实验严格执行：

1. 从冻结 baseline commit 创建独立 `codex/exp-*` branch 和 Git worktree；
2. 写预注册记录：单变量假设、pass threshold、kill condition、目标 \(N\)；
3. 先跑局域 \(B/C\) truth table、独立 oracle 和已知 Q(N)；
4. 短任务先用 \(N=10,11,12\) 三次中位数筛选；
5. 只有在至少两个连续 \(N\) 有稳定收益时才跑 \(N=13\)；
6. 预测内存安全后才跑 \(N=14\)；
7. 记录 wall time、peak RSS、peak support、tensor work、count；
8. 生成独立实验报告和原始 CSV；
9. 明确 `KEEP`、`REJECT` 或 `DIAGNOSTIC_ONLY`；
10. 只有 `KEEP` 实验才允许合入下一轮 baseline；被拒实验保留 branch/report，不污染 baseline。

默认 keep threshold：

- 时间降低至少 15%，且两个连续 \(N\) 一致；或
- peak RSS 降低至少 15%；或
- 最大安全 \(N\) 增加 1；或
- 提供明确、可复现的结构性负结果。

主后端升级仍沿用更严格目标：优先寻找 \(\ge2\times\) 加速、\(\ge30\%\) 内存下降或可达
尺寸增加 1 的方向。

## G. 当前立即执行队列

1. **E1：4-bit incoming signature 的 \(C\) 索引。**
   - 假设：把每次 17 项扫描替换为从显式 \(C\) 自动生成的 16 桶 lookup；
   - 预期：`tensor_entries_examined` 大幅下降，support/RSS/count 不变；
   - kill：N=11、12 中位时间改善均小于 15%；
   - correctness：索引桶展开后必须与 `C.entries()` 集合完全相同。
2. **E2：HashMap 容量策略。**
   - 假设：利用上一层 completed terms 预留下一层容量可减少 rehash；
   - 风险：过度预留使 RSS 上升；
   - kill：时间改善小于 10%或 RSS 上升超过 15%。
3. **E3：单个 `u128` packed key。**
   - 假设：更小 key 改善 hash/cache，并降低 bytes/state；
   - correctness：全范围 pack/unpack round trip + 与标准 boundary map 比对；
   - kill：时间和 RSS 都无稳定改善。

E1 完成前不并行启动 E2；E2 完成后的 keep/reject 结果决定 E3 的新 baseline。

## H. 本轮实际执行记录与停止点

本轮按独立 worktree、单变量和 correctness-first 协议顺序完成了以下实验：

| 实验 | 分支 | 结果 | N=14 或最大判别规模 | 决策 |
|---|---|---|---|---|
| E1：4-bit incoming signature 索引 \(C\) | `codex/exp-c-input-index` | N=14 从 25.19 s 降到 19.31 s；tensor entry 检查减少 93.8%；RSS/support 不变 | N=14 | **KEEP，已合入** |
| E2：`HashMap::with_capacity(input_states)` | `codex/exp-hash-capacity` | N=13 仅快约 3.4%，RSS 收益未延续 | N=13 | **REJECT，未合入** |
| E3：三个 mask 打包为 `u128` key | `codex/exp-packed-u128` | N=14 RSS 从约 986.7 MiB 降到 666.2 MiB，时间再改善 8.8% | N=14 | **KEEP，已合入** |
| E4a：标准 HashMap 上替换确定性 hasher | `codex/exp-fast-u128-hasher` | N=13 中位数慢约 2.1%，RSS 不变 | N=13 | **REJECT，未合入** |
| E5a：复用逐格点 partial row buffers | `codex/exp-partial-buffer-reuse` | 相对 E3，N=14 从 17.61 s 降到 11.09 s，1.59x；RSS/support 不变 | N=14 | **KEEP，已合入** |

从最初显式 \(C\) naive baseline 到当前 promoted baseline：

- N=14 三次中位时间：25.1915 s → 11.0897 s，累计约 **2.27x**；
- N=14 peak RSS：986.34 MiB → 666.17 MiB，下降约 **32.5%**；
- peak support 始终为 5,479,934，说明这些都是常数/内存布局收益，没有改变指数增长；
- 所有已运行 count、局域 truth table、pack/unpack、独立 oracle 和已知值测试均通过。

原始数据与独立报告：

- `experiments/e1_c_input_index/`
- `experiments/e3_packed_u128/`
- `experiments/e5a_partial_buffer_reuse/`
- E2、E4a 的负结果保存在各自未合入的实验分支中。

按用户指示，**测试完 E5 后停止**：

- E5a 已完成；
- E5b“从显式 \(C\) 自动生成并缓存完整 row operator”尚未尝试；
- E6 sort-reduce、E7 CRT、E8 slicing 及后续方向均未启动；
- 恢复研究时应从当前 main baseline 新建独立 worktree，不能从被拒分支继续。

## I. 五方向强制复盘后的计划修订（2026-07-28）

用户已要求继续优化。根据 `AGENTS.md` 的 five-direction review gate，在第六个方向前已经完成
`docs/five_direction_review_01.md`。该复盘推翻了“继续优先做 hash/allocator 微调”的近期
计划。

关键新证据：

- 当前 PEPS N=14：11.0897 s / 666.17 MiB；
- 同硬件单线程 DFS N=14：0.0703587 s，PEPS 慢约 157.6x；
- 同硬件 16-thread DFS N=14：0.0057339 s，PEPS 慢约 1933.9x；
- E1/E3/E5a 都没有降低 5,479,934 的 peak support；
- 最重层 merge ratio 只有约 1.08–1.21。

因此接下来的执行队列改为：

1. **E6：由显式 \(C\) 自动生成 exact row operator。**
   - 作用：测出可移除的局域 contraction 开销上限；
   - correctness：N≤8 全部可达 parent boundary 与逐格点后端逐项一致；
   - keep：N=12、13 至少 2x，或 N=14 的局域工作量降低至少 10x；
   - 限制：即使 KEEP，也不宣称它已解决 support explosion。
2. **E7：exact ZDD/BDD boundary representation。**
   - 目标：不改变 \(C\) 语义的未来行为等价类压缩；
   - keep：连续两个 N 的节点数比 explicit support 低至少 30%；
   - 固定原型时间盒，失败即停止。
3. **E8：direct-TN oracle 与 ordering 小规模比较。**
   - 只接受实际 sparse support 或增长率下降；
   - dense treewidth proxy 单独下降不构成 KEEP。
4. **E9：有条件的 sort-reduce。**
   - 当前低 merge ratio 下优先级下降；
   - 只有 candidate/unique 比或顺序内存模型显示收益才启动。
5. **E10：并行与 slicing。**
   - serial PEPS 尚慢单线程 DFS 两个数量级，暂不以并行掩盖算法 gap。

flat hash、allocator、独立 hasher 降级为配套工程；CRT 保留为大 N exactness 必做项，但不再
被当作速度/support 优化。

---

# 0. 执行摘要

## 0.1 官方要求核对结论

此前提出的“精确稀疏边界收缩 + 收缩顺序 + 有限域/CRT + 对称分片”总体方向**符合 Issue #34**，但必须加入以下约束才能满足正式验收：

1. **必须忠实对应 Sec. VI 的张量网络。**  
   可以使用等价的逐行状态转移、变量消元或稀疏边界表示，但必须给出严格等价性证明，而不能只提交一个普通 N 皇后回溯程序。

2. **必须严格精确。**  
   允许：
   - 任意精度整数；
   - 固定宽度有限域运算；
   - 多素数结果加中国剩余定理（CRT）；
   - 经过证明的精确状态合并。

   不允许作为最终结果：
   - 有限键维数截断；
   - 非零阈值 SVD；
   - 未认证的浮点舍入；
   - 只因结果“接近整数”便四舍五入。

3. **正式验收必须包括 \(Q(27)\)。**  
   Issue 要求至少复现四个 OEIS 基准，且必须包含：
   - \(Q(8)=92\)
   - \(Q(16)=14\,772\,512\)
   - \(Q(20)=39\,029\,188\,884\)
   - \(Q(27)=234\,907\,967\,154\,122\,528\)

4. **\(Q(28)\) 是 headline target，不是唯一成功条件。**  
   如果未达到 \(Q(28)\)，必须报告：
   - 最大成功 \(N\)；
   - 时间和内存 scaling；
   - 下一尺寸失败的明确瓶颈；
   - 与 FPGA/GPU backtracking 的比较。

5. **最终代码必须作为 PR 提交到 quantum.harness。**

## 0.2 一个需要注意的源数据问题

Issue 正文表格列出的 \(Q(25)=2\,207\,893\,435\,360\) 与 OEIS A000170 不一致。  
OEIS 当前给出的正确序列为：

- \(Q(24)=24\,233\,937\,684\,440\)
- \(Q(25)=227\,514\,171\,973\,736\)
- \(Q(26)=22\,317\,699\,616\,364\,044\)
- \(Q(27)=234\,907\,967\,154\,122\,528\)

因此：

> **所有自动化正确性测试应直接从固定版本的 OEIS 数据文件或人工审核后的常量表读取，不要复制 Issue 中的 \(Q(25)\)。**

Issue 指定的四个最低验收基准本身是正确的。

## 0.3 推荐主路线

优先实施：

> **严格整数/有限域的稀疏 boundary contraction，配合 packed state、支持感知的收缩顺序、对称切片和可恢复并行。**

研究主问题是：

> 对这个局域张量极稀疏、但精确边界 support 快速增长的网络，哪种中间表示和收缩顺序最能抑制实际非零 support、峰值内存和通信量？

## 0.4 对 \(Q(28)\) 的定位

只有满足以下前提才允许完整启动 \(Q(28)\)：

1. 已通过严格 TN 方法复现 \(Q(27)\)；
2. 至少两种独立 exactness check 一致；
3. 分片和 checkpoint 已验证；
4. 对 \(N=28\) 的样本任务做过资源投影；
5. 预计成本落在实际可用资源预算内。

否则输出完整的 \(N=28\) workload projection，而不是盲目启动不可完成的任务。

---

# 1. 官方 Issue 的合规性矩阵

| 官方要求 | 本方案对应措施 | 验收证据 | 状态 |
|---|---|---|---|
| 跟随 Sec. VI 的 \(B/C\) 张量和边界向量 | 从局域约束张量构建 oracle；逐行/其他消元形式附严格等价性证明 | 数学推导、局域 truth table、直接小网络收缩 | 必须 |
| 严格无截断 | 整数或有限域后端；禁止浮点阈值决定最终计数 | arithmetic manifest、模数、CRT 唯一性证明 | 必须 |
| 复现至少四个基准且包括 \(Q(27)\) | 自动 benchmark \(N=8,16,20,27\) | `benchmarks.csv`、日志、checksum | 必须 |
| 尽可能计算 \(Q(28)\) | 先完成 Q27，再经过 go/no-go gate | Q28 结果或资源投影 | 进阶 |
| 报告 runtime/memory scaling | 每层记录状态数、support、RSS、时间 | scaling 图、逐层 CSV/Parquet | 必须 |
| 识别瓶颈 | profiler + 复杂度分解 | hotspot、bytes/state、merge ratio、通信量 | 必须 |
| 与 FPGA/GPU backtracking 比较 | 同硬件本地 comparator + 外部文献比较 | 方法差异表、归一化性能说明 | 必须 |
| 提交 quantum.harness PR | 按 harness 目录与报告流程组织 | PR 链接 | 必须 |

## 1.1 “符合研究方向”与“满足正式验收”的区别

以下成果具有研究价值，但**单独不足以通过正式 Issue 验收**：

- 只复现 \(N\leq20\)；
- 只做普通 bitmask DFS；
- 只做浮点 MPS 到 \(N=6\)；
- 只分析 contraction ordering 而没有严格计数；
- 只给出 \(Q(28)\) 的性能预测；
- 只运行近似 CTMRG/MPS 并舍入整数。

正式验收的最低硬门槛仍是：**documented exact TN method + 包含 Q27 的四个基准 + scaling/report/PR**。

---

# 2. 问题定义与 exactness contract

## 2.1 计算对象

计算总解数 \(Q(N)\)：

- 棋盘大小为 \(N\times N\)；
- 每行恰好一枚皇后；
- 每列恰好一枚皇后；
- 每条主对角线和副对角线至多一枚皇后；
- 计数包括旋转、反射得到的所有不同棋盘配置；
- 不计算仅模去 \(D_4\) 对称后的 fundamental solutions，除非之后正确乘回轨道权重。

## 2.2 Sec. VI 张量网络

必须以 Liu–Liao–Wang 的 Sec. VI 为规范来源：

- 每条行、列、两类对角线约束由 bond dimension 2 的矩阵乘积约束表示；
- 每个格点四条约束线相交；
- 求和物理变量后得到 rank-8 局域张量 \(C\)；
- \(C\) 的每条虚腿维数为 2；
- \(C\) 只有 17 个非零元素；
- 边界向量：
  - \(v_0=(1,0)\)：约束线起点；
  - \(v_1=(0,1)\)：要求恰好一个占据，用于行和列；
  - \(v_2=(1,1)\)：允许零或一个占据，用于对角线；
- 完整网络收缩严格等于 \(Q(N)\)。

逐行写成：

\[
Q(N)=\langle v_f|T^N|v_0\rangle .
\]

转移矩阵 \(T\) 满足 \(T^{N+1}=0\)，所以不能把问题替换为寻找主导本征值。

## 2.3 可接受的等价表示

可接受，但必须证明等价：

- 逐行 sparse transfer；
- boundary tensor dictionary；
- exact MPS/MPO；
- 有限域 MPS；
- variable elimination / factor graph contraction；
- ZDD/BDD 表示的边界 support；
- 对称分片后的子网络求和；
- meet-in-the-middle；
- 任意 contraction tree；
- CPU/GPU/MPI 分布式 contraction。

等价性证明至少应包含：

1. 对任意棋盘配置 \(\sigma\)，局域张量网络权重只能为 0 或 1；
2. 权重为 1 当且仅当所有 N 皇后约束满足；
3. 对所有 \(\sigma\) 求和恰好得到总解数；
4. 新状态表示与开放虚指标之间存在一一对应或经过证明的等价类对应；
5. 每个状态合并不会改变未来所有可能续接的总权重。

## 2.4 不可接受的“伪精确”

参考仓库中的 `nqueens_mps.py` 使用 `float64` SVD，并以相对阈值 `1e-14` 删除奇异值。即使 `max_chi=None`，这种做法也不是代数意义上的严格精确。

该实现只能用于：

- 复现普通 MPS 的 bond growth；
- 展示截断失败；
- 作为浮点性能对照；
- 小规模 sanity check。

不得用它的“exact mode”作为正式 exactness 声明。

## 2.5 允许的严格算术

### 路线 A：任意精度整数

中间张量/状态系数直接存 Python/C++ arbitrary precision integers。

优点：

- 实现直接；
- 易于调试；
- 最少证明负担。

缺点：

- 大整数 hash、加法、内存开销会增长。

### 路线 B：有限域 + CRT

对若干互素质数 \(p_i\) 分别计算：

\[
q_i=Q(N)\bmod p_i.
\]

再由 CRT 重构。因为：

\[
0\le Q(N)\le N!,
\]

只要：

\[
\prod_i p_i>N!,
\]

重构结果唯一。

对于 \(N=28\)，\(28!\) 少于 \(2^{100}\)，因此通常两个约 61-bit 的素数已经足够；实现必须实际检查模数乘积，而不是依赖这句话。

优点：

- 固定宽度整数；
- 无浮点误差；
- 不同模数天然并行；
- 可作为独立正确性检查。

缺点：

- 不减少边界状态数；
- 需要处理坏素数导致的有限域秩偶然下降，尤其在 exact MPS 路线中。

---

# 3. Baseline 体系

Agent 不得直接开始优化。必须先建立以下 baseline。

## 3.1 Baseline B0：小棋盘穷举 oracle

实现直接枚举或 permutation 枚举，仅用于小 \(N\)。

目标：

- \(N=0\) 到至少 \(N=10\)；
- 给其他实现提供完全独立的 correctness oracle；
- 验证总解数与 fundamental solution 没有混淆。

评价：

- 只看正确性；
- 不纳入大规模性能结论。

## 3.2 Baseline B1：经典 bitmask DFS

实现标准递归：

- `cols`
- `diag_left`
- `diag_right`
- 可选首行镜像对称；
- 可选前缀分片。

作用：

- 同硬件经典算法基线；
- 测量 tensor-network 路线相对于成熟 combinatorial search 的差距；
- 后期可用于逐 slice 交叉验证。

注意：

> B1 本身不自动满足“exact TN contraction”要求。只有当提交明确证明某个 transfer/variable-elimination 实现就是 Sec. VI 网络的收缩时，才能作为主方法。

## 3.3 Baseline B2：参考逐行 sparse transfer

状态：

\[
(C,D_{\searrow},D_{\nearrow})\mapsto w,
\]

其中三组 bitmask 分别记录：

- 已占用列；
- 下一行受到的主对角攻击；
- 下一行受到的副对角攻击；
- \(w\) 是到达该边界状态的配置数。

这是首选 exact TN baseline，因为它可以直接解释为逐行收缩 Sec. VI 网络。

要求：

- 使用整数或有限域系数；
- 每层输出完整 profiling；
- 在 README 中给出与局域张量网络的等价证明。

## 3.4 Baseline B3：直接小规模张量收缩

对 \(N\leq4\) 或资源允许的范围，显式构造 rank-8 \(C\) 网络：

- 使用通用 einsum/contraction 工具；
- 不做截断；
- 与 B0/B2 核对。

作用：

- 防止 B2 与传统 DFS 在实现上“碰巧一致”，却没有真正验证 Sec. VI tensor convention；
- 验证腿方向、边界向量和对角线方向。

## 3.5 Baseline B4：参考浮点 MPS/MPO

只运行小规模，记录：

- 最大 bond dimension；
- SVD 时间；
- 内存；
- 截断后错误模式；
- 与 B2 的差异。

预设停止：

- 超过本地 10 分钟或 16 GB 前迁移远端；
- 不允许为追求更大 \(N\) 消耗主线时间；
- 不把其结果标注为 strictly exact。

## 3.6 Baseline B5：外部公开性能

报告时比较：

- FPGA carry-chain / backtracking；
- GPU parallel DFS；
- 多 GPU exact tensor contraction 基础设施。

必须区分：

1. 同硬件直接 wall time；
2. 不同硬件的文献结果；
3. 算法 scaling；
4. 能源、设备数量和运行时差异。

禁止将不同硬件的 wall time 简单作为速度倍数结论。

---

# 4. 研究方向池与可行性

## 4.1 方向 R1：packed sparse boundary state

### 方法

把三个 \(N\)-bit mask 打包成：

- 64-bit：适用于较小 \(N\)；
- 128-bit 或两个 64-bit word：适用于 \(N\leq28\)；
- 自定义结构体避免 tuple/object overhead。

### 可行性

**很高。**

### 评价指标

- transitions/s；
- unique states/s；
- peak RSS；
- bytes/state；
- hash probe 次数；
- 最大可达 \(N\)。

### 成功标准

至少满足一项：

- 同规模时间降低 2 倍；
- 内存降低 30%；
- 最大可达 \(N\) 增加 1；
- 每状态内存显著接近理论下限。

### 风险

只改善常数，不改变指数增长率。

---

## 4.2 方向 R2：flat hash map 与自定义 allocator

### 方法

比较：

- Python dict；
- C++ `unordered_map`；
- robin-hood / Swiss-table 风格 flat hash；
- 预分配容量；
- arena allocator。

### 可行性

**高。**

### 评价指标

- insert/update throughput；
- rehash 次数；
- load factor；
- bytes/state；
- time breakdown。

### Kill 条件

若 compiled sort-reduce 已显著优于 hash，则停止继续调 hash。

---

## 4.3 方向 R3：sort-reduce / radix-reduce

### 方法

每层：

1. 批量生成所有合法输出 `(key, value)`；
2. 按 packed key 排序；
3. 相同 key 求和；
4. 形成下一层唯一 support。

可以比较：

- comparison sort；
- radix sort；
- CPU parallel sort；
- GPU sort-reduce。

### 可行性

**高到中。**

### 评价指标

- candidate transitions；
- merge ratio；
- sorting time；
- reduction time；
- 内存峰值；
- CPU/GPU throughput。

### 特别价值

当多个父状态大量汇聚到同一边界状态时，sort-reduce 容易并行且更具顺序访问特征。

### 风险

临时 transition 数可能远大于 unique state 数，导致峰值内存高于 hash。

---

## 4.4 方向 R4：fused transition 与早期约束传播

### 方法

避免生成注定非法的中间状态：

- 列占用冲突立即拒绝；
- 对角线冲突立即拒绝；
- 行恰好一枚的约束直接编码进 transition；
- 合并原本拆成多个 MPO 的局部步骤；
- 预计算可用位置和后继 key。

### 可行性

**很高。**

### 评价指标

- candidate/accepted ratio；
- branch 数；
- instructions/transition；
- 中间 support；
- time。

### 成功标准

accepted transitions 不变，candidate transitions 或 wall time 明显下降。

---

## 4.5 方向 R5：逐行、逐格点和其他收缩顺序

### 候选

- row-by-row；
- column-by-column；
- snake single-site；
- diagonal；
- anti-diagonal；
- edge-to-center；
- nested dissection；
- min-degree；
- min-fill；
- weighted min-fill；
- 小宽度 beam search；
- 局部 contraction-tree rewrite。

### 可行性

- 规则路径：**高到中**
- 自动路径搜索：**中**
- 全局最优搜索：**低**

### 评价指标

#### 稠密结构指标

- frontier logical width；
- induced width；
- estimated dense FLOP；
- peak logical tensor dimension。

#### 实际稀疏指标

- peak support；
- support density；
- candidate transitions；
- unique outputs；
- peak RSS；
- wall time。

### 核心科学问题

> 稠密 treewidth proxy 是否能预测这个极稀疏约束网络的实际 exact contraction cost？

### 成功标准

至少比较三种有本质差异的路径，并解释：

- width 是否降低；
- support 是否降低；
- wall time 是否与二者一致。

### 风险

逐行路径可能已经充分利用“每行恰好一枚”的约束，其他几何路径即使 frontier width 较小，也可能失去强约束传播。

---

## 4.6 方向 R6：支持感知的 contraction cost model

### 方法

定义经验成本，例如：

\[
C_{\mathrm{step}}
=
a\,n_{\mathrm{cand}}
+b\,n_{\mathrm{out}}
+c\,M_{\mathrm{peak}}
+d\,B_{\mathrm{move}}.
\]

用较小 \(N\) 的真实 contraction 数据拟合权重，用于选择路径或局部 rewrite。

### 可行性

**中。**

### 评价指标

- predicted vs measured Spearman/Pearson correlation；
- top-k path 命中率；
- 搜索开销；
- 新路径相对基线收益。

### Kill 条件

若模型无法在留出 \(N\) 上排序路径，则只保留为诊断，不用于主算法。

---

## 4.7 方向 R7：对称性分片

### 方法

按：

- 第一行皇后列；
- 前两到若干行配置；
- 左右镜像；
- 完整 \(D_4\) 轨道；

拆分成独立子网络。

### 可行性

- 第一行镜像：**很高**
- 前缀分片：**很高**
- 完整 \(D_4\)：**中**

### 评价指标

- slice 数；
- 权重正确性；
- 最大/平均 slice 时间；
- P90/P99；
- 总工作量变化；
- parallel efficiency。

### 必须验证

- slice 加权和等于未切片结果；
- 对称固定点处理正确；
- 总解数与 fundamental solutions 不混淆。

### 风险

对称性通常主要改善常数和并行性，不会消除指数增长。

---

## 4.8 方向 R8：有限域与 CRT

### 方法

- 选择多个互素机器字素数；
- 每个模数独立执行相同 contraction；
- 记录 residue；
- 检查模数乘积大于 \(N!\)；
- CRT 重构；
- 可选第三模数作为冗余验证。

### 可行性

**高。**

### 评价指标

- modular arithmetic throughput；
- 与 bigint 的时间和内存对比；
- 模数间一致性；
- CRT 重构时间；
- 总额外计算倍数。

### 成功标准

在已知 \(N\) 上精确重构 OEIS 值，并能自动证明唯一性。

### 风险

CRT 只解决系数算术，不解决 support explosion。

---

## 4.9 方向 R9：exact finite-field MPS

### 方法

在 \(\mathbb F_p\) 上：

- 用 exact Gaussian elimination/rank factorization 代替浮点 SVD；
- 形成 exact MPS；
- 比较不同素数下的 bond rank；
- 多素数 CRT 恢复最终整数。

### 可行性

**中低。**

### 研究价值

可以回答：

> 普通浮点 MPS 中的巨大 bond dimension 是数值条件数问题，还是代数上真实的精确秩增长？

### 评价指标

- bond rank profile；
- rank 对素数的稳定性；
- factorization 时间；
- MPS storage；
- 与 sparse support 的比较。

### Kill 条件

若 exact factorization 在 \(N\leq8\) 已比 sparse boundary 慢两个数量级，降级为小规模诊断，不用于主求解。

---

## 4.10 方向 R10：ZDD/BDD/自动机最小化

### 方法

将所有可续接边界状态表示为：

- zero-suppressed decision diagram；
- binary decision diagram；
- algebraic decision diagram；
- weighted finite automaton。

尝试共享大量相同未来行为的子结构。

### 可行性

**中。**

### 研究动机

N 皇后合法集合非常稀疏。低秩 MPS 的归纳偏置未必合适，而 ZDD 对稀疏组合族可能更自然。

### 评价指标

- diagram node count；
- 节点增长率；
- 与 explicit state count 的压缩率；
- apply/reduce 时间；
- peak memory；
- 可达最大 \(N\)。

### Kill 条件

固定 3–4 小时原型预算；若小 \(N\) 节点数不优于 sparse state 或 reduction 成本过高，立即停止。

---

## 4.11 方向 R11：separator / meet-in-the-middle

### 方法

将棋盘分为上下或左右两部分：

- 分别收缩；
- 以 separator signature 为接口；
- join 相容边界；
- 可结合 ZDD 或 hash join。

### 可行性

**中低。**

### 评价指标

- 两侧 signature 数；
- join candidate 数；
- peak memory；
- 相对于逐行的 exponent estimate。

### 风险

列和两类对角线跨越 separator，接口 signature 可能仍然指数大。

---

## 4.12 方向 R12：CPU 并行与 work stealing

### 方法

- 前缀切片；
- thread pool/OpenMP；
- 动态 work stealing；
- 每个线程独立局部 map，周期性 merge；
- NUMA-aware placement。

### 可行性

**高。**

### 评价指标

\[
\eta_p=\frac{T_1}{pT_p}
\]

以及：

- speedup；
- load imbalance；
- merge overhead；
- memory duplication；
- NUMA traffic。

### 风险

状态 map 共享会导致锁竞争；独立 map 又会放大内存。

---

## 4.13 方向 R13：MPI/Slurm job array

### 方法

- 每个 prefix/symmetry slice 是独立任务；
- deterministic task manifest；
- 每个 task 输出 residue、计数、checksum、runtime；
- reducer 验证 task 完整性并求和；
- 自动 retry；
- checkpoint/restart。

### 可行性

**高，前提是有集群。**

### 评价指标

- completed task rate；
- failed/retried tasks；
- P50/P90/P99 runtime；
- queue overhead；
- total core-hours；
- reducer time。

### Harness 要求

预计超过 10 分钟或 16 GB 的任务应先做资源估计，并通过 cluster/Slurm workflow 运行，而不是长时间占用本地。

---

## 4.14 方向 R14：GPU 批量 expansion + sort-reduce

### 方法

适合：

- packed fixed-width state；
- 批量生成后继；
- radix sort；
- segmented reduction；
- 多 GPU 按 slice 分配。

### 可行性

**中低。**

### 评价指标

- state expansion throughput；
- device memory；
- sort bandwidth；
- host-device transfer；
- GPU occupancy；
- 总 wall time。

### 风险

不规则 branching 和大规模动态 support 可能使 GPU 利用率低；开发时间较高。

---

## 4.15 方向 R15：通信感知的多 GPU contraction

### 方法

参考通用 exact TN 多 GPU 工作：

- 不仅在起始处 slicing；
- 在 contraction tree 中间分布中间张量；
- 选择通信和计算平衡；
- 尽量避免单 GPU 内存成为硬上限。

### 可行性

**低，属于长期扩展。**

### 评价指标

- 每步 communication bytes；
- compute/communication ratio；
- per-GPU peak memory；
- strong scaling；
- network saturation。

---

## 4.16 方向 R16：经过认证的近似 MPS

### 方法

允许 MPS 截断，但同时严格传播误差界。若最终证明：

\[
|\widetilde Q-Q|<\frac12,
\]

则最近整数恢复是严格的。

### 可行性

**低。**

### 评价指标

- 实际误差；
- 可认证误差上界；
- 上界放大率；
- 所需 bond dimension。

### 最大风险

非归一、非幺正的转移会使普通范数误差界迅速爆炸，最终完全无用。

### 定位

只作为高风险研究支线，不能替代主线。

---

# 5. 推荐的阶段化 Autoresearch 工作流

每个阶段都必须满足：

- 明确输入；
- 单一主要问题；
- 预注册评价指标；
- pass/fail gate；
- 保存可复现 artifact；
- 失败时回退到上一稳定版本；
- 不允许未经 gate 直接跳到长时间大规模运行。

---

## Phase 0：Harness 注册与环境锁定

### 目标

在 quantum.harness 中正确建立 challenge workspace。

### Agent 动作

1. 克隆仓库；
2. 运行：
   - `/onboard`
   - `make skills`
   - `/take-challenge` 或仓库当时规定的 challenge registration workflow；
3. 确认 issue #34 的 method route 为 PEPS；
4. 创建 challenge branch；
5. 检查：
   - `AGENTS.md`
   - PEPS method skill
   - Slurm profile
   - challenge report skill；
6. 记录当前 harness commit SHA；
7. 不修改与 challenge 无关的全局 harness 文件。

### 建议目录

实际目录应以 `/take-challenge` 创建结果为准。预计形态：

```text
tracks/peps/solutions/<team-name>/
├── README.md
├── pyproject.toml / CMakeLists.txt
├── src/
├── tests/
├── configs/
├── scripts/
└── docs/
```

运行结果：

```text
tracks/peps/results/<run-id>/
├── run.json
├── benchmarks.csv
├── layer_metrics.csv
├── environment.txt
├── logs/
├── figures/
└── checksums/
```

大型 checkpoint 不应直接提交 Git；在 manifest 中记录其路径和 hash。

### Gate P0

- workspace 可从 clean checkout 构建；
- 测试命令存在；
- 当前 git SHA 和环境被记录；
- challenge issue、track、team folder 正确。

---

## Phase 1：Sec. VI 构造审计

### 目标

证明实现对象确实是 Issue 指定的张量网络。

### Agent 动作

1. 阅读论文 Sec. VI；
2. 实现局域约束 MPO \(A\)；
3. 实现 \(v_0,v_1,v_2\)；
4. 枚举并存储 \(C\) 的非零 entries；
5. 验证非零数为 17；
6. 对单条长度 \(L\) 的约束：
   - `v0 A(...) v1` 当且仅当恰有一个占据时为 1；
   - `v0 A(...) v2` 当且仅当至多一个占据时为 1；
7. 对 \(N\leq4\) 直接收缩完整网络；
8. 与穷举 oracle 比较。

### 输出

- `docs/sec_vi_equivalence.md`
- `tests/test_local_tensor.py`
- `tests/test_direct_small_network.py`
- `data/local_tensor_entries.json`

### Gate P1

全部满足：

- \(C\) 非零 entries 数为 17；
- 所有单线 truth table 通过；
- \(N=0,\ldots,4\) 与穷举一致；
- 对角线方向和边界 convention 有图或明确说明。

失败则禁止进入优化阶段。

---

## Phase 2：独立 baseline 复现

### 目标

建立至少三套相互独立的 exact 结果来源。

### Agent 动作

实现并测试：

1. permutation/bruteforce oracle；
2. bitmask DFS；
3. integer sparse row-transfer；
4. 可选参考 float MPS。

### 测试集

优先连续验证：

\[
N=0,1,\ldots,16.
\]

然后：

- \(N=20\)；
- 更大 \(N\) 根据性能推进。

### 输出指标

对每个 \(N\) 和 method：

- count；
- exact arithmetic type；
- elapsed；
- peak RSS；
- processed nodes/states；
- git commit；
- hardware；
- checksum。

### Gate P2

- 三种 exact 方法在共同范围完全一致；
- \(Q(8)\)、\(Q(16)\) 正确；
- row-transfer 的等价证明完成；
- 所有数据自动写入结果目录；
- 运行相同 config 能复现相同结果。

---

## Phase 3：Instrumentation 与瓶颈识别

### 目标

在优化前确定真实瓶颈。

### 每层必须记录

```text
N
layer_id
ordering
arithmetic_backend
input_states
candidate_transitions
accepted_transitions
unique_output_states
merge_ratio
support_density
elapsed_expand
elapsed_merge
elapsed_arithmetic
elapsed_total
peak_rss
bytes_per_state
```

定义：

\[
\text{merge ratio}
=
1-\frac{n_{\mathrm{unique}}}{n_{\mathrm{accepted}}}.
\]

### Agent 动作

1. 在至少三个 \(N\) 上 profiling；
2. 分解：
   - state expansion；
   - conflict check；
   - hash；
   - sort；
   - coefficient addition；
   - allocation；
3. 输出 flamegraph 或等价 profiler 结果；
4. 给出前两个热点。

### Gate P3

报告能回答：

- 时间主要花在哪里；
- 内存主要由什么占据；
- 状态数还是大整数首先成为限制；
- 哪个优化方向最有可能产生收益。

---

## Phase 4：低风险 kernel tournament

### 目标

选择一个主执行后端。

### 候选实验

- Python tuple dict；
- Python packed int dict；
- compiled flat hash；
- compiled sort-reduce；
- fused transition；
- bigint；
- 单个 61-bit 模数。

### 实验纪律

- 每次只改变一个变量；
- 使用相同 \(N\)、硬件和线程数；
- 每个短任务至少重复三次；
- 使用 median wall time；
- 首次运行可作为 warm-up；
- 必须验证 count/residue 一致。

### 主评价指标

1. wall time；
2. peak RSS；
3. states/s；
4. bytes/state；
5. 最大可达 \(N\)。

### 选择规则

候选成为主后端需至少满足一项：

- 速度提升 \(\ge2\times\)；
- 内存降低 \(\ge30\%\)；
- 最大可达 \(N\) 增加至少 1；
- 明显降低相邻尺寸增长比。

### Gate P4

选出：

- 一个主后端；
- 一个独立验证后端；
- 一份被淘汰方案及淘汰理由。

---

## Phase 5：收缩顺序研究

### 目标

比较至少三种真正不同的 contraction ordering。

### 最低实验集合

1. row-by-row；
2. snake single-site；
3. min-fill 或 weighted min-fill；
4. 有余力加入 diagonal。

### 每条路径记录

- induced/frontier width；
- dense cost estimate；
- peak sparse support；
- candidate transitions；
- peak RSS；
- wall time；
- 最大成功 \(N\)。

### 分析

绘制：

1. logical width vs \(N\)；
2. peak support vs \(N\)；
3. time vs \(N\)；
4. memory vs \(N\)；
5. predicted cost vs measured time；
6. support density vs layer。

### Gate P5

至少得到一个可证伪结论，例如：

- min-fill 降低 dense width，但增加 sparse support；
- snake 减少峰值中间状态；
- row order 因约束传播更强而实际最优；
- support-aware model 比 dense FLOP 更能预测时间。

如果所有新路径都更差，保留负结果并回到 row-by-row 主线。

---

## Phase 6：有限域、CRT 和双重精确验证

### 目标

建立大规模运行的严格算术与冗余校验。

### Agent 动作

1. 生成并记录多个互素机器字素数；
2. 对已知 \(N\) 分别计算 residues；
3. 检查模数乘积 \(>N!\)；
4. CRT 重构；
5. 与 bigint 结果比较；
6. 使用额外第三模数做 spot check；
7. 每个 run 保存：
   - modulus；
   - residue；
   - code SHA；
   - task manifest hash。

### Gate P6

- \(Q(8),Q(16),Q(20)\) 能由 CRT 唯一恢复；
- bigint 与 CRT 完全一致；
- unique reconstruction check 自动执行；
- 任何模数缺失都会让 reducer 拒绝声明最终结果。

---

## Phase 7：对称分片与并行

### 目标

形成可扩展、可恢复的任务图。

### Agent 动作

1. 首行镜像约化；
2. 固定前 \(k\) 行生成合法 prefix；
3. 为每个 prefix 写 task manifest；
4. 估计每个 task 难度；
5. 动态调度/work stealing；
6. 每个 task 独立输出：
   - prefix；
   - symmetry weight；
   - modulus；
   - partial count；
   - runtime；
   - peak memory；
   - checksum；
7. reducer 检查：
   - 任务完整；
   - 无重复；
   - 权重正确；
   - partial sum 一致；
8. 加入 checkpoint/restart。

### 评价指标

- serial fraction；
- speedup；
- parallel efficiency；
- load imbalance；
- P50/P90/P99 task time；
- checkpoint overhead；
- failed/retried task ratio。

### Gate P7

- 分片与不分片结果一致；
- 不同 \(k\) 的分片结果一致；
- kill/restart 后结果一致；
- 多核/多节点效率可量化；
- 长任务有持续 progress output。

---

## Phase 8：正式验收基准 \(Q(20)\) 与 \(Q(27)\)

### 目标

达到 Issue 的正式最低验收线。

### 执行顺序

1. 再次从 clean checkout 构建；
2. 固定 release config；
3. 完成 \(Q(8)\)；
4. 完成 \(Q(16)\)；
5. 完成 \(Q(20)\)；
6. 资源估计后提交 \(Q(27)\)；
7. 至少用两个模数；
8. 对选定 slices 用独立 backend spot check；
9. CRT 重构；
10. 与 OEIS 对照。

### \(Q(27)\) 声明条件

只有全部满足才可标记 `verified`：

- 最终整数等于 OEIS；
- 模数乘积大于 \(27!\)；
- 所有 task 完成；
- reducer manifest 无缺失/重复；
- 两个独立 correctness path 通过；
- 日志、环境和 git SHA 完整。

### Gate P8

复现以下四个值：

| \(N\) | \(Q(N)\) |
|---:|---:|
| 8 | 92 |
| 16 | 14,772,512 |
| 20 | 39,029,188,884 |
| 27 | 234,907,967,154,122,528 |

未完成 \(Q(27)\) 时，可以形成阶段性研究成果，但不能声称已满足 Issue 的完整 acceptance。

---

## Phase 9：\(Q(28)\) workload projection

### 目标

在启动完整运行前获得可信成本估计。

### Agent 动作

1. 固定 \(N=28\) prefix depth；
2. 生成完整 task manifest；
3. 按估计难度分层抽样；
4. 至少覆盖：
   - 中位难度；
   - P90；
   - P99；
   - 预测最重任务；
5. 对每层运行足够样本；
6. 拟合 task time 和 memory 分布；
7. 计算：
   - total core-hours/GPU-hours；
   - peak per-task memory；
   - longest tail；
   - checkpoint storage；
   - 多模数倍数；
   - 独立验证额外成本。

### 不允许

仅用 \(T(27)\times T(27)/T(26)\) 单一点外推宣布成本。  
必须结合 slice 分布和实际 \(N=28\) 样本。

### 输出

- `q28_manifest.jsonl`
- `q28_sampling_results.csv`
- `q28_projection.md`
- 置信区间/上下界
- go/no-go recommendation

---

## Phase 10：\(Q(28)\) go/no-go gate

### Go 条件

全部满足：

1. \(Q(27)\) 已严格复现；
2. 两种 exact arithmetic/check 一致；
3. 完整 task manifest 已冻结；
4. 总成本在可用预算内；
5. 最重 task 不超过单任务 wall/memory cap；
6. checkpoint/retry 已通过故障注入测试；
7. reducer 能检测缺失、重复和损坏；
8. 预留至少一次关键 slice 独立复核资源。

### No-go 条件

任一发生：

- 预计成本远超可用资源；
- 重尾任务不可控；
- 单 task 超内存；
- Q27 尚未验证；
- arithmetic/reducer 仍有歧义；
- 无法证明方法仍是 Sec. VI exact contraction。

### No-go 时的正确交付

- 最大已成功 \(N\)；
- Q28 完整任务图；
- 样本 partial exact counts；
- 预计资源；
- 主要 exponent/bottleneck；
- 需要怎样的算法突破才能可行。

这完全符合 Issue “否则报告最大 \(N\) 及下一步为何失败”的科学目标；但正式 acceptance 仍应至少包含 Q27。

---

# 6. Benchmark 协议

## 6.1 本地同硬件比较

最低比较：

| 方法 | exact | TN 主方法资格 | 作用 |
|---|---:|---:|---|
| brute force/permutation | 是 | 否 | 小规模 oracle |
| bitmask DFS | 是 | 否/对照 | 经典算法基线 |
| symmetry bitmask DFS | 是 | 否/对照 | 优化经典基线 |
| sparse row-transfer | 是 | 是，需等价证明 | 主 exact TN baseline |
| float MPS/MPO | 否 | 否 | 低秩方法失败对照 |
| proposed backend | 是 | 是 | 核心贡献 |

## 6.2 公平性要求

所有本地 benchmark 固定：

- CPU/GPU 型号；
- 核数/线程数；
- 内存；
- compiler 和 flags；
- Python/C++/CUDA 版本；
- arithmetic backend；
- modulus；
- symmetry；
- prefix depth；
- warm-up；
- 重复次数；
- wall-time 测量方法；
- peak RSS 测量方法。

## 6.3 外部文献比较

至少讨论：

- FPGA carry-chain 方法如何利用 \(D_4\) symmetry 和专用逻辑；
- GPU DFS 如何做大规模前缀并行；
- 通用 multi-GPU exact TN contraction 如何处理中间张量分布和通信；
- 本方法与它们的算法差异；
- 不同硬件不能直接用 wall time 给出公平 speedup。

## 6.4 Scaling 图

必须至少有：

1. wall time vs \(N\)；
2. peak memory vs \(N\)；
3. peak support vs \(N\)；
4. total transitions vs \(N\)；
5. 相邻尺寸增长比；
6. 各阶段 time fraction；
7. 与 bitmask DFS 的同硬件对比。

建议拟合：

\[
\log T(N)=aN+b
\]

和：

\[
\log S_{\max}(N)=cN+d.
\]

必须同时报告 fit window，避免只给单个指数。

---

# 7. Autoresearch 决策逻辑

Agent 每完成一个实验，按以下顺序判断：

## 7.1 正确性优先

1. 是否与 oracle/residue 一致？
2. 是否仍是严格 exact？
3. 是否保留 Sec. VI 等价性？
4. 若否：立即回滚，不进入性能比较。

## 7.2 性能收益分类

将结果分为：

- **结构性收益**：降低增长指数或 peak support；
- **内存结构收益**：降低 bytes/state 或 peak RSS；
- **常数收益**：提高 transitions/s；
- **并行收益**：提高总吞吐但不改变单任务复杂度；
- **无收益**：差异在测量噪声内；
- **负收益**：更慢或更耗内存。

## 7.3 继续条件

一个方向只有在满足以下之一时继续投入：

- 在至少两个连续 \(N\) 上稳定改善；
- 使最大可达 \(N\) 增加；
- 提供新的可解释科学结论；
- 明显改善 Q27/Q28 资源投影。

## 7.4 停止条件

立即停止某方向：

- exactness 无法证明；
- 小规模即慢两个数量级且没有结构压缩；
- 只在单个 \(N\) 偶然更快；
- 开发成本超过剩余时间；
- 与主线重复；
- 需要新基础设施但预计不能在黑客松内验证。

---

# 8. 推荐的两三天执行优先级

## Day 1：正确性与基线

### 必须完成

- Phase 0–2；
- Sec. VI truth-table；
- direct small TN；
- brute force；
- bitmask DFS；
- integer sparse row-transfer；
- 连续已知值验证；
- benchmark harness；
- 初步 profiling。

### 当天结束应有

- 一键运行测试；
- 至少到 \(N=16\) 的正确结果；
- 三种方法一致；
- 第一个 time/memory scaling 图；
- 明确主瓶颈。

## Day 2：核心算法改进

### 优先顺序

1. packed key；
2. compiled backend；
3. fused transition；
4. hash vs sort-reduce；
5. row vs snake/min-fill；
6. modular arithmetic。

### 当天结束应有

- 一个明确优于 baseline 的 exact 后端；
- 至少三种收缩路径比较；
- support-aware 分析；
- CRT 原型；
- 选择最终主方法。

## Day 3：并行、规模推进与报告

### 优先顺序

1. prefix slicing；
2. symmetry；
3. parallel execution；
4. checkpoint；
5. 最大 \(N\) 推进；
6. Q27 feasibility；
7. Q28 sampling projection；
8. challenge report/PR 整理。

### 现实预期

两三天内未必能完成 Q27，更不应承诺 Q28。  
但 Agent 的实现和研究报告必须为后续持续计算做好：

- exact arithmetic；
- resumable tasks；
- deterministic manifests；
- scalable benchmark；
- 明确的 go/no-go gate。

---

# 9. 推荐代码架构

```text
src/
├── tensor_definition.py
├── direct_contract.py
├── brute_force.py
├── dfs_bitmask.py
├── sparse_transfer.py
├── state_encoding.py
├── arithmetic/
│   ├── bigint.py
│   ├── mod64.py
│   └── crt.py
├── orderings/
│   ├── row.py
│   ├── snake.py
│   ├── diagonal.py
│   └── min_fill.py
├── parallel/
│   ├── slicing.py
│   ├── manifest.py
│   ├── worker.py
│   └── reduce.py
├── metrics.py
└── cli.py

cpp/
├── packed_state.hpp
├── hash_backend.cpp
├── sort_reduce_backend.cpp
└── bindings.cpp

tests/
├── test_constraint_line.py
├── test_local_tensor.py
├── test_direct_contract.py
├── test_known_counts.py
├── test_equivalence.py
├── test_modular_crt.py
├── test_symmetry.py
├── test_slicing.py
└── test_restart.py
```

## CLI 建议

```bash
nqueens-tn solve \
  --n 16 \
  --backend sparse-hash \
  --ordering row \
  --arithmetic bigint \
  --threads 1 \
  --output tracks/peps/results/<run-id>
```

```bash
nqueens-tn solve \
  --n 27 \
  --backend packed-sort-reduce \
  --ordering row \
  --arithmetic mod \
  --modulus-file configs/primes.txt \
  --slice-depth 5 \
  --task-manifest q27.jsonl
```

```bash
nqueens-tn project \
  --n 28 \
  --manifest q28.jsonl \
  --sample-policy stratified \
  --output q28_projection
```

---

# 10. 结果文件与元数据

## 10.1 `run.json`

遵循 harness 现有 normal run schema；不要擅自创建不兼容顶层格式。至少确保能表达：

- challenge issue 34；
- exactness；
- method description；
- arithmetic；
- \(N\)；
- ordering；
- backend；
- hardware；
- git SHA；
- start/end time；
- primary result；
- verification status；
- runtime/memory；
- figure paths；
- uncertainty/remaining bottleneck。

## 10.2 `benchmarks.csv`

建议字段：

```text
run_id
git_sha
N
method
backend
ordering
arithmetic
modulus
symmetry
slice_depth
threads
count_or_residue
verified
elapsed_s
cpu_s
peak_rss_bytes
processed_states
candidate_transitions
unique_states_peak
hardware_id
```

## 10.3 `layer_metrics.csv`

建议字段：

```text
run_id
N
layer
input_states
candidate_transitions
accepted_transitions
unique_output_states
merge_ratio
support_density
expand_s
merge_s
arithmetic_s
total_s
peak_rss_bytes
```

## 10.4 长任务进度

每个长任务必须：

- stdout line-buffered；
- 约 10–50 次有意义的 progress update；
- 每完成一个 slice 立刻写入 manifest；
- 输出 running total/residue；
- 输出 completed/total tasks；
- 输出 ETA 仅作为动态估计；
- kill 后最多损失一个 slice。

---

# 11. 必须交付的文档

## 11.1 README

包含：

- 问题定义；
- Sec. VI 等价性；
- 构建和运行；
- exactness 声明；
- arithmetic；
- benchmark；
- 最大 \(N\)；
- Q28 状态；
- 目录结构；
- PR 复现步骤。

## 11.2 Exactness proof

`docs/exactness.md`：

1. 局域约束；
2. 完整网络权重；
3. row-transfer equivalence；
4. state merge correctness；
5. symmetry weighting；
6. modular arithmetic；
7. CRT uniqueness；
8. final verification protocol。

## 11.3 Scaling report

`docs/scaling.md`：

- time；
- memory；
- support；
- growth fit；
- bottleneck；
- 与 DFS/FPGA/GPU 比较；
- Q28 projection。

## 11.4 Negative results

`docs/negative_results.md`：

记录：

- 浮点 MPS 截断失败；
- 不优的 contraction ordering；
- 无效 cost model；
- 被 kill 的 ZDD/exact MPS 原型；
- 失败原因和指标。

负结果可避免后续 Agent 重复消耗时间。

---

# 12. 正式交付检查表

## 科学正确性

- [ ] 使用 Sec. VI 张量和边界 convention；
- [ ] 17 个局域非零 entries 验证；
- [ ] 等价重构有证明；
- [ ] 最终结果不依赖浮点阈值；
- [ ] \(Q(8)\) 正确；
- [ ] \(Q(16)\) 正确；
- [ ] \(Q(20)\) 正确；
- [ ] \(Q(27)\) 正确；
- [ ] 至少两种 exact check；
- [ ] OEIS 数据使用正确版本；
- [ ] 没有误用 Issue 中的 Q25 typo。

## 工程复现

- [ ] clean checkout 可构建；
- [ ] tests 通过；
- [ ] 单命令运行；
- [ ] 环境和 hardware 记录；
- [ ] seed/config 固定；
- [ ] 结果目录完整；
- [ ] 长任务可恢复；
- [ ] reducer 检查缺失和重复任务。

## 性能报告

- [ ] wall time；
- [ ] peak memory；
- [ ] state/support scaling；
- [ ] bottleneck；
- [ ] fit window；
- [ ] 同硬件 DFS benchmark；
- [ ] FPGA/GPU 文献比较；
- [ ] 最大成功 \(N\)；
- [ ] Q28 result 或 projection。

## Harness 提交

- [ ] solution 位于正确 PEPS track/team folder；
- [ ] report 通过 harness report workflow 生成；
- [ ] PR 指向 quantum.harness；
- [ ] PR 引用 Issue #34；
- [ ] 不提交大型原始 checkpoint；
- [ ] 所有关键结果可从提交代码重新生成。

---

# 13. Agent 的推荐首个具体任务

Agent 开始执行时，第一项研究任务应是：

> **实现并验证 Sec. VI 局域张量与 integer sparse row-transfer 的严格等价性，同时建立 brute-force、DFS 和 direct-TN 三重小规模 oracle。**

在此任务通过前：

- 不写 GPU；
- 不做路径搜索；
- 不启动大 \(N\)；
- 不优化 MPS；
- 不尝试 Q28。

通过后，第二项任务是：

> **在相同逐行 contraction 上比较 tuple-dict、packed-key hash 和 sort-reduce，找到时间和内存主后端。**

第三项任务是：

> **比较 row、snake、weighted min-fill 的 logical width、actual support 和 wall time，判断是否存在结构性 contraction-order 改进。**

---

# 14. 最终研究叙事

即使没有得到 \(Q(28)\)，一份高质量成果应能回答：

1. Sec. VI 网络怎样被严格收缩？
2. 普通浮点 MPS 为什么不是可靠的 exact solver？
3. exact contraction 的复杂度主要来自：
   - 边界宽度；
   - 实际 support；
   - 系数位宽；
   - hash/sort；
   - 还是通信？
4. 稠密 contraction cost 是否能预测稀疏网络的真实成本？
5. 哪种表示和 ordering 最有效？
6. 与经典 DFS 相比，TN 路线的优势和劣势是什么？
7. 复现 Q27 需要多少资源？
8. Q28 在当前方法下是否可行？
9. 若不可行，需要降低哪个增长指数或哪类中间状态？

最佳标题可以是：

> **Exact Sparse Contraction of the N-Queens Tensor Network: Support-Aware Ordering, Modular Arithmetic, and a Feasibility Study of \(Q(28)\)**

---

# 15. 参考资料

1. [Quantum Harness Issue #34](https://github.com/QuantumBFS/quantum.harness/issues/34)
2. Z.-Y. Liu, H.-J. Liao, L. Wang, [Statistical mechanics of the N-queens problem](https://arxiv.org/abs/2605.10326), Sec. VI.
3. [Reference implementation: LiuZY613/nqueen-lattice-gas](https://github.com/LiuZY613/nqueen-lattice-gas)
4. [OEIS A000170](https://oeis.org/A000170)
5. T. B. Preußer and M. R. Engelhardt, Putting Queens in Carry Chains, No. 27.
6. G. Yao and Y. Li, High-performance N-queens solver on GPU, arXiv:2511.12009.
7. F. Pan et al., Parallelizing Large-Scale Tensor Network Contraction on Multiple GPUs, arXiv:2606.01852.
8. [Quantum Harness AGENTS.md](https://github.com/QuantumBFS/quantum.harness/blob/main/AGENTS.md)

---

# 附录 A：Autoresearch 主循环伪代码

```text
load official requirements
lock source versions
initialize experiment registry

run Phase 1 exactness audit
if not pass:
    stop and repair tensor conventions

run baselines
if exact methods disagree:
    minimize failing N
    debug before optimization

profile baseline
rank bottlenecks

for direction in prioritized_research_directions:
    define hypothesis
    define one-variable experiment
    define metrics and kill criterion
    run smallest discriminating instance
    verify exactness
    if exactness fails:
        reject direction
    elif no stable benefit:
        archive negative result
    else:
        test on next two N
        if benefit persists:
            merge into main backend
        else:
            archive as non-general improvement

establish finite-field + CRT verification
establish slicing + checkpoint
reproduce Q8, Q16, Q20

estimate Q27 cost
if feasible:
    run Q27
    independently verify
else:
    report research result but mark official acceptance incomplete

if Q27 verified:
    build Q28 manifest
    sample Q28 task distribution
    compute resource projection
    apply go/no-go gate

if go:
    run Q28 with checkpoint and redundant exact checks
else:
    report largest N and precise limiting bottleneck

generate challenge report
prepare PR
```

---

# 附录 B：每个实验的预注册模板

```yaml
experiment_id:
question:
hypothesis:
baseline_commit:
candidate_commit:
N_values:
hardware:
threads:
backend:
ordering:
arithmetic:
moduli:
symmetry:
slice_depth:

primary_metric:
secondary_metrics:
correctness_oracle:
pass_threshold:
kill_condition:
expected_resource:
actual_resource:

result:
verification:
decision:
artifact_paths:
notes:
```

---

# 附录 C：官方 acceptance 与内部阶段成功标签

建议 Agent 使用以下状态，避免夸大进展：

- `TENSOR_CONSTRUCTION_VERIFIED`
- `SMALL_N_EXACT_VERIFIED`
- `Q20_VERIFIED`
- `OPTIMIZED_EXACT_BACKEND`
- `Q27_FEASIBLE_NOT_RUN`
- `Q27_VERIFIED`
- `Q28_PROJECTED_INFEASIBLE`
- `Q28_RUNNING_CHECKPOINTED`
- `Q28_VERIFIED`
- `OFFICIAL_ACCEPTANCE_READY`

只有达到：

- documented exact TN code；
- 四个最低 benchmark，包括 Q27；
- scaling/comparison report；
- harness PR ready；

才标记：

```text
OFFICIAL_ACCEPTANCE_READY
```

---

## 第二次五方向 gate：E6–E10 已完成

E6–E10 已全部在独立 worktree/branch 中执行，第二次强制复盘也已完成。完整假设、commit、
benchmark 表、RSS/support/work 指标、KEEP/REJECT 原因和修订计划统一记录在
`REPORT.md` 第 13–15 节；机器可读数据镜像在 `benchmarks/e6_*` 至 `benchmarks/e10_*`。

执行结论：

- E6 compiled row operator：KEEP；
- E7 exact weighted ADD/ZDD：REJECT；
- E8 generic direct-TN ordering：REJECT；
- E9 exact sort-reduce：KEEP，并成为默认串行 backend；
- E10 exact parallel slicing：KEEP，16 threads 为推荐吞吐配置。

十轮后仍没有方向降低 N=14 peak support 5,479,934，也没有超过同线程 DFS。下一阶段不再
把 hasher、reserve 或增加线程数作为主方向；优先验证 geometry-aware cutwidth 和可证明的
future-equivalence quotient。开始 E11 前必须沿用 `REPORT.md` 第 15 节的 keep/kill gates。

---

## 基于 `Q&A.md` 三轮讨论与 scaling 实测的第三阶段修订

### 结论：必须深化稀疏性；对称性必要但不是充分条件

当前 backend 已经只存储非零 boundary states，因此“引入稀疏性”不能理解成把 dense tensor
换成 hash/vector——这一步已经完成。下一阶段必须利用更深的、张量值相关的稀疏结构：

1. 不再扫描显式为零的 row positions；
2. 合并 future-equivalent sparse states；
3. 用 actual `nnz/support` 而不是 dense width 驱动 ordering；
4. 只在有限域 rank 确认存在精确线性依赖后发展 exact MPS。

对称性也应引入，但要准确估计上限。逐行 boundary 保持的棋盘自同构主要是左右反射
\(Z_2\)；旋转和上下反射通常不保持“已收缩前 k 行”这一 cut。因此单独的 symmetry slicing/
orbit canonicalization 通常最多提供约 2x 常数收益。DFS baseline 已经使用首行镜像约化，
所以 symmetry 是公平比较所必需的，但不能单独填平当前差距。

### Scaling 对优先级的约束

commit `cccc5211ee15e8bcf20c283142e1597be9776db8` 的受控 release benchmark 显示：

- PEPS N=10→15：peak support 8,838→32,120,057，几何平均每 N 增长 5.15x；
- PEPS row candidates 308,110→1,783,273,650，几何平均增长 5.66x；
- DFS N=11→16：recursive nodes 89,878→563,208,896，几何平均增长 5.75x；
- N=15 PEPS 生成 1,783,273,650 candidates，而 DFS 只有 91,883,698 candidate
  placements，work ratio 为 19.4x；
- N=15 串行 PEPS 约 10.8 ns/candidate，DFS 约 5.4 ns/candidate，单项常数只差约 2x。

因此主要差距是“生成了多少 sparse work”，不是整数加法或单个 entry 的成本。进一步调
allocator/hasher 的优先级降到最低。

当前 packed boundary 只有 `3N` 个二值 virtual indices，所以 support 严格满足
\(S_{\max}\le2^{3N}\)，整套逐行算法也有
\(\operatorname{poly}(N)2^{3N}=\exp(O(N))\) 上界。N=10–15 上
`exp(bN)`、`exp(bN log N)`、`exp(bN²)` 的回归都很接近；不能把窄区间内略高的 \(R^2\)
解释为真正的 \(\exp(N^2)\) 渐近律。原始数据与拟合在：

- `benchmarks/current_scaling_peps_release.csv`
- `benchmarks/current_scaling_dfs_release.csv`
- `benchmarks/current_scaling_model_fits.csv`

### E11–E15 新顺序

| ID | 方向 | 稀疏/对称机制 | 可行性 | 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---:|:---|:---|
| E11 | **从显式 C 编译 sparse legal-position iterator** | 从 occupied `CEntry` 的 incoming predicate 机械生成 bitwise valid-position mask，只枚举非零 row transitions | 很高 | 2/5 | N=14、15 serial 与 16t 均至少 3x；输出 map/support/weight 逐层完全一致 | runtime <1.5x 或任何 tensor-level mismatch |
| E12 | **完整 D4 action、stabilizer 与 orbit contraction** | 显式实现 8 个棋盘自同构；对每个 cut/slice 求 stabilizer，分别利用 cut-preserving orbit 与跨方向 task reuse | 中高 | 4/5 | N=13–15 aggregate work 至少下降 35%，runtime 至少 1.5x；逐 orbit exact | 只有最终答案盲乘 multiplicity、无法处理 fixed orbit，或收益 <20% |
| E13 | **width-certified support-aware path search** | line-graph/carving-width bounds + OMEinsumContractionOrders `TreeSA`/slicing 候选 + sampled/actual nnz 重评分 | 中 | 4/5 | 两个连续 N 的 actual peak-support slope 比 row sweep 低至少 10%，且 count exact | 仅 dense FLOP/width 改善；actual support 连续两档未降 20% |
| E14 | **future-equivalence sparse quotient** | 用 suffix behavior/bisimulation/Myhill–Nerode signature 合并 bitmask 不同但后续行为相同的 states | 中低 | 5/5 | N=10、11 canonical classes ≤ explicit support 的 70%，native apply ≤2x E11 | 两档 class ratio >0.85 或证明成本先爆炸 |
| E15 | **有限域 flattening-rank 诊断** | 在多个 61-bit primes 上测 selected cuts 的 exact rank；只有 rank slope 显著更低才发展 CRT exact MPS | 中低 | 5/5 | 至少两个 N 的 rank/support ≤0.5，且 rank slope 低 ≥15% | rank 接近 support，立即拒绝 exact MPS 主线 |

### 方向解释

#### E11 为什么优先于所有其他方向

E6 的 `CompiledRowOperator` 已从 17-entry `C` 证明 occupied branch 的全部 incoming 条件，
但 runtime 仍对每个 parent 扫描全部 N 列。N=15 实测：

\[
1\,783\,273\,650\ \text{column candidates}
\quad\rightarrow\quad
143\,138\,637\ \text{accepted tensor transitions}.
\]

稀疏 iterator 的理论检查数可下降约 12.46x。实现必须消费 `CompiledRowOperator` 生成的
predicate，并对 N≤10 每个可达 parent 与原 compiled/sitewise 输出 map 比较；不得直接写一个
无 tensor 来源的 DFS `available_columns` recurrence。

#### E12：尽量使用完整 D4，但按 stabilizer 分层

完整棋盘与 Sec. VI 边界条件在 D4 的 8 个元素下不变：恒等、90/180/270 度旋转、水平/
垂直反射和两条对角反射。实现必须显式生成这些作用对 site coordinate、row/column channel
和两族 diagonal channel 的置换，并在 tensor level 验证 transformed \(B/C\) 与 boundary
contraction 等价。

但一个“已完成前 k 行”的中间 cut 通常只被左右反射保持。其余 D4 元素会把它映射成：

- bottom-up row sweep；
- left/right column sweep；
- 旋转或反向的另一 contraction task；
- 某个带 anchor/slice 条件的不同 sector。

所以不能强行把每层都除以 8。E12 分三层：

1. **cut stabilizer quotient：** 对 row sweep 使用左右反射 canonical boundary orbit；
2. **cross-task reuse：** 让旋转/上下反射复用 row/column、top/bottom 的 kernel、profile、
   checkpoint 或 anchored slice；
3. **full-solution orbit：** 只有在能用局域/有限状态 symmetry-breaking automaton 正确处理
   orbit size 1/2/4/8 时才尝试 fundamental-domain contraction。

首行左右镜像可以用 projected tensor slice 表示并按 orbit multiplicity 求和。进一步的
boundary canonicalization 必须证明 transfer 与 stabilizer action 可交换：

\[
R\,T = T\,R.
\]

只在最终答案上盲目乘 2/4/8 不算合规 symmetry contraction。报告必须逐 sector 记录 group
element、stabilizer、orbit size、fixed-point 数、multiplicity 和独立 unsymmetrized 校验。
cut 内部的直接 quotient 上限通常仍约 2x；完整 D4 的主要额外价值是跨方向/切片复用，而
不是保证 8x。

#### E13 的 OMEinsum 路径探索定位

当前 [OMEinsum](https://github.com/under-Peter/OMEinsum.jl) 本体要求用 nested einsum 手工
指定 contraction order；自动路径搜索在
[OMEinsumContractionOrders.jl 文档](https://under-peter.github.io/OMEinsum.jl/dev/contractionorder/)
中提供 `optimize_code`、`TreeSA`、复杂度评分和 slicing。
E13 可以把 Sec. VI 网络结构导出成 einsum/graph，使用这些优化器产生 contraction-tree
候选和 dense time/space/read-write 上界。

这些结果只能作为候选生成器。每条候选必须回到 Rust direct-TN oracle，用 actual sparse
support、accepted transitions、RSS 和 exact count 重新评分。不得因为 OMEinsum 的 dense
complexity 下降就 KEEP；路径搜索优先级低于 E11/E12。

#### E13–E15 才可能改变指数

E13 优化 actual sparse separator；E14 寻找张量值相关的最小状态数；E15 检查 exact linear
rank 是否远低于 support。它们分别对应 `Q&A.md` 提出的三类可能突破。E7 已否定普通固定变量
顺序 ADD/ZDD，E8 已否定朴素 diagonal wavefront，所以不得在没有新结构证据时重复这两类
实验。

### 降级或停止的路线

- dense exact MPS/PEPS、无截断 boundary-MPS：不作为主线；
- SVD/CTMRG/rounding：违反 exactness；
- reserve、hasher、allocator 微调：仅可作为通过 gate 的结构方向的配套；
- 继续增加 CPU threads 或直接上 GPU：E10 已显示 16→32 threads 基本饱和，在 sparse work
  数下降前不再单独立项；
- 通用 dense contraction ordering：只提供 fixed-network 下界/上界，不代表特殊 17-nnz
  tensor 的真实成本。

完成 E15 后必须再次执行 five-direction review gate；在此之前不得启动 E16。

## M. E11–E15 强制五方向复盘（2026-07-28）

本节在 E11–E15 全部完成后写入，满足 five-direction review gate；以下新顺序取代先前所有
E16 以后优先级。复盘时 main baseline 为 `cb227b0` 的 exact D4 orbit contraction，所有实验
均有独立 worktree/branch、自包含报告和 raw CSV。

### M.1 结果总表

| 方向 | 核心结果 | 机制判断 | 决策 |
|:---|:---|:---|:---|
| E11 C-derived sparse iterator | N=15 checks 1.783B→143.1M（12.46x），serial 仅 1.38x，16t 1.04x | predicate 已非主瓶颈；candidate write/sort/merge 主导 | REJECT standalone |
| E12 D4 orbit contraction | N=13–15 serial 1.78–1.93x；support 降 43–48%；N=15 RSS 2.88→1.63 GiB | 只有 `{I, vertical reflection}` 保持 interior row cut；首行 projected slices 真正减半后续 work | KEEP，已作为默认 |
| E13 Rust simple path search | greedy 胜 generic row tree，但 N=11 support 470,776，production D4 仅 22,253 | site-level tree 暴露过多 open legs；row macro 的 partial evaluation 比简单 OMEinsum-style tree 更重要 | REJECT 当前候选 |
| E14 future-equivalence | N=10–13 peak classes/support 16.3%、15.6%、14.7%、10.6%；replay 极快 | 存在巨大 exact suffix bisimulation；但 concrete graph prebuild 使总时间慢 4.3–6.7x | KEEP 结构证据，非 production |
| E15 finite-field rank | N=10–13 rank/support 30.4%、11.2%、8.4%、3.77%；两素域一致 | boundary coefficient tensor 有强 exact linear low-rank 结构；普通 sparse support 严重高估最小线性状态 | KEEP 新主线，非 production |

### M.2 原假设与实测机制

“必须同时引入稀疏性和对称性”被部分验证，但需要把“稀疏性”重新定义：

1. **局域 transition 稀疏不足。** E11 将检查次数降低一个数量级，却不降低 materialized
   support，故 serial/parallel 都未达到 gate。
2. **D4 是可靠常数项。** E12 的收益几乎完全由 cut-preserving orbit 真正减少 candidate
   与 state 数产生；其余 D4 元素只能复用反向/转置 task，不能虚报 8x。
3. **简单换 contraction tree 不够。** E13 验证了类似 OMEinsum 的 tree/cost 搜索框架很适合
   产生和否决候选，但 production row macro 已利用 tensor 值结构，site-level width 不是公平
   成本模型。
4. **有价值的稀疏性是语义商和线性秩。** E14/E15 首次给出可能改变有效状态增长率的证据。
5. **诊断压缩不等于求解加速。** E14/E15 都先 materialize full support，因此其 replay/rank
   数不能直接拿来与 DFS 比；下一轮唯一目标是避免 full-support prebuild。

E9 的排序 locality、E10 的并行和 E11 的局域 iterator 都不应立即重做。只有 boundary
representation 被 E16 改变后，旧消融结论才可能失效。

### M.3 新主假设

Sec. VI PEPS 的 row boundary 在空间左右 flattening 上具有远低于 sparse support 的 exact
finite-field rank。若把 compiled row operator 表示为由显式 `C` 机械生成的 exact MPO，并在
每行后以有限域行消元/秩分解做**无阈值**压缩，则可能把有效 bond dimension 的增长从约
5x/N 降到实测 2–3x/N。D4 projected first-row slices 应在该表示上继续成立。

反证条件：任何实现若必须先展开完整 boundary support 才能重新分解，便没有 production
价值；若 apply 后的 exact rank 快速回到 support，E15 的低秩只是一种不稳定的事后性质。

### M.4 E16–E20 五个独立方向（结合 D4/tree-search 讨论后的定稿）

| ID | 方向 | exact PEPS 义务 | feasibility / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E16 | **单素域 streaming exact MPS/MPO row apply** | row MPO 必须从 17-entry C 自动生成；从 rank-1 boundary 开始；零阈值 exact compression | 中 / 5 | N=8–10 从头到尾不 materialize full boundary；每层 coefficient/rank 与 sparse oracle 一致；总时间 ≤ direct 的 10x | 任一 production layer 需 full support、rank mismatch，或 N=9 已不可运行 |
| E17 | **exact skeleton/PLUQ two-block factorized apply** | 直接维持 E15 的左右 flattening `U·V`/pivot bases；所有 pivot 与更新在有限域 exact 执行 | 中低 / 5 | N=8–10 不展开完整矩阵，rank 与 E15 一致；至少一档比 E16 的 field ops/RSS 低 20% | 更新必然需要枚举全部 `(L,R)` support，或 factor rank 连续两档高于 oracle 1.5x |
| E18 | **在线 future-signature / quotient apply** | signature 必须等于完整 successor-class map，不能用 completion count 或 hash 碰撞代替证明 | 中低 / 5 | 不预建 concrete suffix graph时，N=10–12 class 数在 E14 的 1.5x 内，build+apply ≤ direct 的 2x | 仍需枚举全部 concrete transitions 两遍，或 class ratio >0.5 |
| E19 | **D4-conditioned macro-tree search** | 外层 sector 必须逐项记录 stabilizer/orbit weight；内层 greedy/tree rotation 以 C-derived row/half-row macro 为原子 | 中 / 4 | same-revision actual nnz/RSS 评分；两个连续 N 相对 D4 row baseline 至少快 15% 或 support 降 20% | 仅 dense `tc/sc` 改善、sector 拆分丢失 merge 后总 work 上升，或两档无收益 |
| E20 | **bidirectional low-rank/quotient separator join** | top/bottom 子网络和 join signature 必须覆盖所有跨 separator virtual bonds；join exact | 低 / 5 | N=10–12 peak live states/rank 比单向 baseline 低 30%，总时间开始出现更低增长率 | join interface/support 爆炸，或任何隐藏 DFS placement recurrence |

这里不再把 CRT、消融或“最终 benchmark”单列为研究方向：它们是每个 KEEP 候选进入
production 的强制工程阶段。任何 E16/E17/E20 候选若进入整数计数，必须使用足以覆盖
\(N!\) 上界的 61-bit prime 乘积、CRT 和至少一个冗余 prime；任何 KEEP 都必须与 D4-only、
对应无 D4 变体、E11 sparse iterator 和相同线程数做消融。

E19 与 D4/tree-search 文档一致，但明确不预注册正收益保证。D4 sector reduction 与 path
search 在数学上可组合；sector 拆分可能丢失跨 sector boundary merge，dense TreeSA score
也可能与 17-nnz tensor 的 actual sparse cost 相反。只允许“候选集中保留当前 D4 row
baseline、实测选择不回退”，不能把“兼容”写成“必然继续加速”。

### M.5 执行顺序与最小区分实验

执行优先级为 E16 → E17 → E18 → E19 → E20。E17 是 E16 的替代低秩表示，即使 E16
REJECT 也允许独立尝试；E19 不依赖低秩成功，避免把所有风险押在单一路线。

E16 不能从 full sparse matrix 做 SVD/Gaussian 后宣称成功。最小合规实验必须：

1. 从 rank-1 top boundary 开始；
2. 把一行 N 个显式 `C` tensor 编译成 exact finite-field MPO；
3. 直接作用于 factorized/MPS boundary；
4. 每个 spatial bond 做无阈值 exact compression；
5. 仅为验证，在旁路 materialize 小 N sparse boundary 并比较所有 coefficient/rank；
6. 分别记录 pre/post rank、field operations、fill-in、wall time、RSS；
7. N=8–10 若不能避免 full support 或两档 rank mismatch，立即停止。

E17 必须从 rank-1/pivot-basis representation 直接更新，不允许调用 E15 的 full-matrix
diagnostic 作为 production step。E18 必须把 quotient construction 与 replay 时间一起计入。
E19 必须同时报告每 sector 和 aggregate 总成本。E20 必须报告 join-key 数、rank、support 和
两侧生成 work。

完成 E20 后再次执行 five-direction review gate；在复盘前不得启动新的第六方向。不得因为
E15 的事后 rank 很低或某个预构建 quotient replay 很快，就提前宣称能计算 Q(28)。

## N. E16–E20 强制复盘后的修订（2026-07-28）

E16–E20 已全部完成，复盘详见 `REPORT.md` §21。实测推翻了“事后 low-rank / quotient
足以变成 production 加速”的假设：

- E16 保持正确 rank 却被 dense elimination 和 wire SWAP 拖慢约 4.6e5x；
- E17 的 PLUQ factor products 在 N=12 达 sparse support 的 261.5x；
- E18 虽把 quotient construction 从两遍 transition 降为一遍，仍需 memoize 全部
  concrete states，时间为 direct 的 2.93–4.44x；
- E19 证明 D4 与 tree search 可组合，但 generic representation 仍远输 full-row macro；
- E20 证明 horizontal bidirectional split 的 bottom v2 interface 在 join 前已经爆炸。

### N.1 新主假设

唯一仍与证据一致的 exponent-changing 路线是：把 boundary 表示为由显式 C 局域 apply
直接维护的 canonical symbolic graph，使等价子函数在 concrete `(left,right)` state
生成前就 hash-cons。D4 projected slices 保留为外层 sector；path search 改为搜索 symbolic
variable tree，而不是裸 site contraction tree。

若 E21 的 canonical node 数仍与 concrete support 同斜率，或 apply 必须先枚举所有 concrete
successors，新主假设即被反证，研究应回到 E24 的 production sparse kernel 与可并行分片，
而不是继续包装 post-hoc compression。

### N.2 E21–E25

| ID | 方向 | exact PEPS 义务 | feasibility / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E21 | **C-derived exact weighted DD/ZDD boundary apply** | terminals 是 checked integer/finite-field coefficients；每个 apply rule 从 17-entry C 机械生成；canonical unique table 无概率等价 | 中低 / 5 | N=10–12 不枚举 full support；peak nodes ≤0.5×D4 support，count/rank 与 oracle 一致；总时间 ≤5× direct | apply cache 仍为每个 concrete state 建节点，或 nodes/support 连续两档 >0.8 |
| E22 | **actual-node variable-tree greedy / simplified treeSA** | 只变更 DD variable tree/macro association；D4 sector 和 C apply 不变 | 中 / 4 | 两个连续 N 相对 E21 baseline nodes 降 20% 或 wall time 降 15%，aggregate sector 成本计全 | 仅 dense width 改善、actual nodes/RSS 不降，或 reorder conversion 吞掉收益 |
| E23 | **channel-aligned hierarchical exact factors at DD leaves** | column/dr/dl tensor-product blocks分别 exact factor；无 SVD/阈值；必须从 rank-1 apply | 低 / 5 | factor products ≤2× symbolic nodes，两个素域 rank 一致，N=10–12 比 E21 RSS 或时间低 20% | 重现 E17 pivot fill-in，products/nodes >10 连续两档 |
| E24 | **D4 production radix/arena/batched-transition kernel** | transition 仍由 compiled C operator 生成；只改容器、排序、分配和批处理 | 高 / 3 | N=13–15 两档以上快 20%，support/count/work 完全相同；对 radix、arena、batch 做消融 | 任一 count/work 改变，或三项组合收益 <10% |
| E25 | **symbolic tilted separator / bottom-v2 automaton** | top/bottom 均为 C-derived symbolic subnetwork；join 覆盖完整 virtual interface | 低 / 5 | 仅在 E21/E23 KEEP 后启动；N=8–10 bottom nodes 与 top 同量级且 live nodes 比单向低 30% | 任何 4^N bottom materialization、bottom/top >4，或 join interface 重现 E20 增长 |

### N.3 顺序、消融和停止规则

执行顺序为 E21 → E22 → E23 → E24 → E25，但：

1. E22 依赖一个正确可复现的 E21 canonical baseline；E21 REJECT 时跳过 E22，并记录
   dependency rejection，不伪造第六方向替代它；
2. E23 可在 E21 仅通过 correctness、未通过性能 gate 时做一次小 N 区分实验；
3. E24 与 symbolic 路线独立，应无论 E21–E23 成败都执行；
4. E25 只有 E21 或 E23 至少一个 KEEP 才允许启动，否则按依赖 kill，不重跑 E20；
5. 每个 symbolic candidate 必须同时记录 unique nodes、terminal count、apply-cache
   lookups/hits、peak live nodes、field/integer operations、wall time、RSS 和 concrete-oracle
   旁路范围；
6. 完成或依赖拒绝 E25 后再次执行 five-direction review，复盘前不得启动 E26。

### 强制消融与“错误 REJECT”复查

从 E11 开始，每个 KEEP 候选必须做同 revision、同编译配置的消融，而不是只与很早的
baseline 比较。至少保留以下矩阵：

| 变体 | C-derived sparse iterator | sort-reduce | D4/cut symmetry | parallel |
|:---|:---:|:---:|:---:|:---:|
| A0 tensor reference | off | off/hash | off | 1 |
| A1 E11 only | on | off/hash | off | 1 |
| A2 E9 only | off | on | off | 1 |
| A3 E11+E9 | on | on | off | 1 |
| A4 E11+E9+E12 | on | on | on | 1 |
| A5 full serial | on | on | on | 1 |
| A6 full parallel | on | on | on | 16 |

要求：

1. N=12–15 至少报告 runtime、RSS、support、generated/accepted transitions；
2. 计算每项的边际收益与关键二阶交互，例如
   `speedup(E11+E9)` 是否接近两者乘积；
3. 若某 KEEP 在新 baseline 上收益 <5%，可降级或撤销；
4. E2 reserve、E4 hasher 在 sparse iterator 改变负载后允许一次低成本复测；
5. E7 fixed-order DD、E8 naive diagonal ordering 只有在 E13/E14 提供新变量顺序或 quotient
   证据时才能复活，不能原样重跑；
6. 每次复活旧方向必须使用新实验 ID、说明原 REJECT 的假设为何失效，并重新预注册 gate。

最终报告必须同时给出 incremental chain 和 ablation matrix，避免把后加入的优化收益错误
归因给前置改动。

## O. E21–E25 强制复盘后的修订（2026-07-28）

E21–E25 的完整复盘见 `REPORT.md` §27。E21 的 canonical DD 与 E23 的
weighted-edge quotient 均未通过 production gate；E22 证明 actual-node
order search 有约 20% node 收益，但 DD 仍比 direct D4 慢 80--105x。
E25 因此按预注册前置条件 dependency reject，未重跑 E20。

E24 是本轮唯一 production KEEP：arena、融合 candidate generation 与
C-derived sparse position iterator 将 N=12--15 serial runtime 改善
2.27--2.46x；固定 8 线程标准排序将原 PEPS baseline 再改善到约 4x。
但 N=15 仍有 18,178,233 peak states、80,077,350 accepted candidates
和约 1.71 GB RSS；同线程 DFS 快 41.68x。

### O.1 新主假设

短期最可检验的假设不再是 post-hoc symbolic compression，而是：

> 保留显式 17-entry C 派生的 exact sparse transition，但用 key-prefix
> partition、局部即时归并和有界 sorted runs 避免全层 candidate 的一次性
> materialization；只有在该 production representation 上，再搜索两行
> macro/tree association。

如果 E26/E27 只能改变排序常数而不能降低 peak candidates/RSS 或增长率，
则 flat row frontier 已达到结构瓶颈；届时必须明确报告无法超过 DFS 的
实测范围，而不能宣称数学上的普遍不可能性。

### O.2 E26–E30

| ID | 方向 | exact PEPS 义务 | 可行性 / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E26 | **key-prefix sharded parallel sort-reduce** | successor 仍由 compiled 17-entry C 生成；按 packed virtual key 的不重叠 prefix 完整分区；每 bucket exact checked reduce | 高 / 2 | N=14/15 至少一项：同线程时间降 20% 或 RSS 降 30%；count/support/work 完全一致 | 最佳 8--256 buckets 改善 <10%，或额外 partition pass 抵消收益 |
| E27 | **parent-chunk sorted runs + exact k-way merge** | 每个 run 覆盖一段完整 parent transitions；最终按 key exact 合并所有 run，无丢弃/近似 | 高 / 3 | N=15 peak RSS 降 40%，时间回退不超过 20%，count/support/work 一致 | run metadata/merge 比全层多 2x 时间，或仍保留全部 candidates |
| E28 | **row-aware compact key + SoA coefficients** | 动态省略的 bits 必须由该 row 的 v0/v2/shift 唯一恢复；u128 coefficient 独立 checked | 中高 / 3 | 在 E26/E27 上 N=14/15 RSS 再降 20% 或时间降 15% | 重现 E24 deferred：RSS 上升或 N=15 收益 <5% |
| E29 | **two-row C-derived macro apply** | 明确收缩两行所有局域 C 与内部 virtual bonds；不得写 queen-pair recurrence | 中 / 4 | N=10--13 output support 不增，accepted candidates/row-equivalent 降 25% | N² macro branches 或中间 support 超单行 2x 连续两档 |
| E30 | **actual-cost sharded macro tree search** | 只搜索 E26/E29 合法 exact macro association；D4/boundary/arithmetic不变 | 中 / 4 | 两档 actual candidates、RSS 或 wall time 降 15%；搜索成本计入 | 仅 estimated width 改善，actual cost 不降，或搜索开销吞掉收益 |

### O.3 顺序与消融

1. 严格按 E26 → E27 → E28 → E29 → E30，每项独立 worktree/branch；
2. E26 比较 1/8 threads，并与 E24 `arena_batched_sparse` /
   `parallel_sort` 同 revision 消融；
3. E27 必须报告 peak live candidates、run count、merge heap operations；
4. E28 同时报告 `size_of`、active key bits、boundary bytes 与 candidate bytes；
5. E29 必须以 explicit sitewise C contraction 小 N 真值表验证 macro；
6. E30 可借鉴 OMEinsum/treeSA 的 cost-search 思路，但只实现本项目需要的
   greedy/local moves，暂不引入 Julia runtime；
7. 完成 E30 后再次执行 five-direction review，复盘前不得启动 E31。

### O.4 当前进度

E26 已完成并 **KEEP Prefix/256**。N=14/15 相对 E24 同线程全局 sort
分别快 24.5%/32.5%（8 threads），N=15 RSS 降 26.6%；count、support
和 C-derived transition work 完全一致。完整报告见
`experiments/e26_prefix_sharded_reduce/REPORT.md`。

下一方向严格为 E27 parent-chunk sorted runs + exact k-way merge；不得
把继续调 shard 数量计为新方向。

E27 已完成并 **REJECT**。N=15 RSS 仅降 5.1%，时间回退 74.8%，
148,116,850 次 heap operations 成为新瓶颈；更小 chunk 没有进一步
降低 RSS。被拒实现不进入 production main。下一方向按顺序为 E28
row-aware compact key / SoA coefficients。

## P. E26–E30 强制复盘后的修订（2026-07-28）

完整复盘见 `REPORT.md` §33。E26/E28 KEEP，E27/E29/E30 REJECT。
当前 production baseline 为 E28：N=15 在 8 threads 下
1.61179 s / 946,487,296 bytes，exact count/support/transitions 不变。

### P.1 新判断

- key sharding 和 24-byte layout 的收益来自真实 cache/memory traffic，
  应继续保留；
- retained runs 不能避免 input runs 与 output 同时存活；
- canonical merge 不能随意延后，否则 local C apply 重复；
- macro tree search 只有在候选 edge 本身有 actual work 优势时才值得；
- 当前 row frontier 尚未构成数学不可能性证明，需要替代 cut 与
  certified distinguishability 两类证据。

### P.2 E31–E35

| ID | 方向 | exact PEPS 义务 | 可行性 / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E31 | **parallel compact candidate generation** | 每个 worker 完整消费 E28 C-derived parent shards；thread-local buckets 的并集与 serial candidate multiset 完全一致 | 高 / 2 | N=14/15 8t time 降 20%，RSS 增幅 <30%；count/support/work identical | speedup <10% 或 thread-local capacity 令 RSS 增 >50% |
| E32 | **checked u64 coefficient fast path + exact promotion** | 每次 add/mul checked；任一 overflow 自动重放/提升该 layer 为 u128；强制 promotion 测试 | 高 / 3 | N=14/15 RSS 或 time 再降 20%，所有 counts 一致 | 无法证明 promotion 完整，或收益 <10% |
| E33 | **out-of-place compact shard LSD radix** | key/value permutation 完整；与 standard sort 的 exact multiset/reduction 对照 | 中高 / 3 | 两档 time 降 15%，RSS 增幅 <25% | 重现 E24 radix：两档变慢或 scratch 抵消内存收益 |
| E34 | **corner/diamond explicit-C contraction path** | frontier 完整覆盖 cut virtual bonds；所有 exposed endpoints 正确施加 v0/v1/v2；D4 stabilizer 明确 | 中低 / 5 | N=10--12 peak support/candidates 比 row cut 降 30%，count 与 oracle 一致 | active interface 或 support 连续两档 >2x row baseline |
| E35 | **certified row-frontier distinguishability audit** | 用 exact two-prime signatures 后附确定性 witness/replay，不能把 hash collision 当证明 | 中 / 5 | 给出 N=10--14 可复查 lower bound、增长拟合和 DFS/PEPS 资源投影；或发现可合并 classes ≥30% 且在线可构造 | 只能给 post-hoc probabilistic quotient，无法认证或无法在线构造 |

### P.3 强制规则

1. E31--E33 都以 E28 Prefix/256、24-byte entry 为同 revision baseline；
2. E31 必须分别报告 generation/sort/reduce wall time和 thread-local bytes；
3. E32 必须用人工小位宽触发 promotion，与 u128 baseline 逐层比较；
4. E33 对相同 candidate buffers 做 standard/radix 消融；
5. E34 先画出并测试 cut interface，不得把 DFS state recurrence叫作 corner PEPS；
6. E35 若不能给确定性 witness，只能称 empirical bound，禁止宣称“不可能”；
7. 完成 E35 后执行下一次 five-direction review，复盘前不得启动 E36。

## Q. E31–E35 强制复盘后的修订（2026-07-28）

完整复盘见 `REPORT.md` §34。E31/E32 KEEP 为 production，E33
radix REJECT，E34 diamond ordering REJECT 但 generic explicit-C
frontier oracle KEEP，E35 certified audit KEEP 为 diagnostic、REJECT
为 production。

当前 E32 production 在同 revision、8 threads 下 N=13--15 分别为
0.03242/0.12237/0.82142 s；DFS comparator 为
0.002363/0.010423/0.068610 s，差距约 11.7--13.7x。与上一检查点不同，
PEPS accepted C transitions 已比 DFS placements 少 8--13%；主要差距
已从 work count 转为 16-byte records 的生成、跨线程汇合与
sort/reduce traffic。

E35 认证 N=10--14 row-frontier coarsest exact weighted-bisimulation
class peak从 735 增到 313,373，区间 fit base 4.444；但 class 只能在
访问 concrete graph 后构造。该结果不构成所有 PEPS paths 的下界。

### Q.1 新主假设

下一轮同时验证两个假设：

1. 当前 production 仍有一次 2x 级的 layout 机会：在常用 N 上将
   `3N`-bit virtual key 与小 coefficient 共同 pack 为一个 u64，并
   保留 checked promotion；再复用跨行 bucket arenas。
2. 晚层必须避免 materialize + sort 全部 candidate records。先 merge
   若干 row，然后递归收缩剩余 C-derived row tensors，可能把每个分支
   的成本逼近 register-only DFS；完整 D4 只有在这种 full-solution
   recursion 中才可能通过 orbit/stabilizer canonical augmentation
   超过 row-cut 的二阶 stabilizer。

第二条不是把 conventional DFS 改名为 PEPS：递归 successor 必须由
`CompiledRowOperator::compile(SiteTensorC::sec_vi())` 机械生成，每步
保留 v0/v1/v2 与 exact coefficient，并对 sitewise explicit-C oracle
测试。独立 `dfs_bitmask` 仍只作 comparator。

### Q.2 E36–E40

| ID | 方向 | exact PEPS 义务 | 可行性 / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E36 | **joint u64 key/coefficient packing + promotion** | `3N` key bits可逐位恢复；剩余 bits 存 coefficient；所有 add/mul checked，任一超界从初始 v0 boundary 重跑 E32 u64-coefficient 或 u128；人工小位宽强制两级 promotion | 高 / 3 | N=14/15 RSS 或 time 再降 25%；entry=8 bytes；count/support/work identical | promotion 链不完整，或两档收益 <10%，或排序无法按 key 正确 reduce |
| E37 | **cross-row arena/bucket capacity reuse** | 只复用已完成上一层的 Vec allocations；候选 multiset、prefix partition、checked reduce 不变；不得让旧 coefficient 泄漏 | 高 / 2 | E36 KEEP 时在 E36 上、否则 E32 上，N=14/15 time 降 10% 且 RSS 不增；记录 allocation/capacity reuse | allocator retained pages 使收益 <5%，或双 buffer 令 RSS 增 >20% |
| E38 | **C-derived recursive tail contraction** | prefix 仍为 exact sparse PEPS；tail 每层调用由 17-entry C 编译并全测试的 row relation，终点施加 column v1/diagonal v2；不得调用 `dfs_bitmask` 或复制其 handwritten recurrence | 中高 / 4 | cut grid 中两档比 E32 快 3x，或与 DFS gap ≤3x；Q(0..16) exact；记录 recursive C nodes/accepted entries/RSS | 最佳 cut 不快于 E32，或 operator/vector overhead 使每 node >5x DFS，或 fidelity 测试失败 |
| E39 | **full-D4 canonical augmentation for recursive sectors** | 显式 8 个 coordinate/channel actions；partial prune 仅在可证明非 canonical 时；complete orbit size 按 stabilizer 精确给 1/2/4/8，逐 orbit 与无对称 E38 对照 | 低 / 5 | N=12--15 recursive nodes 降 40% 或 time 降 25%；D4 ablation（none/vertical/full）完整 | 只能终点 canonicalize、不能安全 early prune、orbit weights 错，或实际 node 降 <15% |
| E40 | **actual-cost adaptive merge→tail path** | 只搜索 E36/E37 exact merge rows 与 E38/E39 exact recursive tail 的切换；搜索成本计入；每个 candidate 的 C work/count 可 replay | 中 / 4 | 同机 N=14/15 相对 E32 再快 2x，且至少一档达到 DFS 1.2x 内或超过 DFS；RSS < E32；完整消融 | actual cost 预测连续两档选错，最佳 gap 仍 >3x，或 search overhead 吞掉收益 |

### Q.3 顺序、消融与停止规则

1. 严格按 E36 → E37 → E38 → E39 → E40；每项独立 worktree/branch；
2. E36 必须报告每行 maximum coefficient、available coefficient bits、
   fast-path/promotion layer、8/16/24-byte entry 消融；
3. E37 报告 Vec allocations、reused capacity、peak live/retained bytes；
4. E38 至少测试 pure merged E32、pure C-recursive、每个可行 cut 的
   hybrid；recursive node 必须能逐层与 compiled-C transition replay；
5. E39 明确 row-prefix 的 D4 stabilizer 与 full recursion 的 D4 action
   不同，禁止在 row frontier 上直接乘 8；
6. E40 的搜索空间先限于 row cuts 和少量 D4 modes；只有 actual
   C-derived work 存在差异时才加入 greedy/treeSA moves；
7. checkpoint benchmark 固定 Ryzen 9 7945HX、release/thin-LTO、
   8 threads；DFS comparator warmup/repeat policy与 §34 一致；
8. 完成 E40 后强制复盘，允许完全推翻 Q.1；复盘前不得启动 E41。

## R. E36–E40 强制复盘后的修订（2026-07-29）

完整复盘见 `REPORT.md` §35。E36/E37/E38/E40 KEEP；E39 full-D4
production REJECT、diagnostic KEEP。E40 含 selector overhead 的 N=14/15
为 0.01197/0.07122 s，DFS 为 0.01007/0.06600 s，差距已降到
1.19x/1.08x。N=16 因系统双峰不能认证 crossover。

### R.1 新判断

1. sparse explicit-C transition 是必要基础；
2. vertical reflection 因第一行即可决定，收益稳定；full D4 相对它
   只有约 1.3% node 增益，当前不值得进入 hot loop；
3. merged prefix 已缩到 4,811/7,426 sectors，晚层 sort 不再是主瓶颈；
4. 当前 PEPS 与 DFS 的 accepted work 几乎相同，剩余差距是 task seed、
   terminal recursion、checked accumulation 和 scheduling 常数；
5. Q(28) 与小 N crossover 是两条不同目标：前者还要求 N>21 exact
   key/coefficient backend 和大幅 work reduction。

### R.2 E41–E45

| ID | 方向 | exact PEPS 义务 | 可行性 / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E41 | **prefix-free C-derived sector seeding** | 从 explicit-C compiled relation 直接生成 vertical-orbit task sectors；每个 task 覆盖不重叠 contraction branches；与 E40 merged-prefix sectors 的 weighted union/count replay | 高 / 3 | N=14/15 median 比 E40 降 8%，或至少一档稳定超过 DFS；RSS 不增 | task duplication 改变 accepted work/count，或收益 <3% |
| E42 | **certified last-k tensor microkernel** | 只对剩余 2--4 行枚举并编译完整 C contraction；terminal table 必须逐 boundary 与 generic recursive C replay；checked exact coefficient | 高 / 3 | N=14--16 两档 wall 降 8%；code-size/compile-time 可控 | table/branch overhead 使两档收益 <3%，或只复制 DFS terminal trick而无 C certificate |
| E43 | **actual-cost task ordering/chunk search** | 只重排 E41/E42 exact sectors；所有 tasks 完整消费一次；搜索/采样成本计入 | 中高 / 3 | 降低 N=16 p90 15% 且 N=14/15 median 降 5%；跨顺序复测稳定 | 只改善有利执行顺序，median/p10 无收益，或 atomics/deques 增加 RSS >25% |
| E44 | **bounded exact recursive transposition table** | key 含完整 remaining-depth virtual boundary；cache value checked exact；hit/miss 逐项与无 memo C recursion replay | 中 / 4 | N=16--18 nodes 或 wall 降 20%，cache RSS 受预算约束 | hit rate <10%、hash overhead >收益，或 memory scaling 重现 full frontier |
| E45 | **N>21 wide-key + finite-field/CRT promotion** | u128 virtual key 或分离 key；素数模运算逐 prime exact，CRT modulus product 超过可证明 count bound；小 N 与 integer backend 一致 | 中 / 5 | 完成 N=17--21 scaling，证明 N=22+ backend 可用；两素数/整数消融完整；更新 Q28 projection | CRT bound 不足、promotion 不可 replay、或 backend 在 N<=18 慢 >2x 且无扩展收益 |

### R.3 顺序、消融与停止规则

1. 严格按 E41 → E42 → E43 → E44 → E45；每项独立 worktree/branch；
2. E41 必须把“无 prefix merge”写成 contraction association 变化，并逐
   task replay compiled C；不得调用或复制 `dfs_bitmask`；
3. E42 必须报告 k=1/2/3/4 消融、binary size 和 compile time；
4. E43 至少比较 Rayon flat tasks、atomic 1/4/16/64 chunks 和一次
   hardness ordering；报告 median/min/p10/p90，禁止挑单次 crossover；
5. E44 的 cache budget、hits、lookups、peak entries/RSS 必须预注册；
6. E45 在使用 CRT 前先给 Q(N) 上界与所需 modulus bits；浮点重构禁止；
7. E39 full D4 只有在 E41/E44 产生新的早期-comparable sectors 时才可用
   新 ID 复活，且必须证明相对 vertical 的增量收益，不得引用 vs none；
8. E30 tree search 不得原样复活；只有 E41--E44 提供至少两个 actual-cost
   不同的合法 contraction edges 时，才考虑简化 greedy/treeSA；
9. 完成 E45 后强制复盘；在此之前不得启动 E46。

## S. E41–E45 强制复盘后的修订（2026-07-29）

完整复盘和反作弊审计见 `REPORT.md` §36。E41 prefix-free、E43
scheduling、E44 transposition table REJECT；E42 certified last-4 KEEP
为最快 production；E45 wide key/CRT KEEP 为 optional exact/low-memory
backend。

最新同批 control 中 E42 在 N=15/17 分别比 DFS 快约 6.2%/5.2%，但
N=18 又慢 12.3%。这已经反证“任何合规 PEPS 都不可能超过当前 DFS
baseline”，却没有给出更优 scaling。E45 把 N=18 RSS 从 E42 的
600.7 MB 降到 7.3 MB，但双 residue traversal 慢约 27%；Q(28) 当前
投影仍约 1,600 年。

### S.1 复盘后主假设

下一轮不复活 full D4、普通 memoization 或高成本 treeSA：

1. E39 已证明 full D4 相对 vertical 只再减少约 1.3--1.8% nodes；
2. E44 证明 row-recursive states 的可复用率低于 1%；
3. E43 证明当前任务顺序不是主要长尾来源；
4. E41 证明 prefix seed/merge 已不是时间主体；
5. E42 唯一显著收益来自减少最宽 tail 层的指令、栈和分支常数。

新的最强可检验假设是：

> 对 N<=20，`N! < 2^64` 给出全局非负 contraction count 的确定性上界。
> 因而可把 E45 的低内存 direct sectors 与单路 checked-u64 结合，避免
> E45 的双 residue lanes；随后继续扩大由 C 认证的 terminal supernode，
> 消除递归栈，并在独立 sectors/residue lanes 上做可回放的 SIMD。

这条路线首先追求在 N=18 稳定超过 DFS，同时保留 E45 的低 RSS。它只做
常数优化，不假装解决 Q(28) 的指数；若 E46--E50 全部完成后 observed
base 仍不低于 DFS，下一次 review 必须重新寻找改变 contraction exponent
的表示，而不是继续无限微调 hot loop。

### S.2 E46–E50 五个方向

| ID | 方向 | exact PEPS 义务 | 可行性 / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E46 | **wide direct sectors 的 certified scalar-u64 backend** | tasks 仍逐步应用 explicit-C 编译的 `RecursiveTailRelation`；N<=20 启动前证明 `N!<=u64::MAX`，每次加/乘仍 checked；人工 coefficient limit 强制回退 E45 CRT/generic replay | 高 / 2 | N=16--18 至少两档比 E45 two-lane 快 25%；N=18 达 DFS 的 1.05x 内或更快；RSS <25 MB | 任一 overflow/promotion 无法从同一 C sectors replay，或两档收益 <12%，或 RSS >50 MB |
| E47 | **C-generated last-5/last-6 terminal supernode** | 从 `CertifiedSecViTailPlan` 生成完整 5/6-row contraction tree；每个 leaf 明确施加 column v1、diagonal v2；逐 boundary 与 generic recursive C 对照，禁止复制 DFS 的特例表 | 中高 / 3 | k=4/5/6 消融中，N=16--18 两档相对 E46 再快 8%；binary size/compile time 增幅 <25% | 两档收益 <3%、代码尺寸失控，或任一 boundary replay mismatch |
| E48 | **iterative fixed-stack C-tail machine** | 只把 E47 相同 C-derived DFS tree 的调用栈显式化；访问顺序、accepted entries、checked reduction 与 recursive reference 完全相同 | 高 / 3 | N=17/18 median 降 10% 或 p90 降 15%，RSS 不增 10%；node/entry counters identical | 两档收益 <5%、显式 stack 增加 branch/store，或 work counters 改变 |
| E49 | **跨 direct sectors 的 exact SIMD/batched traversal** | 每个 lane 对应完整、不重叠的 C-derived boundary task；lane compaction 必须保留 orbit weight 和 exact sum；scalar lane-by-lane replay 为 oracle | 中 / 4 | AVX2/portable batch 相对 E48 在 N=16--18 两档快 15%，vector utilization 与 fallback 比例完整记录 | divergence/compaction 令两档收益 <8%、unsafe path 无法逐 lane 验证，或 RSS 增 >25% |
| E50 | **3/4-prime residue-lane SIMD CRT** | prime、N! bound、CRT reconstruction 与 E45 不变；只向量化同一 C contraction 的 modular add/mul；forced 3/4-lane 与 scalar CRT逐 residue 相等 | 中 / 4 | N=17/18 forced 3/4-prime 至少快 25%，并更新 N=21--28 资源投影；所有 residues/reconstruction exact | 两档收益 <10%、mod reduction 吞掉 SIMD，或任何 residue mismatch |

### S.3 执行、消融和停止规则

1. 严格按 E46 → E47 → E48 → E49 → E50；每项独立
   worktree/branch，main 只合入 KEEP 实现和所有实验报告；
2. E46 同 revision 比较 E42 merged scalar、E45 direct two-lane、E46
   direct scalar，固定相同 task target/threads；不得把旧 CSV 混作新实现；
3. E46 必须记录 `N!` bound、selected arithmetic backend、promotion
   reason、tasks、recursive nodes/accepted C entries、wall、RSS；
4. E47 报告 k=4/5/6 的 binary size、clean release compile time和
   N=14--18 runtime；微内核只能在 C certificate 成功后进入；
5. E48 同时保留 recursive reference，逐 N 比较 task count、node count、
   accepted entries 和 count；metrics replay 不进入 uninstrumented wall；
6. E49 必须报告 active-lane histogram、lane compactions、scalar fallbacks
   和有效 SIMD occupancy；不能只报告一个有利 N；
7. E50 无需等待数小时完成 N=21；先用 N=17/18 强制 3/4 primes 做
   exact throughput 消融，再据实更新 Q(28) 投影；
8. full D4 只有出现比 E39 更早可比较的新 cut/lane grouping 时才能以
   新 ID 复活；必须报告相对 vertical 的增量，不能引用 vs none；
9. treeSA 只有出现至少两个 actual C-work、materialization 或 SIMD
   utilization 真正不同的合法 contraction edges 时才进入候选；last-k
   的 4/5/6 grid 不需要通用路径搜索；
10. checkpoint 继续固定 Ryzen 9 7945HX、rustc 1.94、release/thin-LTO、
    8 threads，并与同批 DFS 比较 median/p10/p90；所有 raw CSV 保存在
    `benchmarks/`；
11. 完成 E50 后必须执行下一次 five-direction review；复盘前不得启动
    E51。当前检查点严格停在 E45，E46 尚未开始。

## T. E46–E50 强制复盘后的修订（2026-07-29）

完整复盘见 `REPORT.md` §37。E46 direct scalar-u64 和 E47 last-6
KEEP；E48 fixed stack、E49 cross-sector AVX2、E50 explicit residue
AVX2 REJECT。当前 production PEPS 在 N=14--18 比仓库 DFS comparator
快约 21--25%，窗口 geometric wall base 约 6.92 vs DFS 7.00；这不是
渐近证明，但已经稳定超过同硬件 baseline。

### T.1 新判断

1. 后端/association 仍比手写低层机器优化更重要；
2. E47 last-6 尚未干净迁移到 E45 的 N>20 scalar CRT path；
3. last-5/6 收益表明 last-7/8 仍值得一次有 kill gate 的扩展；
4. E13/E30 只否定 dense/site-level cost model，未否定由 actual sparse
   macro cost 驱动的离线 treeSA；
5. treeSA 搜索时间可按用户要求排除，但获胜路径的 contraction execution
   不能排除任何 setup/materialization/reduction；
6. 不同 N 的最优路径是否共享 normalized cut/tree motif 必须单独验证，
   不能从小 N 最优直接假定可迁移到大 N。

### T.2 E51–E55

| ID | 方向 | exact PEPS 义务 | 可行性 / 难度 | keep gate | kill gate |
|:---|:---|:---|:---:|:---|:---|
| E51 | **clean scalar-CRT last-6 migration** | E45 1--4 prime backend 的每个 node 使用 E47 certified last-6；prime/N!/CRT 证明不变；不得合入 E50 intrinsics | 高 / 2 | forced 2/3/4 lanes 在 N=16--18 两档相对 last-4 快 10%；residues/work identical；scalar-u64 path 不回退 | 两档收益 <5%、任一 residue mismatch，或代码把 AVX candidate 混入 |
| E52 | **C-generated last-7/last-8 terminal supernode** | 由 `CertifiedSecViTailPlan` 组合完整 7/8-row subtree；逐 boundary 与 generic recursive C replay；checked promotion 保留 | 中高 / 3 | k=6/7/8 中 N=16--18 两档再快 8%；binary/compile 增幅 <30% | 两档收益 <3%、code-size explosion，或 replay mismatch |
| E53 | **actual-sparse macro treeSA（离线搜索）** | tree leaves/edges 只允许 explicit-C site、half-row、row、terminal macros；每个候选 tree 覆盖所有 virtual bonds 和 v0/v1/v2；winning tree exact execute | 中低 / 5 | N=10--16 至少两档 winning-path execution 比 E52 baseline 快 15% 或 peak support/materialization 降 25%；search time不计 | 只有 dense proxy 改善、actual execution不降，或 interface 无法 exact materialize |
| E54 | **跨 N tree motif 提取与冻结迁移** | 从 E53 各 N 最优树提取 normalized cut depth、subtree balance、macro sequence；目标 N=17/18 禁止重新搜索，只加载冻结 motif | 低 / 5 | transferred path 在 N=15--18 距 per-N searched best ≤5%，且两档比 row baseline 快 10%；给出可复查共性规律 | 小 N trees 无稳定 motif、迁移回退 >10%，或 target 必须重新 treeSA |
| E55 | **treeSA-cut 上的 D4 stabilizer 消融** | 仅当 E53/E54 产生非 row cut 时启动；显式计算该 cut 的 D4 stabilizer/orbit weights；none/vertical/full 与同一 tree 对照 | 低 / 5 | 相对 treeSA+vertical 两档 nodes 降 20% 或 wall 降 15%，所有 orbit counts exact | 新 cut 仍只有 row-cut stabilizer、增量 nodes <10%，或任何 orbit replay mismatch |

### T.3 treeSA 搜索协议

1. 先以 N=10--12 的 explicit-C exact executor校准每种 macro edge 的
   actual output support、candidate entries、peak live bytes 和 kernel
   ns/entry；
2. treeSA state 是合法 contraction tree；move set 仅含 subtree rotation、
   macro regroup 和合法 separator exchange，不改 tensor values；
3. objective 同时报告 estimated cost 与 winner 的 actual execution；
   禁止只凭 treewidth/FLOP proxy KEEP；
4. 搜索时间、proposal 数和 seed仍记录到 raw CSV，但不计入用户指定的
   solver wall comparison；
5. 每个 N 至少使用多个 deterministic seeds，保存 winner tree 的
   machine-readable serialization、cut widths 和 actual sparse trace；
6. E54 的 source N 与 target N 预先分离；看到 N=17/18 结果后不得回改
   motif extraction rule；
7. treeSA winner仍必须通过 B/C 17-entry、boundary、generic-C replay、
   independent oracle 和 known Q(N) tests。

### T.4 执行与停止规则

1. 严格按 E51 → E52 → E53 → E54 → E55，每项独立 worktree/branch；
2. E53 即使 REJECT，也必须保留 searched trees、seeds、estimated/actual
   cost scatter 和失败机制；不能只留最佳数字；
3. E54/E55 的 dependency 不成立时按 dependency REJECT 归档，不临时
   偷换成第六方向；
4. full D4 的比较基线必须是相同 tree 的 vertical mode，不得引用 vs none
   制造增益；
5. checkpoint 继续固定 Ryzen 9 7945HX、rustc 1.94、release/thin-LTO、
   8 threads；raw CSV 存 `benchmarks/`；
6. 完成 E55 后执行下一次 five-direction review；复盘前不得启动 E56。
   当前检查点停在 E50，E51 尚未开始。

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

## D. 研究方向重新排序

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

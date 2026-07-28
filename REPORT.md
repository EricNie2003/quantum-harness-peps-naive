# Sec. VI N-Queens PEPS 的 Rust naive 精确收缩

## 1. 结论

本实现显式构造并收缩 Liu、Liao、Wang 在
*Statistical mechanics of the N-queens problem*（arXiv:2605.10326v2）
Sec. VI 定义的局域张量：

- rank-9 张量 \(B\)：8 条 dimension-2 virtual legs 和 1 条 physical leg；
- \(B\) 恰有 17 个非零元；
- 对 physical index 求和得到 rank-8 张量 \(C\)，同样恰有 17 个非零元；
- 每个格点通过查询 \(C\) 的稀疏非零元完成收缩；
- 行、列和对角线端点分别使用论文的 \(v_0,v_1,v_2\)；
- 全程使用 checked `u128`，没有浮点、SVD、截断或对称剪枝。

Release benchmark 正确复现了 \(Q(4)\) 到 \(Q(14)\)。在测试机器上：

- \(Q(8)=92\)：三次中位 0.981 ms，峰值 RSS 5.32 MiB；
- \(Q(12)=14\,200\)：三次中位 0.575 s，峰值 RSS 37.30 MiB；
- 初始 naive \(Q(14)=365\,596\)：三次中位 25.192 s，峰值 RSS 986.34 MiB；
- 完成 E1–E10 后的串行 production baseline（E9 exact sort-reduce）：
  \(Q(14)\) 五次中位 3.085 s，峰值 RSS 444.28 MiB，相对初始 naive 约 8.17x 加速；
- E10 exact parallel slicing：\(Q(14)\) 在 16/32 线程分别为 0.625/0.605 s，
  但同线程 DFS 分别为 0.00631/0.00489 s，仍慢 99.1x/123.8x；
- 十个方向均未降低 N=14 的 peak sparse support 5,479,934，因此尚未达到超过 DFS
  或计算 \(Q(28)\) 的目标。

另加入了一个严格分离的、传统 DFS bitmask comparator。它不是 PEPS 实现，也不参与上述
张量收缩路径。native release benchmark 中，DFS 单线程 \(Q(16)\) 的 9 次中位数为
3.153 s；16 线程 \(Q(16)\) 为 0.211 s，\(Q(17)\) 为 1.884 s。

## 2. Sec. VI 局域张量

每个格点有四条有向约束通道：

\[
(u,d),\quad(l,r),\quad
(d^{\rm in}_{\searrow},d^{\rm out}_{\searrow}),\quad
(d^{\rm in}_{\swarrow},d^{\rm out}_{\swarrow}).
\]

每条腿的维数为 2，信号 0 表示沿该约束线尚未遇到皇后，信号 1 表示已经遇到皇后。
physical index \(\alpha\in\{0,1\}\) 表示格点为空或被占据。

### 2.1 Rank-9 张量 \(B\)

代码中的 `SiteTensorB::sec_vi()` 逐项生成 Eq. (16) 的非零元。

当 \(\alpha=0\) 时，每条通道独立透传：

\[
(x_{\rm in},x_{\rm out})=(0,0)\ \text{或}\ (1,1).
\]

四条通道共有 \(2^4=16\) 种组合。当 \(\alpha=1\) 时，四条通道都必须满足：

\[
(x_{\rm in},x_{\rm out})=(0,1),
\]

产生唯一的占据项。因此 \(B\) 一共有 \(16+1=17\) 个非零元，且值都为 1。

### 2.2 Rank-8 张量 \(C\)

计数时把 physical leg 与 \((1,1)^T\) 收缩：

\[
C_{ud,lr,\ldots}=\sum_{\alpha=0}^1 B^\alpha_{ud,lr,\ldots}.
\]

`SiteTensorC::from_b` 实际执行该求和并合并相同 virtual tuple。空格项与占据项没有重复，
所以 \(C\) 仍恰有 17 个非零元。

测试分别断言：

- \(B\) 有 16 个 \(\alpha=0\) 项和 1 个 \(\alpha=1\) 项；
- 空格项的四条通道均为透传；
- 占据项的四条通道均为 \(0\to1\)；
- \(C\) 有 17 个非零元；
- 输入信号全 0 时，\(C\) 同时存在“空格”和“放皇后”两个输出分支。

## 3. 网络方向与边界向量

实现采用以下方向：

- row：从左到右；
- column：从上到下；
- \(\searrow\) diagonal：从左上到右下；
- \(\swarrow\) diagonal：从右上到左下。

方向只决定约束信号的传播顺序；对角线整体反向并同时交换起止边界不会改变“至多一枚皇后”
的约束。

论文的边界向量被显式落实为：

\[
v_0=(1,0),\qquad v_1=(0,1),\qquad v_2=(1,1).
\]

- 每条线的 incoming endpoint 使用 \(v_0\)，初始信号必须为 0；
- row 的 outgoing endpoint 使用 \(v_1\)，每行必须输出信号 1；
- column 的棋盘底部使用 \(v_1\)，每列必须输出信号 1；
- diagonal 的 outgoing endpoint 使用 \(v_2\)，输出 0 或 1 都被接受。

边缘处新进入棋盘的 diagonal signal 固定为 0；离开棋盘的 diagonal signal 被 \(v_2\)
求和并丢弃。

## 4. Naive contraction 设计

### 4.1 边界表示

完整收缩从上到下进行。已收缩区域和未收缩区域之间的开放 virtual boundary 表示为：

```rust
struct BoundaryState {
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
}
```

三个 mask 只保存下一行真正进入格点的 virtual indices。其精确系数存储在：

```rust
HashMap<BoundaryState, u128>
```

这是稀疏边界张量，不是 DFS 调用栈。

### 4.2 逐格点应用 \(C\)

对每个输入边界态，一行从左到右收缩。左端 row signal 由 \(v_0\) 固定为 0。
在每个格点读取四个 incoming indices：

```text
column_in, row_in, diag_dr_in, diag_dl_in
```

当前实现先在 `SiteTensorC::from_b` 中遍历显式 `SiteTensorC.entries`，机械地按四个
incoming virtual bits 生成 16 个索引桶。收缩时查找对应桶，并把每个匹配 `CEntry` 的
四个 outgoing indices 写入新边界。新增测试对全部 16 个 signature 验证桶内容与线性过滤
`C.entries()` 完全一致。行末只保留 `row_out=1` 的项，即与 \(v_1\) 收缩。

不同父项产生同一 outgoing virtual boundary 时，其 `u128` 系数在哈希表中精确相加。
最后一行后，只保留所有 column signals 均为 1 的状态；剩余 diagonal signals 由
\(v_2\) 无条件求和。

这一实现没有把局域张量替换为 `available_columns` 或 queen bit recurrence。代码仍消费
从显式 \(C\) 机械生成的完整 `CEntry`；`tensor_entries_examined` 在初始 naive 版本中表示
线性扫描的 17 项，在 E1 之后表示索引桶中实际检查的条目。因此 benchmark 中记录了：

- `tensor_entries_examined`；
- `tensor_entries_matched`；
- `completed_row_terms`；
- `output_states`。

## 5. 正确性验证

测试包含两条相互独立的最终计数检查：

1. PEPS contraction 与内置 A000170 常量 \(Q(0)\ldots Q(10)\) 比较；
2. PEPS contraction 与独立的朴素棋盘枚举 oracle 在 \(N=0\ldots9\) 比较。

此外还单独验证局域 \(B/C\) truth table 和最终 column/diagonal 边界。

```powershell
cargo test --release
```

完成 E10 后，实测 17 个 release tests 全部通过；其中 DFS 额外对
\(Q(0)\ldots Q(16)\) 做已知值核验，并验证 1 线程和 4 线程结果一致、不同任务拆分的
processed-state 指标一致。

| N | PEPS 结果 | 已知结果 | 状态 |
|---:|---:|---:|:---:|
| 4 | 2 | 2 | 通过 |
| 5 | 10 | 10 | 通过 |
| 6 | 4 | 4 | 通过 |
| 7 | 40 | 40 | 通过 |
| 8 | 92 | 92 | 通过 |
| 9 | 352 | 352 | 通过 |
| 10 | 724 | 724 | 通过 |
| 11 | 2,680 | 2,680 | 通过 |
| 12 | 14,200 | 14,200 | 通过 |
| 13 | 73,712 | 73,712 | 通过 |
| 14 | 365,596 | 365,596 | 通过 |

## 6. Benchmark 方法

环境：

- CPU：AMD Ryzen 9 7945HX，16 核 / 32 逻辑处理器；
- 内存：34,024,747,008 bytes（约 31.7 GiB）；
- OS/ABI：Windows，`x86_64-pc-windows-msvc`；
- Rust：`rustc 1.94.0 (4a4ef493e 2026-03-02)`；
- LLVM：21.1.8；
- 单线程，release profile，thin LTO；
- 每个 \(N\) 重复 3 次，报告 wall-time 中位数和最小值；
- `elapsed_s` 只包含 contraction，不包含编译、进程启动或 CSV 输出。

命令：

```powershell
cargo run --release -- bench 13 --min 4 --repeats 3 --csv
cargo run --release -- bench 14 --min 14 --repeats 3 --csv
```

峰值内存通过 Windows `GetProcessMemoryInfo` 的 `PeakWorkingSetSize` 获取。它是操作系统记录的
进程 resident working-set 高水位，而不是 Rust allocator 的实时 heap 大小。N=4 至 N=13
在同一进程中按递增顺序执行，因此某一行的 HWM 也包含之前较小问题的运行；由于规模单调增长，
大 \(N\) 的峰值由当前运行主导。N=14 在独立进程中测量。

## 7. 初始冻结 naive benchmark

下表是进行 E1 之前的显式 \(C\) naive baseline，用于衡量之后的单变量优化。它与
`benchmarks/naive_release.csv` 对应，不能当作当前 HEAD 的性能。

| N | Q(N) | 中位时间 (s) | 最小时间 (s) | 峰值 RSS (MiB) | 峰值 support | 检查的 C 元素 |
|---:|---:|---:|---:|---:|---:|---:|
| 4 | 2 | 0.000010 | 0.000009 | 5.12 | 6 | 1,428 |
| 5 | 10 | 0.000025 | 0.000024 | 5.14 | 14 | 5,304 |
| 6 | 4 | 0.000076 | 0.000073 | 5.15 | 43 | 19,958 |
| 7 | 40 | 0.000289 | 0.000232 | 5.20 | 145 | 83,198 |
| 8 | 92 | 0.000981 | 0.000970 | 5.32 | 538 | 359,074 |
| 9 | 352 | 0.004398 | 0.004255 | 5.66 | 2,153 | 1,648,898 |
| 10 | 724 | 0.021062 | 0.020361 | 7.47 | 8,838 | 7,820,816 |
| 11 | 2,680 | 0.110500 | 0.107688 | 14.37 | 39,307 | 39,941,024 |
| 12 | 14,200 | 0.575259 | 0.560782 | 37.30 | 188,100 | 217,658,123 |
| 13 | 73,712 | 3.295454 | 3.291451 | 202.68 | 978,362 | 1,242,983,084 |
| 14 | 365,596 | 25.191505 | 24.076416 | 986.34 | 5,479,934 | 7,498,659,064 |

原始数据位于 `benchmarks/naive_release.csv`。

### 7.1 第一轮 promoted baseline（历史）

第一轮停止点包含三个通过 gate 的改动：

- E1：由显式 \(C\) 自动生成 incoming-signature index；
- E3：三个开放 virtual masks 打包为单个 `u128` hash key；
- E5a：复用逐格点 partial row buffers。

| 阶段 | N=14 中位时间 (s) | 峰值 RSS (MiB) | 峰值 support | 决策 |
|---|---:|---:|---:|---|
| 初始 naive | 25.1915 | 986.34 | 5,479,934 | 冻结基线 |
| E1 后 | 19.3084 | 986.67 | 5,479,934 | KEEP |
| E3 后 | 17.6112 | 666.17 | 5,479,934 | KEEP |
| E5a 后（第一轮停止点） | 11.0897 | 666.17 | 5,479,934 | KEEP |

各阶段的独立原始数据与报告位于 `experiments/e1_c_input_index/`、
`experiments/e3_packed_u128/` 和 `experiments/e5a_partial_buffer_reuse/`。

## 8. 初始 N=14 逐层分析

本节保留 E1 之前的 naive profiling，用于说明 support 峰值位置；局域项检查数和层时间不再
代表当前 HEAD。

一次独立逐层运行总耗时 23.877 s，峰值 RSS 986.32 MiB。

| 行 | 输入状态 | 输出状态 | C 元素检查数 | 层时间 (s) | RSS HWM (MiB) |
|---:|---:|---:|---:|---:|---:|
| 8 | 819,838 | 2,135,353 | 435,448,999 | 1.191 | 349.31 |
| 9 | 2,135,353 | 4,096,606 | 978,042,776 | 3.098 | 790.31 |
| 10 | 4,096,606 | 5,479,934 | 1,631,684,225 | 5.777 | 986.31 |
| 11 | 5,479,934 | 4,715,884 | 1,935,092,881 | 6.203 | 986.31 |
| 12 | 4,715,884 | 2,310,917 | 1,501,374,618 | 4.685 | 986.31 |

峰值 support 在第 10 行输出后出现，最重计算层是第 11 行。虽然第 11 行输出 support 已下降，
它仍需处理前一层的 5,479,934 个输入状态。

从 N=13 到 N=14：

- 时间中位数增长约 7.64 倍；
- 峰值 support 增长约 5.60 倍；
- 峰值 RSS 增长约 4.87 倍；
- 检查的局域 \(C\) 元素数增长约 6.03 倍。

初始 naive 实现每次都线性扫描 17 个非零元。N=14 共检查约 75 亿个局域项，其中约
4.65 亿项匹配 incoming virtual indices。主要瓶颈是边界 support、哈希表内存和 naive
局域项扫描。

完整逐层数据位于 `benchmarks/n14_layers.csv`。

## 9. 复现

```powershell
cargo test --release
cargo run --release -- solve 8 --layers
cargo run --release -- bench 14 --min 4 --repeats 3 --csv
cargo run --release --bin sort_reduce -- sort-reduce 12 14 5
cargo run --release --bin parallel_slicing -- 16 12 14 5
```

## 10. 当前范围

这是严格 Sec. VI PEPS baseline，已经完成 E1–E10 两组研究方向，但尚未达到 Issue #34
的最终性能目标：

- 当前只 benchmark 到 \(N=14\)；
- 未复现 \(Q(16),Q(20),Q(27)\)；
- 未加入有限域/CRT；
- 已实现 exact row-operator compilation、sort-reduce、并行 slicing，并用通用
  direct-TN oracle 比较 row/snake/diagonal ordering；
- 未实现 checkpoint、外存 contraction 或足以降低 support 的结构 quotient；
- 未超过独立 DFS bitmask comparator。

下一步只能按第 15 节修订计划从显式 \(C\) 机械推导，并通过逐项核验，不能退化为把经典
DFS 重新标记成 PEPS。

## 11. 优化 DFS bitmask comparator baseline

### 11.1 定位与实现

`src/dfs_bitmask.rs` 和 `src/bin/dfs_bitmask.rs` 实现一个独立的传统搜索对照。它不构造或
消费局域张量 \(B/C\)，不满足 PEPS 主方法资格，只能作为 oracle/comparator。代码没有从
已知值表返回答案；`known_count` 只在搜索完成后用于验证。

主要优化为：

- 用三个 `u64` bitboard 表示已占列和两组当前行对角攻击位；
- 用 `available & available.wrapping_neg()` 提取最低可用位；
- 首行按左右镜像只搜索一半，奇数 \(N\) 的中心列单独赋权；
- 最后一行直接测试可用位，避免每个解再进入一次递归；
- 单线程不做多余前缀拆分；多线程自适应拆到至少 `64 * threads` 个合法前缀；
- 共享原子 task index 做动态调度，并把估计较重的前缀优先入队；
- 性能路径通过 const generic 完全编译掉逐节点指标更新；每个 benchmark 点另做一次
  instrumented run，且核验其计数与性能路径一致；
- 子树使用 checked `u64` 累加，镜像加权和线程归并使用 checked `u128`，溢出会被检测。

公开接口限制 \(N\le27\)。这一限制与当前已知值验证区间一致，而不是通过查表提前结束搜索。

### 11.2 Benchmark 协议

冻结的搜索代码提交为 `19b9bd7a9f27e3917671512886e7c37471e0c613`。环境沿用第 6 节：

- CPU：AMD Ryzen 9 7945HX，16 核 / 32 逻辑处理器；
- Rust：`rustc 1.94.0 (4a4ef493e 2026-03-02)`，LLVM 21.1.8；
- target：`x86_64-pc-windows-msvc`；
- release profile：thin LTO、1 codegen unit；
- 额外编译 flag：`-C target-cpu=native`；
- 单线程测 \(N=8\ldots16\)，16 线程测 \(N=8\ldots17\)；
- 每点先 warm-up 2 次，再计时 9 次，报告中位数、最小值、P10 和 P90；
- `elapsed_s` 包含前缀生成、worker 创建、DFS 和归并，不包含进程启动或 CSV 输出；
- processed-state 指标由额外一次 instrumented run 取得，耗时记录在
  `metrics_elapsed_s`，不混入性能样本。

精确命令：

```powershell
$env:RUSTFLAGS='-C target-cpu=native'
cargo build --release --bin dfs_bitmask
.\target\release\dfs_bitmask.exe bench 16 --min 8 --threads 1 --repeats 9 --warmup 2 --csv
.\target\release\dfs_bitmask.exe bench 17 --min 8 --threads 16 --repeats 9 --warmup 2 --csv
```

峰值 RSS 与 PEPS benchmark 一样使用 Windows `GetProcessMemoryInfo` 的
`PeakWorkingSetSize`。每组 \(N\) 在一个递增运行的进程内执行，因此是进程 working-set
高水位，会包含先前较小 \(N\) 和线程栈；它不是 allocator heap，也无法分离代码页、栈和
堆。DFS 没有稀疏张量 support，也不检查局域 \(C\) 项，因此 CSV 对
`peak_sparse_support`、`local_tensor_entries_examined` 和
`local_tensor_entries_accepted` 明确记录 `NA`。

### 11.3 结果

所有点均与内置的独立已知值表一致。

| N | Q(N) | 单线程中位 (s) | 16 线程中位 (s) | 单线程 RSS (MiB) | 16 线程 RSS (MiB) | 对称约化搜索节点 |
|---:|---:|---:|---:|---:|---:|---:|
| 8 | 92 | 0.000002 | 0.000703 | 4.93 | 5.59 | 983 |
| 10 | 724 | 0.000042 | 0.000717 | 4.95 | 5.75 | 17,408 |
| 12 | 14,200 | 0.002170 | 0.000993 | 4.95 | 5.93 | 420,995 |
| 14 | 365,596 | 0.070359 | 0.005734 | 4.95 | 6.27 | 13,496,479 |
| 15 | 2,279,184 | 0.510162 | 0.034080 | 4.95 | 6.27 | 90,634,738 |
| 16 | 14,772,512 | 3.153447 | 0.211017 | 4.95 | 6.27 | 563,208,896 |
| 17 | 95,815,104 | 未测 | 1.884083 | 未测 | 6.27 | 4,224,112,371 |

线程创建和前缀拆分使 16 线程版本在 \(N\le11\) 明显慢于单线程；从 \(N=12\) 开始多线程
才有收益。\(N=16\) 的中位加速比为约 14.94x（16 个物理核），说明该规模下动态前缀调度
已经接近充分，但这一结果不能外推为 PEPS contraction 的并行效率。

与第 7.1 节当前 PEPS 的旧测量做同硬件量级比较时，DFS 在 \(N=14\) 的单线程中位数
0.070 s，PEPS 为 11.090 s。两者算法和指标完全不同，而且 DFS 本次使用
`target-cpu=native`、运行日期也不同；因此这里只把它作为约两个数量级的性能目标，不把
比值表述为严格控制变量的 speedup。

原始机器可读数据：

- `benchmarks/dfs_bitmask_single_release.csv`
- `benchmarks/dfs_bitmask_16t_release.csv`

## 12. E1–E5 强制研究复盘

完成 E1、E2、E3、E4a、E5a 后，按照 `AGENTS.md` 的 five-direction review gate 暂停新
实验并完成复盘。完整文档见 `docs/five_direction_review_01.md`。

最重要的新结论是：当前 PEPS 在 N=14 相比单线程 DFS 慢约 157.6x，相比 16-thread DFS
慢约 1933.9x。E1、E3、E5a 分别消除了局域扫描、降低 bytes/state、消除了 partial Vec
分配，但 peak support 始终为 5,479,934。E2 和 E4a 失败说明 reserve 时机和 hasher 计算
不是主要瓶颈。

因此研究优先级已从 HashMap 微调改为：

1. 从显式 \(C\) 自动生成 exact row operator，测量剩余局域开销；
2. exact ZDD/BDD 或未来行为等价类，尝试结构性压缩 boundary support；
3. 只有实际 sparse support 下降时才保留新 ordering；
4. 在 serial gap 显著缩小前，不用并行度掩盖算法差距。

第六方向开始前的所有 review 和修订要求均已记录。

## 13. E6–E10 实验与 benchmark 汇总

第二组五个方向均在独立 Git worktree/branch 中完成。E7、E8 被 gate 拒绝，未把实验代码
合入 production baseline；E6、E9、E10 通过 correctness 和性能 gate 后合入。

### 13.1 十方向总表

| 方向 | 核心假设 | 关键 benchmark / 观测 | 决策 |
|:---|:---|:---|:---:|
| E1 | 从显式 \(C\) 机械生成 input-signature index | N=14 25.1915→19.3084 s；局域扫描减少 93.8% | KEEP |
| E2 | 按 input support 预留 `HashMap` | N=13 仅约 3.4% 改善，RSS 无稳定下降 | REJECT |
| E3 | 三组 virtual masks 打包成 `u128` | N=14 RSS 986.67→666.17 MiB，时间 19.3084→17.6112 s | KEEP |
| E4a | 替换确定性 hasher | N=13 中位数约慢 2.1%，RSS 不变 | REJECT |
| E5a | 复用逐格点 partial-row buffers | N=14 17.6112→11.0897 s，support 不变 | KEEP |
| E6 | 从 17-entry \(C\) 编译 exact row operator | N=14 11.0897→5.8532 s；286,010,088 row candidates | KEEP |
| E7 | weighted ADD/ZDD 压缩 boundary function | N=11 最佳 ZDD 48,302 nodes / 39,307 states=1.23；约慢 15x | REJECT |
| E8 | snake/diagonal direct-TN ordering 降 support | N=6 row/snake=72，diagonal=125；波前 support 高 73.6% | REJECT |
| E9 | exact sort-reduce 替代 hash materialization | N=14 6.0380→3.0850 s；RSS 666.17→444.28 MiB | KEEP |
| E10 | boundary slicing + parallel expansion/sort | N=14：1t 3.1559，16t 0.6249，32t 0.6046 s | KEEP |

十个方向共 6 个 KEEP、4 个 REJECT。所有 KEEP 都保持同一显式 \(B/C\)、边界向量、
exact coefficient 和最终 count；DFS 始终是独立 comparator。

### 13.2 E6：由显式 C 编译 exact row operator

`CompiledRowOperator::compile` 遍历实际 `SiteTensorC.entries()`，只有在确认 16 个
identity pass-through、唯一四通道 \(0\to1\) occupied entry 且所有 coefficient 为 1 时
才成功。N≤8 对每个可达 parent boundary 与逐格点 contraction 的完整输出 map 比较，
N≤10 完整计数比较。

| N | sitewise E5a (s) | compiled E6 (s) | speedup | RSS (MiB) | peak support |
|---:|---:|---:|---:|---:|---:|
| 10 | 0.008197 | 0.004330 | 1.89x | 6.73 | 8,838 |
| 11 | 0.043204 | 0.020335 | 2.12x | 11.36 | 39,307 |
| 12 | 0.245592 | 0.112681 | 2.18x | 26.80 | 188,100 |
| 13 | 1.741573 | 0.823089 | 2.12x | 138.24 | 978,362 |
| 14 | 11.089654 | 5.853217 | 1.89x | 666.16 | 5,479,934 |

N=14 从逐格点 matched C steps 464,957,208 降为 286,010,088 row candidates，其中
23,859,616 合法。E6 证明局域 automaton 开销很大，但 support/materialization 仍主导。
原始数据和自包含报告位于 `experiments/e6_compiled_row_operator/`。
镜像原始 CSV：`benchmarks/e6_compiled_row_operator_release.csv`。

### 13.3 E7：exact weighted ADD/ZDD

E7 在 E6 完整 boundary coefficient map 上构造 exact weighted ADD/ZDD，terminal 保存
`u128`，分别比较 grouped/interleaved 变量顺序。

| N | explicit peak support | best diagram | best nodes | nodes/support | profile total (s) | E6 contraction (s) |
|---:|---:|:---|---:|---:|---:|---:|
| 10 | 8,838 | interleaved ZDD | 13,103 | 1.48 | 0.0664 | 0.00433 |
| 11 | 39,307 | interleaved ZDD | 48,302 | 1.23 | 0.3073 | 0.02033 |

节点数、时间和 RSS 均未过 gate，故在 N=11 停止。失败原因不是 coefficient 种类过多，
而是固定变量顺序下 boundary bit-pattern 的子函数共享不足。实验保存在
`codex/exp-boundary-diagram` 分支。
主分支镜像原始 CSV：`benchmarks/e7_boundary_diagram_release.csv`。

### 13.4 E8：direct-TN ordering oracle

E8 每格直接从 17-entry `C` 构造 factor，按通用 sparse join/project 比较三种 site ordering；
使用 `BigUint`，严格施加 \(v_0,v_1,v_2\)。

| N | ordering | count | peak support | frontier vars | candidate pairs | matched pairs |
|---:|:---|---:|---:|---:|---:|---:|
| 5 | row-major | 10 | 23 | 14 | 3,096 | 289 |
| 5 | snake | 10 | 23 | 14 | 3,096 | 289 |
| 5 | diagonal | 10 | 38 | 16 | 4,355 | 422 |
| 6 | row-major | 4 | 72 | 17 | 13,122 | 1,088 |
| 6 | snake | 4 | 72 | 17 | 13,122 | 1,088 |
| 6 | diagonal | 4 | 125 | 20 | 20,691 | 1,782 |

snake 与 row-major 完全相同，diagonal-wavefront 同时切穿更多四族 constraint lines，使
support 在 N=5/6 高 65.2%/73.6%。实验保存在 `codex/exp-ordering-oracle` 分支。
主分支镜像原始 CSV：`benchmarks/e8_ordering_oracle_release.csv`。

### 13.5 E9：exact sort-reduce materialization

E9 不改变 candidate 生成，只把层输出改为连续 vector，按完整 packed virtual-boundary key
排序并用 checked `u128` 原地 reduce。测试在 N=0–10 逐层核对 hash/sort 的 count、support、
completed terms、output weights 和 operator work。

| N | hash median (s) | sort median (s) | speedup | hash RSS (MiB) | sort RSS (MiB) | support |
|---:|---:|---:|---:|---:|---:|---:|
| 10 | 0.004926 | 0.003407 | 1.45x | 6.68 | 6.32 | 8,838 |
| 11 | 0.021051 | 0.016333 | 1.29x | 11.37 | 9.72 | 39,307 |
| 12 | 0.113696 | 0.081879 | 1.39x | 26.82 | 21.02 | 188,100 |
| 13 | 0.868104 | 0.457664 | 1.90x | 138.23 | 102.48 | 978,362 |
| 14 | 6.038000 | 3.085015 | 1.96x | 666.17 | 444.28 | 5,479,934 |

原计划因 merge ratio 低而降低了 sort-reduce 优先级，但实测推翻了该成本模型：主要收益来自
连续写入、紧凑 entry 和避免近乎唯一 key 的 HashMap probing，而不是 state merge。
原始数据和报告位于 `experiments/e9_sort_reduce/`。
镜像原始 CSV：`benchmarks/e9_sort_reduce_release.csv`。

### 13.6 E10：exact slicing 与 parallel sort

E10 把 unique parent boundary vector 切为约 `4*threads` slices；每个 worker 调用同一个
compiled-C row operator，合并 candidate 后 parallel sort，最后 checked-add reduce。
threads=1/2/4、N=0–10 的逐层等价测试通过。

| N | threads | median (s) | min (s) | speedup | RSS (MiB) | support |
|---:|---:|---:|---:|---:|---:|---:|
| 12 | 1 | 0.090255 | 0.088089 | 1.00x | 20.71 | 188,100 |
| 12 | 8 | 0.027684 | 0.026098 | 3.26x | 27.34 | 188,100 |
| 12 | 16 | 0.026349 | 0.024647 | 3.43x | 29.93 | 188,100 |
| 12 | 32 | 0.026530 | 0.025790 | 3.40x | 32.09 | 188,100 |
| 13 | 1 | 0.468509 | 0.463875 | 1.00x | 102.17 | 978,362 |
| 13 | 8 | 0.127353 | 0.125342 | 3.68x | 82.40 | 978,362 |
| 13 | 16 | 0.114947 | 0.111997 | 4.08x | 94.01 | 978,362 |
| 13 | 32 | 0.116107 | 0.113085 | 4.04x | 101.70 | 978,362 |
| 14 | 1 | 3.155874 | 3.009632 | 1.00x | 444.36 | 5,479,934 |
| 14 | 8 | 0.717977 | 0.677335 | 4.40x | 386.89 | 5,479,934 |
| 14 | 16 | 0.624911 | 0.597097 | 5.05x | 388.78 | 5,479,934 |
| 14 | 32 | 0.604584 | 0.574414 | 5.22x | 421.90 | 5,479,934 |

同 revision、顺序无竞争的 DFS N=14 comparator 为 16t 0.006306 s、32t 0.004885 s；
PEPS 分别慢 99.1x、123.8x。32t 相对 16t 仅再快 3.4%，表明 memory traffic、candidate
拼接及串行 reduce 已饱和。原始数据和报告位于 `experiments/e10_parallel_slicing/`。
镜像原始 CSV：`benchmarks/e10_parallel_slicing_release.csv` 和
`benchmarks/e10_dfs_comparator_release.csv`。

## 14. 前十方向强制研究复盘

### 14.1 总体进展

保持同一个 N=14 peak support 的情况下，性能演化为：

| 阶段 | threads | N=14 median (s) | RSS (MiB) | support | 相对初始 naive |
|:---|---:|---:|---:|---:|---:|
| initial explicit-C naive | 1 | 25.1915 | 986.34 | 5,479,934 | 1.00x |
| E1 indexed C | 1 | 19.3084 | 986.67 | 5,479,934 | 1.30x |
| E3 packed boundary | 1 | 17.6112 | 666.17 | 5,479,934 | 1.43x |
| E5a buffer reuse | 1 | 11.0897 | 666.17 | 5,479,934 | 2.27x |
| E6 compiled row operator | 1 | 5.8532 | 666.16 | 5,479,934 | 4.30x |
| E9 exact sort-reduce | 1 | 3.0850 | 444.28 | 5,479,934 | 8.17x |
| E10 parallel slicing | 16 | 0.6249 | 388.78 | 5,479,934 | 40.31x |
| E10 parallel slicing | 32 | 0.6046 | 421.90 | 5,479,934 | 41.67x |

串行实现累计快约 8.17x，并行 wall time 累计快约 41.7x；但没有一个方向减少 support。
因此过去十轮成功的是常数、布局和吞吐，不是渐近复杂度。

### 14.2 各类机制为何成功或失败

- **局域工作成功下降：** E1 和 E6 分别消除 17-entry 线性扫描及逐格 horizontal automaton，
  说明机械 partial evaluation 是合规且高收益的。
- **bytes/state 成功下降：** E3 的单个 `u128` key 和 E9 的连续 vector 减少容器元数据、
  probing 与 allocator 压力；N=14 RSS 从约 986 MiB 降至 444 MiB。
- **调度成功但已饱和：** E10 对大层获得 5.22x，但 16→32 threads 只有 3.4%，符合
  memory-bound/materialization-bound 特征。
- **容器微调失败：** E2 reserve 和 E4a hasher 不改变 entry 数或访问局部性，收益被扩容
  时机、hash quality/代码生成和 memory traffic 淹没。
- **普通 symbolic sharing 失败：** E7 的 fixed-order ADD/ZDD node 数高于 explicit support，
  表明当前 bit ordering 下几乎没有足够子函数共享。
- **朴素二维 ordering 失败：** E8 diagonal wavefront 增加四族 constraint lines 的同时
  cutwidth；只看几何“波前”而不计算活跃约束线是错误成本模型。
- **原 merge-ratio 模型被推翻：** E9 在 merge ratio 低时仍接近 2x，说明 sort-reduce 的
  主要价值是顺序内存和紧凑表示，而不是重复 key 的数量。

### 14.3 与原假设和目标的比较

第一轮复盘正确预测：局域算术、HashMap materialization 和 support 是三类不同瓶颈；E6/E9
确实依次解决前两类的相当部分。它低估了 sort-reduce 的 locality 收益，也高估了普通
decision diagram 和朴素 wavefront 的结构潜力。

最终目标仍未达到：

- best serial PEPS N=14 仍比 1-thread DFS 约慢 43.8x；
- best 16/32-thread PEPS 比同线程 DFS 慢 99.1x/123.8x；
- N=13→14 support 从 978,362 增至 5,479,934，约 5.60x；
- 当前没有可信路径外推至 \(Q(28)\)。即使常数再降一个数量级，support 的指数增长仍会先
  耗尽内存。

最大 verified PEPS 仍是 N=14；精确 bottleneck 是 5,479,934 个物化 boundary states、
286,010,088 row candidates，以及随 N 约 5–6x/step 增长的 support/work。

## 15. 十轮后的修订研究计划

本复盘推翻“继续做通用容器/线程微调即可追上 DFS”的计划。下一阶段的主假设改为：

> 必须在不破坏显式 Sec. VI \(C\) 和 exact boundary contraction 的前提下，找到可证明的
> future-equivalence quotient 或显著更低的四族 constraint-line cutwidth；否则不能靠
> 常数优化跨越当前 40–120x 的 DFS gap。

按优先级排列的后续方向：

1. **Geometry-aware cutwidth search oracle。** 在 E8 direct-TN oracle 上按四族活跃约束线数
   搜索/评估 ordering；必须同时降低 actual sparse support，而不只降低 dense proxy。
   kill：两个连续可测 N 未降低 support 25%。
2. **Future-signature quotient。** 从 compiled-C transfer 自动构造“对所有剩余行具有相同
   行为”的 exact 等价类，并在小 N 穷举证明；不是重复 fixed-order BDD。
   kill：N=10、11 canonical classes 不低于 explicit states 的 70%。
3. **分层/动态变量顺序 decision diagram。** 只有在 quotient 或 geometry 分析给出共享
   结构后才重访；kill：节点数或 apply cost 任一连续两档不优于 E9 explicit vector。
4. **CRT/finite-field backend。** 用于 coefficient 超过 `u128` 前的 exactness 和 SIMD/GPU
   准备，不把它描述为 support 优化；必须以多个 primes 重建并交叉验证小 N。
5. **外存/分布式 sort 与 checkpoint。** 仅在结构方向已降低增长率、资源投影允许更大 N
   时启动；否则只会把不可行的指数 materialization 推迟一两个 N。

在任何新方向开始前记录 hypothesis、branch/worktree、correctness gate、keep/kill 和资源
上限。下一组五方向完成前不得绕过下一次 review gate。当前按用户要求在 E10 与本复盘后
停止，不启动 E11。

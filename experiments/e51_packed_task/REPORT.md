# E51：packed direct-sector task tape

## 决策

**REJECT，不并入 production。** 8-byte packed task 将 task tape 缩为原
32-byte AoS 的 1/4，但 N=16 只快 1.9%，N=17 反而慢 0.3%；process
peak RSS 的变化不到 2%，未达到预注册的 5% wall 或 15% RSS gate。
N=16/17 已触发 early kill，故不运行 N=18。

| N | AoS median | packed median | wall change | AoS / packed task bytes | AoS / packed RSS |
|---:|---:|---:|---:|---:|---:|
| 16 | 0.316640 s | 0.310656 s | -1.9% | 2,268,992 / 567,248 | 13.06 / 12.88 MB |
| 17 | 2.767604 s | 2.776628 s | +0.3% | 3,661,888 / 915,472 | 17.49 / 17.68 MB |

两条路径的 count、split depth、task count、recursive nodes、accepted C
entries 完全相同。packed decode 每 task 只发生一次，但 task metadata
读取不是 dominant work；更小 tape 没有改变 5.71e8 / 4.28e9 个
C-derived recursive nodes。

## 正式 CPU profile

本实验在写代码后按用户要求补做正式 profiler，不把源码阅读当成测量：

- baseline revision：main `5dc5128`；
- tool：`samply 0.13.1`，1 kHz sampling，8 threads，N=17；
- build：release/thin-LTO/codegen-units=1，额外 `debuginfo=2` 只用于
  profiling；
- sampled RVA 用同一代码的 MSVC linker `/MAP` 离线映射，未上传 profile；
- benchmark binary 先跑一次 fast solve，随后 metrics call 又跑一次 fast
  solve和一次 generic replay，三段通过函数 RVA 分开统计。

production 的 38,578 个 EXE on-CPU leaf samples 中，38,570 个
（99.979%）位于
`contract_certified_tail_last_k_u64::<6>`；按 `threadCPUDelta` 权重为
99.985%。generic validation replay 的 47,883 samples 全部单独落在
`contract_recursive_tail`，没有混入 production 百分比。task build、
worker entry和 reduction 合计只有 8 production leaf samples。

这说明 E51 的失败并非 task packing 实现错误，而是原假设针对了错误的
working set：task tape 顺序读取只占不可见的小比例，真正热点在 last-6
子树内部。

硬件 cache counter 也作了诚实的可用性检查：

- WPR CPU profile 因 system-performance policy 返回 `0xc5585011`；
- `xperf -pmcsources` 能列出 `DcacheMisses`、`IcacheMisses`、
  `BranchMispredictions` 等事件，但 `-PmcProfile` 配置返回 `0x1069`；
- 因此本报告只声称 **函数热点已采样**，不声称已经测得 L1/L2/LLC miss
  rate。原始汇总和失败状态保存在
  `benchmarks/e51_baseline_samply_hotspots.csv`。

## 实现、exactness 与 PEPS fidelity

- code revision：`766bc2477bae46edee819de435a82bb57fc82d7d`；
- branch/worktree：`codex/exp-packed-task` /
  `.worktrees/e51-packed-task`；
- base：main `5dc5128`；
- `PackedWideTask` 是一个 `u64`：低 `N` bits 为 columns，随后两个
  `N`-bit fields 为两族 diagonal virtual signals，第 `3N` bit 编码
  vertical orbit weight 1/2；只允许 N<=20，所以 `3N+1<=61`；
- prefix 仍逐步调用从 explicit 17-entry C 编译的
  `RecursiveTailRelation`；packed format 只改变 boundary task 的物理
  布局；
- tail 仍调用 E47 的 certified last-6 contraction；column v1 和
  diagonal v2 终点、checked u64 和 certified CRT fallback 均不变。

测试静态要求 `size_of::<PackedWideTask>()=8`、
`size_of::<WideCrtTask>()=32`。对 N=1..20，每个实际生成的 prefix task
逐项验证 baseline task == unpack(pack(task))，并比较 split depth、
prefix nodes、accepted/kept entries。N=0..10 complete packed contraction
与 generic C replay/known count 一致；coefficient limit=1 强制从相同
packed sectors 解包并 CRT replay 得 Q(8)=92。完整 release suite 为
53 passed，`clippy --release --all-targets -- -D warnings` 通过。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；rustc 1.94.0 / LLVM 21.1.8；
- release、thin LTO、codegen-units=1、8 threads；
- N=16/17：1 warmup + 5 samples，另做一次 generic-C metrics replay；
- command：
  - `e51_packed_task aos 16 17 5 1 2048`
  - `e51_packed_task packed 16 17 5 1 2048`
- memory：Windows `PeakWorkingSet64` process high-water mark，包含 runtime、
  allocator、worker stacks 和独立 profile replay，不等于 live task heap；
- raw data：
  - `benchmarks/e51_aos_control.csv`
  - `benchmarks/e51_packed_candidate.csv`
  - `benchmarks/e51_baseline_samply_hotspots.csv`

下一方向不应再假设 prefix/task metadata 是主要瓶颈。cache 实验应直接
改变 certified last-6 hot function 的 local working set、代码 footprint
或 bounded exact suffix reuse。

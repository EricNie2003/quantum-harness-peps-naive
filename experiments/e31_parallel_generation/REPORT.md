# E31：负载均衡、内存受控的并行候选生成

## 决策

**KEEP。**

最终方案把 E28 每行的 exact sparse `C`-entry apply 并行化；在 8
线程、256 prefix shards 下：

| N | E28 control | E31 | speedup | E28 RSS | E31 RSS |
|---:|---:|---:|---:|---:|---:|
| 14 | 0.298 s | 0.179 s | 1.66x | 157.8 MB | 168.2 MB |
| 15 | 2.453 s | 1.287 s | 1.91x | 946.0 MB | 954.3 MB |

N=15 的线程局部候选峰值容量只有 7.28 MB。相对于第一版 Rayon
fold 的 675.8 MB，下降 98.9%，同时总时间从 1.684 s 降至
1.287 s。结果超过 15% time gate，RSS 只增加 0.9%，因此进入
production 候选。

## 实现与 fidelity

- code revision：`62ab8c5`；
- branch/worktree：`codex/exp-parallel-generation` /
  `.worktrees/e31-parallel-generation`；
- base：main `7ff79d5`（E28 compact production）；
- arithmetic：每个候选仍为 E28 `u64 key + u128 coefficient`，所有
  加法通过 checked `u128`；
- contraction：每个父 boundary 调用同一个从 explicit 17-entry
  `C` 编译的 row operator；D4 top-row orbit 权重、`v0/v1/v2` 和
  prefix sharding 均不变。并行只改变候选产生与汇合的调度。

按父状态数而不是 shard 数构造最多 8 个连续 source ranges，避免
prefix shards 的严重负载不均。每个 worker 使用 256 个小型
destination buffers；每处理 256 个父状态时，把达到 1024 entries
的 buffer 刷入对应的共享 bucket，行尾强制刷空。锁只位于批量
flush，不在每个 tensor transition 上。所有 worker 完成后，每个
destination shard 独立并行 sort/reduce。

35 个 release tests 与 Clippy 通过；新增测试在 N=0--10 验证 E31
与 E28 的 count、support 和 operator work 完全相同。Q(14)、
Q(15) 均通过 known-count 检查。

## 消融

| variant | N=15 time | RSS | worker partials | local capacity |
|:---|---:|---:|---:|---:|
| Rayon fold | 1.684 s | 1.324 GB | 211 | 675.8 MB |
| fixed source chunks | 2.681 s | 1.373 GB | 8 | 705.7 MB |
| balanced bounded flush | 1.287 s | 0.954 GB | 8 | 7.28 MB |

Rayon fold 动态均衡但产生 211 份完整 destination-bucket arrays；
固定按 shard 数切 8 块减少 partial 数，却因 prefix support
不均造成尾部 worker。最终版本同时按 actual parent counts 均衡，
并通过 bounded flush 消除整层候选的线程局部复制。这说明收益来自
生成阶段并行化和正确的内存组织，而不是 support/work 改变：
N=15 support 仍为 18,178,233，operator matched 仍为 80,077,350。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：`cargo run --release`（项目 release profile/thin-LTO）；
- threads：`RAYON_NUM_THREADS=8`；
- commands：
  - `cargo run --release --bin e31_parallel_generation -- 256 14 15 3`
  - `cargo run --release --bin e28_compact -- 256 14 15 3`
- repetition：同一进程顺序运行 3 次，报告中位数与最小值；control
  紧接 E31 运行。Windows 背景负载造成 control N=15 离散较大，
  因而后续 checkpoint 还应重新交错复测。
- memory：进程 `PeakWorkingSet64` 高水位；包含 allocator、线程栈及
  同进程先前 repeat 的未归还页，不能等同算法瞬时 live bytes。
  `peak_thread_local_bytes` 是 Rust Vec capacity 的显式和，不含
  allocator metadata 与共享 bucket。

Raw data：`benchmarks/e31_parallel_generation_release.csv`。

# E45：wide virtual boundary + exact finite-field/CRT backend

## 决策

**KEEP 为 optional exact/low-memory backend；不替换 E42 production fast
path。**

E45 达成了原方向的结构 gate：

- 不再受 E42 joint-u64 的 N<=21 key/weight packing 限制；
- N=22--27 自动选择 3 个经确定性检验的 primes，N=28 选择 4 个；
- CRT modulus product 严格大于 `N!`，所以重构的非负 count 唯一；
- 完成并验证到 Q(19)=4,968,057,848；
- N=22--28 的 actual C-derived wide prefixes 已构造，但明确标记为
  `prefix_only_not_QN`，没有冒充完成 count。

它没有达到 throughput 目标。N=17/18：

| N | E45 wide CRT | E42 last-4 | DFS | E45 RSS | E42 RSS |
|---:|---:|---:|---:|---:|---:|
| 17 | 4.336 s | 3.435 s | 3.624 s | 17.6 MB | 76.4 MB |
| 18 | 31.297 s | 24.565 s | 21.876 s | 7.3 MB | 600.7 MB |

E45 比 E42 慢约 26--27%，但 N=18 RSS 低约 83x。其价值是 exact width
和内存可扩展性，不是当前速度。

## 代码、PEPS fidelity 与 exactness

- code revision：`f7a6f36`；
- branch/worktree：`codex/exp-wide-crt` / `.worktrees/e45-wide-crt`；
- base：main `b198821`（accepted E42 + rejected E43 artifacts）；
- symmetry：top-row vertical reflection orbit；
- state：三个独立 u64 masks 保存 column、DR、DL virtual boundary，
  task weight 独立保存，不再把 key/coefficient 联合塞进 u64。

后端首先从 explicit rank-8 C 的 17 个非零元编译
`RecursiveTailRelation`；`CertifiedSecViTailPlan` 只在唯一 occupied
entry 的四通道都是 0→1、row v0→v1、value=1 时通过。wide prefix 每一
层都枚举这个 relation 的 matching C entries；tail 在一次 traversal 中
维护 1--4 条 modular residue lanes，最后四行仍是同一 certified C
transition 的展开。column endpoints 与 v1 收缩，两个 diagonal families
与 v2 收缩。没有调用 DFS module。

primes 为：

`4294967291, 4294967279, 4294967231, 4294967197`。

程序启动时用 trial division 到平方根逐个确定性验证 primality。对给定
N，选择最短 prefix，使 prime product `M > N!`；因为任何 N-Queens
solution 都是一个 column permutation，`Q(N) <= N! < M`。CRT 使用
checked u128 增量重构，并再次检查结果不超过 `N!`。N=19 首次实际出现
不同 residues `673090557|673090569`，重构为 4,968,057,848，证明路径
不是简单地把小整数复制到各 lane。

测试包括：

- N=0..12 modular result 与 generic checked-u128 C replay/known counts；
- N=22 使用 3 primes、N=28 使用 4 primes，且 `M>N!`；
- 以 Q(27) 合成四 residues 后 CRT 恢复原值；
- N=22/28 actual wide prefix seed；
- 既有 explicit B/C 17-entry、boundary vectors、reachable-parent
  compiled-C/sitewise-C 和 forced-overflow tests。

## Granularity 消融

| N | 512 tasks/thread | 2048 tasks/thread | effect |
|---:|---:|---:|---:|
| 16 | 0.4660 s / cut4 / 9,844 tasks | 0.4397 s / cut5 / 70,906 | -5.6% |
| 17 | 4.3527 s / cut4 / 14,272 tasks | 4.3362 s / cut5 / 114,434 | -0.4% |

较深 prefix 只改善 N=16；N=17 已由双 residue arithmetic 主导。正式
N=18/19 使用 target=16,384 total tasks；实际 cut4 分别产生 18,132 /
25,080 tasks，RSS 保持约 6--7 MB。

## Scaling、最大 N 与 Q(28) 投影

| N | exact E45 time | recursive nodes | ratio |
|---:|---:|---:|---:|
| 16 | 0.440 s | 0.571B | — |
| 17 | 4.336 s | 4.276B | 9.86x wall / 7.49x nodes |
| 18 | 31.297 s | 29.683B | 7.22x wall / 6.94x nodes |
| 19 | 243.230 s | not profiled | 7.77x wall |

N=19 为单个 uninstrumented exact sample；为避免额外约数分钟的 generic
profile replay，CSV 明确记录 `metrics_collected=false`，local accepted
只含 prefix，不能当作完整 work count。

按最近 7.77x wall ratio：

- N=20：约 31.5 min；
- N=21：约 6.1 h（`21!` 开始需要 3 lanes，另计约 1.5x）；
- N=22：约 47 h；
- N=23：约 15 d；
- N=24：约 119 d；
- N=25：约 2.5 y；
- N=26：约 20 y；
- N=27：约 153 y；
- N=28：约 1,600 y（4 lanes 再计约 4/3）。

这是数量级投影，不是承诺值；它忽略更大 N 的 cache/频率变化，但足以
否定当前 row-recursive kernel 直接求 Q(28) 的资源可行性。N=28 prefix
只需 7,850 tasks、约 0.057 ms；真正瓶颈完全在指数 tail，不在 wide key、
CRT 或 prefix memory。最大完成并验证的是 N=19，不是 N=28。

## Benchmark protocol

- CPU：AMD Ryzen 9 7945HX，16 cores / 32 logical processors；
- compiler：rustc 1.94.0，x86_64-pc-windows-msvc，LLVM 21.1.8；
- build：release、thin LTO、codegen-units=1；8 threads；
- N=14--17 exploration：1 warmup + 5 samples（target ablation 为 3）；
- N=18：1 warmup + 3 samples + 独立 generic u128 profile；
- N=19：1 uninstrumented exact sample、无 warmup、无 profile；
- commands：
  - `cargo run --release --bin e45_wide_crt -- plan 22 28`
  - `cargo run --release --bin e45_wide_crt -- bench 14 17 5 1 512 1`
  - `cargo run --release --bin e45_wide_crt -- bench 16 17 3 1 2048 1`
  - `cargo run --release --bin e45_wide_crt -- bench 18 18 3 1 2048 1`
  - `cargo run --release --bin e45_wide_crt -- bench 19 19 1 0 2048 0`
- memory：Windows `PeakWorkingSet64` 进程高水位，包含 allocator、线程栈、
  runtime 和（若启用）profile replay，不等于 live heap。

Raw data：

- `benchmarks/e45_wide_crt_plan_n22_n28.csv`
- `benchmarks/e45_wide_crt_release.csv`
- `benchmarks/e45_target_ablation.csv`
- `benchmarks/e45_e42_dfs_controls.csv`

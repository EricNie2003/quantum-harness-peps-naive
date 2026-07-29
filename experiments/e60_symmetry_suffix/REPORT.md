# E60：symmetry-canonical suffix reuse certification

## 决策

**online optimization REJECT；exact diagnostic KEEP。** 在分配 cache 前，
先证明 vertical boundary canonicalization 是 exact tail automorphism，
再统计 production task boundary 的真实复用率。没有两档达到 10% keep
gate，所以没有实现或 benchmark 在线 canonical cache。

| N | tasks | exact dup | extra vertical dup | total canonical dup |
|---:|---:|---:|---:|---:|
| 14 | 27,034 | 0.392% | 0.041% | 0.433% |
| 15 | 47,460 | 0.299% | 5.808% | 6.089% |
| 16 | 70,906 | 0.227% | 0.020% | 0.247% |
| 17 | 114,434 | 0.180% | 5.226% | 5.397% |

偶数 N 的 vertical orbit 已由 first-row top-queen slicing 几乎完全消掉；
奇数 N 的 center-column fixed orbit仍留下约 5% reflected pairs。这个收益
只覆盖奇数 N，且 canonical bit-reverse、hash/sort与 merge 仍有成本，低于
预注册的两档 10% reuse gate。结合 E54 普通 suffix cache <0.28% hit，
没有证据支持再把 canonical lookup 放入 99.98% 热核。

## exactness 与 D4 结论

- base revision：main `641d906`；
- benchmarked code state：base 加 patch SHA-256
  `e86967b516fa2edb8481ed239fdffbfaca353b7abfce931dc287674a90f19013`；
- branch/worktree：`codex/exp-symmetry-suffix` /
  `.worktrees/e60-symmetry-suffix`；
- vertical transform 为
  `(columns, dr, dl) -> (reverse(columns), reverse(dl), reverse(dr))`；
- N=1..8 的每个 reachable parent逐项验证：
  reflected legal positions等于 bit-reverse positions，且每个
  explicit-C successor 与“先 successor 再 reflect”完全相同；
- interior row cut 的 stabilizer仍只有 identity/vertical，其他六个 D4
  actions 不保持 row cut，不能合法混入这个 canonical key；
- N=0..12 audit invariants、B/C 17-entry 和既有 boundary tests通过；
- full release suite 53 passed；format/clippy通过。

## Measurement protocol

- AMD Ryzen 9 7945HX，rustc 1.94.0 / LLVM 21.1.8；
- release/thin-LTO/codegen-units=1，8 threads；
- N=14--17，production `target_tasks_per_thread=2048`；
- exact/canonical keys 都是完整 packed three-mask boundary，没有 hash-only
  equivalence；HashSet只用于离线计数；
- command：
  `cargo run --release --bin e60_symmetry_suffix -- 14 17 2048`；
- RSS：Windows `PeakWorkingSet64` process high-water；
- raw CSV：`benchmarks/e60_vertical_suffix_audit.csv`。

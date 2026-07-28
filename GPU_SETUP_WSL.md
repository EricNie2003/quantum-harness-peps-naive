# RTX 4060 CUDA setup under Ubuntu/WSL2

The E11 backend is optional. Ordinary CPU builds do not require CUDA. The GPU
feature targets the RTX 4060 (compute capability 8.9) and H200 (9.0) in one
binary.

## 1. Prepare Windows and WSL

1. Update WSL from an Administrator PowerShell:

   ```powershell
   wsl --update
   ```

2. Install a current NVIDIA Windows driver that supports CUDA in WSL, reboot,
   and verify from Ubuntu:

   ```bash
   nvidia-smi
   ```

The Windows driver supplies the WSL GPU driver. Do **not** install a Linux
NVIDIA display driver inside WSL. Follow NVIDIA's WSL guide and install a
toolkit-only package such as `cuda-toolkit-12-x`, not `cuda`, `cuda-12-x`, or
`cuda-drivers`:

<https://docs.nvidia.com/cuda/wsl-user-guide/index.html>

CUDA Toolkit 12.4 or newer is the supported build baseline. After following
the WSL-Ubuntu instructions from NVIDIA's CUDA download page, verify:

```bash
nvcc --version
nvidia-smi
```

If CUDA is installed outside `/usr/local/cuda`, export its root before Cargo:

```bash
export CUDA_HOME=/path/to/cuda
```

## 2. Fetch and build the experiment branch

```bash
source "$HOME/.cargo/env"
git fetch origin
git switch codex/exp-gpu-sort-reduce

cargo test --release
cargo build --release --features cuda
cargo run --release --features cuda --bin gpu_sort_reduce -- probe --device 0
cargo run --release --features cuda --bin gpu_sort_reduce -- self-test --device 0
```

The CUDA build fails clearly if `nvcc`, the CUDA headers, `libcudart`, or a
supported device is missing. `compact64` is rejected above N=20; `wide128` is
selected automatically for N=21--42.

## 3. Correctness smoke test

```bash
cargo test --release --features cuda 2>&1 | tee /tmp/nqueens_gpu_tests.log
cargo run --release --features cuda --bin gpu_sort_reduce -- \
  solve 14 --device 0 --scheme compact64 --layers \
  2>&1 | tee /tmp/nqueens_gpu_n14.log
```

The solve must end with `Q(N)=365596` and `verified=true`. Do not benchmark an
unverified build.

## 4. Collect the RTX 4060 benchmark

Run with the laptop connected to power and without competing GPU workloads:

```bash
./scripts/benchmark_gpu_wsl.sh
```

Optional environment variables are `GPU_DEVICE`, `MIN_N`, `MAX_N`, `WARMUP`,
`REPEATS`, and `OUTPUT_ROOT`. The default protocol runs N=12--14, two warmups,
and nine measured repetitions for both exact GPU schemes and the CPU/DFS
comparators.

Return these artifacts for review:

- `benchmarks/e11_gpu_sort_reduce_rtx4060.csv`
- `benchmarks/e11_gpu_sort_reduce_rtx4060_raw/environment.txt`
- `benchmarks/e11_gpu_sort_reduce_rtx4060_raw/terminal.log`
- the CPU and DFS CSV files in the same raw directory

Peak host RSS is read from Linux `/proc/self/status` (`VmHWM`). It is the
process-lifetime high-water mark, so in a multi-repetition benchmark it is
cumulative rather than isolated to one sample. Peak device memory is the
maximum CUDA payload allocated by this backend. It does not include
driver/context allocations or memory owned by other processes. The 85% guard
is based on total device memory and fails before a known backend allocation
would cross that limit.

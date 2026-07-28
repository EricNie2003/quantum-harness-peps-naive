#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

device="${GPU_DEVICE:-0}"
min_n="${MIN_N:-12}"
max_n="${MAX_N:-14}"
warmup="${WARMUP:-2}"
repeats="${REPEATS:-9}"
output_root="${OUTPUT_ROOT:-benchmarks/e11_gpu_sort_reduce_rtx4060_raw}"
mkdir -p "$output_root"
log="$output_root/terminal.log"
exec > >(tee "$log") 2>&1

logical_threads="$(nproc)"
physical_threads="$(lscpu -p=CORE,SOCKET | sed '/^#/d' | sort -u | wc -l)"
if [[ "$physical_threads" -lt 1 ]]; then
    physical_threads="$logical_threads"
fi

echo "E11 GPU benchmark: environment capture"
{
    date --iso-8601=seconds
    git rev-parse HEAD
    git status --short --branch
    uname -a
    rustc --version --verbose
    cargo --version
    nvcc --version
    nvidia-smi --query-gpu=index,name,driver_version,memory.total,compute_cap,power.limit --format=csv
    lscpu
    printf 'logical_threads=%s\nphysical_threads=%s\n' "$logical_threads" "$physical_threads"
    printf 'device=%s\nmin_n=%s\nmax_n=%s\nwarmup=%s\nrepeats=%s\n' \
        "$device" "$min_n" "$max_n" "$warmup" "$repeats"
} > "$output_root/environment.txt"

echo "E11 GPU benchmark: release tests and CUDA device self-test"
cargo test --release
cargo test --release --features cuda
cargo run --release --features cuda --bin gpu_sort_reduce -- probe --device "$device"
cargo run --release --features cuda --bin gpu_sort_reduce -- self-test --device "$device"

echo "E11 GPU benchmark: same-laptop CPU PEPS baselines"
cargo run --release --bin parallel_slicing -- 1 "$min_n" "$max_n" 1 > /dev/null
cargo run --release --bin parallel_slicing -- 1 "$min_n" "$max_n" "$repeats" \
    > "$output_root/cpu_1t.csv"
cargo run --release --bin parallel_slicing -- "$physical_threads" "$min_n" "$max_n" 1 \
    > /dev/null
cargo run --release --bin parallel_slicing -- \
    "$physical_threads" "$min_n" "$max_n" "$repeats" \
    > "$output_root/cpu_physical.csv"
if [[ "$logical_threads" != "$physical_threads" ]]; then
    cargo run --release --bin parallel_slicing -- \
        "$logical_threads" "$min_n" "$max_n" 1 > /dev/null
    cargo run --release --bin parallel_slicing -- \
        "$logical_threads" "$min_n" "$max_n" "$repeats" \
        > "$output_root/cpu_logical.csv"
fi

echo "E11 GPU benchmark: compact64"
cargo run --release --features cuda --bin gpu_sort_reduce -- \
    bench "$max_n" --min "$min_n" --device "$device" --scheme compact64 \
    --warmup "$warmup" --repeats "$repeats" --csv \
    > "$output_root/gpu_compact64.csv"

echo "E11 GPU benchmark: wide128"
cargo run --release --features cuda --bin gpu_sort_reduce -- \
    bench "$max_n" --min "$min_n" --device "$device" --scheme wide128 \
    --warmup "$warmup" --repeats "$repeats" --csv \
    > "$output_root/gpu_wide128.csv"

{
    head -n 1 "$output_root/gpu_compact64.csv"
    tail -n +2 "$output_root/gpu_compact64.csv"
    tail -n +2 "$output_root/gpu_wide128.csv"
} > benchmarks/e11_gpu_sort_reduce_rtx4060.csv

echo "E11 GPU benchmark: DFS comparator"
cargo run --release --bin dfs_bitmask -- \
    bench "$max_n" --min "$max_n" --threads "$physical_threads" \
    --warmup "$warmup" --repeats "$repeats" --csv \
    > "$output_root/dfs_physical.csv"

echo "E11 GPU benchmark complete"
echo "combined_gpu_csv=benchmarks/e11_gpu_sort_reduce_rtx4060.csv"
echo "raw_results=$output_root"
echo "terminal_log=$log"

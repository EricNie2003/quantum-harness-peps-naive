#!/bin/bash
#SBATCH --job-name=qpeps-cmp-dfs
#SBATCH --partition=wzacnormal03
#SBATCH --account=giggleliu
#SBATCH --qos=user_tangdingwen
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=128
#SBATCH --threads-per-core=1
#SBATCH --mem=8G
#SBATCH --time=06:00:00
#SBATCH --output=logs/%x-%j.out
#SBATCH --error=logs/%x-%j.err

set -euo pipefail

if [[ $# -ne 4 ]]; then
    echo "usage: sbatch scnet_compare_dfs.sbatch MIN_N MAX_N REPEATS WARMUP" >&2
    exit 2
fi

min_n=$1
max_n=$2
repeats=$3
warmup=$4
stage_dir=${SLURM_SUBMIT_DIR:?SLURM_SUBMIT_DIR is not set}
result_stem="comparison_dfs_scnet_n${min_n}-${max_n}_job${SLURM_JOB_ID}"
csv_path="$stage_dir/results/${result_stem}.csv"
meta_path="$stage_dir/results/${result_stem}.meta.txt"
point_dir="$stage_dir/results/${result_stem}_points"

cd "$stage_dir"
mkdir -p results "$point_dir"

finish_metadata() {
    exit_status=$?
    {
        echo "finished_at=$(date --iso-8601=seconds)"
        echo "exit_status=$exit_status"
    } >> "$meta_path"
}
trap finish_metadata EXIT

{
    echo "status=started"
    echo "started_at=$(date --iso-8601=seconds)"
    echo "source_revision=b89f4f1320bdbe9e0fcce700b9d98b879f012bea"
    echo "algorithm_class=conventional_dfs_bitmask_comparator"
    echo "build_command=cargo build --release --target x86_64-unknown-linux-musl --bin dfs_bitmask"
    echo "build_compiler=rustc 1.97.1 (8bab26f4f 2026-07-14); LLVM 22.1.6"
    echo "build_target=x86_64-unknown-linux-musl"
    echo "command=dfs_bitmask bench N --min N --threads $SLURM_CPUS_PER_TASK --repeats $repeats --warmup $warmup --csv"
    echo "slurm_job_id=$SLURM_JOB_ID"
    echo "slurm_node_list=$SLURM_JOB_NODELIST"
    echo "slurm_cpus_per_task=$SLURM_CPUS_PER_TASK"
    echo "slurm_memory_per_node_mb=${SLURM_MEM_PER_NODE:-NA}"
    echo "repetition_policy=fresh process per N; requested warmups, requested uninstrumented samples, then one instrumented profile replay"
    echo "memory_method=per-N GNU time -v Maximum resident set size and in-process VmHWM"
    file ./bin/dfs_bitmask
    uname -a
    lscpu
} > "$meta_path"

sha256sum bin/dfs_bitmask > "$stage_dir/results/${result_stem}.sha256"
export RUST_BACKTRACE=1
export RAYON_NUM_THREADS=$SLURM_CPUS_PER_TASK

first_point=1
for n in $(seq "$min_n" "$max_n"); do
    point_csv="$point_dir/n${n}.csv"
    point_time="$point_dir/n${n}.time.txt"
    srun --nodes=1 --ntasks=1 --cpus-per-task="$SLURM_CPUS_PER_TASK" --cpu-bind=cores \
        /usr/bin/time -v -o "$point_time" \
        ./bin/dfs_bitmask bench "$n" --min "$n" --threads "$SLURM_CPUS_PER_TASK" \
        --repeats "$repeats" --warmup "$warmup" --csv > "$point_csv"
    if [[ $first_point -eq 1 ]]; then
        cp "$point_csv" "$csv_path"
        first_point=0
    else
        tail -n +2 "$point_csv" >> "$csv_path"
    fi
done

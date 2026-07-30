#!/bin/bash
#SBATCH --job-name=qpeps-cmp-naive
#SBATCH --partition=wzacnormal03
#SBATCH --account=giggleliu
#SBATCH --qos=user_tangdingwen
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=1
#SBATCH --threads-per-core=1
#SBATCH --mem=8G
#SBATCH --time=12:00:00
#SBATCH --output=logs/%x-%j.out
#SBATCH --error=logs/%x-%j.err

set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: sbatch scnet_compare_naive20b.sbatch MIN_N MAX_N REPEATS" >&2
    exit 2
fi

min_n=$1
max_n=$2
repeats=$3
stage_dir=${SLURM_SUBMIT_DIR:?SLURM_SUBMIT_DIR is not set}
result_stem="comparison_naive20b_scnet_n${min_n}-${max_n}_job${SLURM_JOB_ID}"
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
    echo "source_revision=20b5334f55819ab0b4bdce7aa701527de736c3dc"
    echo "source_subject=Index C entries by incoming virtual signature"
    echo "algorithm_class=naive_exact_explicit_C_hashmap_row_PEPS"
    echo "build_command=cargo build --release --target x86_64-unknown-linux-musl --bin nqueens-peps-naive"
    echo "build_compiler=rustc 1.97.1 (8bab26f4f 2026-07-14); LLVM 22.1.6"
    echo "build_target=x86_64-unknown-linux-musl"
    echo "command=nqueens-peps-naive bench N --min N --repeats $repeats --csv"
    echo "slurm_job_id=$SLURM_JOB_ID"
    echo "slurm_node_list=$SLURM_JOB_NODELIST"
    echo "slurm_cpus_per_task=$SLURM_CPUS_PER_TASK"
    echo "slurm_memory_per_node_mb=${SLURM_MEM_PER_NODE:-NA}"
    echo "repetition_policy=fresh process per N; requested samples; historical CLI has no separate warmup option"
    echo "memory_method=per-N GNU time -v Maximum resident set size; historical Linux in-process RSS field is zero and ignored"
    file ./bin/nqueens-peps-naive
    uname -a
    lscpu
} > "$meta_path"

sha256sum bin/nqueens-peps-naive > "$stage_dir/results/${result_stem}.sha256"
export RUST_BACKTRACE=1

first_point=1
for n in $(seq "$min_n" "$max_n"); do
    point_csv="$point_dir/n${n}.csv"
    point_time="$point_dir/n${n}.time.txt"
    srun --nodes=1 --ntasks=1 --cpus-per-task=1 --cpu-bind=cores \
        /usr/bin/time -v -o "$point_time" \
        ./bin/nqueens-peps-naive bench "$n" --min "$n" --repeats "$repeats" --csv \
        > "$point_csv"
    if [[ $first_point -eq 1 ]]; then
        cp "$point_csv" "$csv_path"
        first_point=0
    else
        tail -n +2 "$point_csv" >> "$csv_path"
    fi
done

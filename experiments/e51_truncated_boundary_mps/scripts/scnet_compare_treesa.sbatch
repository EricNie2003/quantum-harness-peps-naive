#!/bin/bash
#SBATCH --job-name=qpeps-cmp-treesa
#SBATCH --partition=wzacnormal03
#SBATCH --account=giggleliu
#SBATCH --qos=user_tangdingwen
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=1
#SBATCH --threads-per-core=1
#SBATCH --mem=16G
#SBATCH --time=04:00:00
#SBATCH --output=logs/%x-%j.out
#SBATCH --error=logs/%x-%j.err

set -euo pipefail

if [[ $# -ne 5 ]]; then
    echo "usage: sbatch scnet_compare_treesa.sbatch MIN_N MAX_N SEED NTRIALS NITERS" >&2
    exit 2
fi

min_n=$1
max_n=$2
seed=$3
ntrials=$4
niters=$5
stage_dir=${SLURM_SUBMIT_DIR:?SLURM_SUBMIT_DIR is not set}
result_stem="comparison_treesa_scnet_n${min_n}-${max_n}_job${SLURM_JOB_ID}"
csv_path="$stage_dir/results/${result_stem}.csv"
meta_path="$stage_dir/results/${result_stem}.meta.txt"
point_dir="$stage_dir/results/${result_stem}_points"
plan_dir="$stage_dir/results/${result_stem}_plans"

cd "$stage_dir"
mkdir -p results "$point_dir" "$plan_dir" depot

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
    echo "source_revision=c715e36e835a1890055c09fad34ca7c2e854bf0d"
    echo "implementation_revision=e9a80a5"
    echo "algorithm_class=exact_explicit_C_D4_site_tree_PEPS_with_TreeSA"
    echo "executor_build_command=cargo build --release --target x86_64-unknown-linux-musl --bin e37_treesa_d4"
    echo "executor_build_compiler=rustc 1.97.1 (8bab26f4f 2026-07-14); LLVM 22.1.6"
    echo "executor_build_target=x86_64-unknown-linux-musl"
    echo "planner=OMEinsumContractionOrders.TreeSA space-only"
    echo "planner_command=generate_plan.jl PLAN N $seed $ntrials $niters"
    echo "executor_command=e37_treesa_d4 PLAN d4"
    echo "slurm_job_id=$SLURM_JOB_ID"
    echo "slurm_node_list=$SLURM_JOB_NODELIST"
    echo "slurm_cpus_per_task=$SLURM_CPUS_PER_TASK"
    echo "slurm_memory_per_node_mb=${SLURM_MEM_PER_NODE:-NA}"
    echo "repetition_policy=one deterministic-seed TreeSA plan and one fresh-process exact executor per N"
    echo "memory_method=separate per-N GNU time -v maxima for Julia planner and Rust executor; end-to-end peak is their maximum"
    julia --version
    file ./bin/e37_treesa_d4
    uname -a
    lscpu
} > "$meta_path"

sha256sum bin/e37_treesa_d4 experiments/e37_treesa_d4/generate_plan.jl \
    experiments/e37_treesa_d4/Manifest.toml > "$stage_dir/results/${result_stem}.sha256"

export JULIA_NUM_THREADS=1
export JULIA_DEPOT_PATH="$stage_dir/depot:"
export JULIA_PKG_PRECOMPILE_AUTO=0
export RUST_BACKTRACE=1

first_point=1
for n in $(seq "$min_n" "$max_n"); do
    plan="$plan_dir/treesa_n${n}_seed${seed}.plan"
    plan_time="$point_dir/n${n}.plan.time.txt"
    plan_stdout="$point_dir/n${n}.plan.stdout.txt"
    executor_csv="$point_dir/n${n}.executor.csv"
    executor_time="$point_dir/n${n}.executor.time.txt"

    srun --nodes=1 --ntasks=1 --cpus-per-task=1 --cpu-bind=cores \
        /usr/bin/time -v -o "$plan_time" \
        julia --startup-file=no --project=experiments/e37_treesa_d4 \
        experiments/e37_treesa_d4/generate_plan.jl \
        "$plan" "$n" "$seed" "$ntrials" "$niters" > "$plan_stdout"

    srun --nodes=1 --ntasks=1 --cpus-per-task=1 --cpu-bind=cores \
        /usr/bin/time -v -o "$executor_time" \
        ./bin/e37_treesa_d4 "$plan" d4 > "$executor_csv"

    if [[ $first_point -eq 1 ]]; then
        cp "$executor_csv" "$csv_path"
        first_point=0
    else
        tail -n +2 "$executor_csv" >> "$csv_path"
    fi
done

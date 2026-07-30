#!/bin/bash
#SBATCH --job-name=qpeps-e51-mps
#SBATCH --partition=wzacnormal03
#SBATCH --account=giggleliu
#SBATCH --qos=user_tangdingwen
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=128
#SBATCH --threads-per-core=1
#SBATCH --mem=32G
#SBATCH --time=12:00:00
#SBATCH --output=logs/%x-%j.out
#SBATCH --error=logs/%x-%j.err

set -euo pipefail

if [[ $# -ne 5 ]]; then
    echo "usage: sbatch scnet_e51_truncation.sbatch MIN_N MAX_N CHI_CSV REPEATS WARMUP" >&2
    exit 2
fi

min_n=$1
max_n=$2
chi_csv=$3
repeats=$4
warmup=$5
stage_dir=${SLURM_SUBMIT_DIR:?SLURM_SUBMIT_DIR is not set}
source_revision=${SOURCE_REVISION:-unknown}
run_tests=${E51_RUN_TESTS:-1}
result_stem="e51_truncated_mps_scnet_n${min_n}-${max_n}_job${SLURM_JOB_ID}"
csv_path="$stage_dir/results/${result_stem}.csv"
meta_path="$stage_dir/results/${result_stem}.meta.txt"
point_dir="$stage_dir/results/${result_stem}_points"

cd "$stage_dir"
mkdir -p results "$point_dir" depot

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
    echo "source_revision=$source_revision"
    echo "source_branch=codex/exp-truncated-boundary-mps"
    echo "algorithm_class=approximate_float64_boundary_mps_svd"
    echo "classification=diagnostic_only_not_issue34_exact"
    echo "command=sbatch scnet_e51_truncation.sbatch $min_n $max_n $chi_csv $repeats $warmup"
    echo "slurm_job_id=$SLURM_JOB_ID"
    echo "slurm_job_name=$SLURM_JOB_NAME"
    echo "slurm_partition=$SLURM_JOB_PARTITION"
    echo "slurm_node_list=$SLURM_JOB_NODELIST"
    echo "slurm_cpus_per_task=$SLURM_CPUS_PER_TASK"
    echo "slurm_threads_per_core=1"
    echo "slurm_memory_request=32G"
    echo "julia_num_threads=1"
    echo "openblas_num_threads=$SLURM_CPUS_PER_TASK"
    echo "run_tests=$run_tests"
    echo "repetition_policy=one warm process per N/chi; requested warmups followed by requested samples; median internal kernel wall"
    echo "memory_method=per-point GNU time -v Maximum resident set size plus process VmHWM; includes Julia runtime and LAPACK workspaces"
    julia --version
    uname -a
    lscpu
} > "$meta_path"

sha256sum Project.toml src/TruncatedBoundaryMPS.jl scripts/benchmark_point.jl > "$stage_dir/results/${result_stem}.sha256"

export JULIA_NUM_THREADS=1
export OPENBLAS_NUM_THREADS=$SLURM_CPUS_PER_TASK
export JULIA_DEPOT_PATH="$stage_dir/depot:"
export JULIA_PKG_PRECOMPILE_AUTO=0

if [[ "$run_tests" == 1 ]]; then
    srun --nodes=1 --ntasks=1 --cpus-per-task="$SLURM_CPUS_PER_TASK" --cpu-bind=cores \
        /usr/bin/time -v -o "$stage_dir/results/${result_stem}.tests.time.txt" \
        julia --startup-file=no --project=. test/runtests.jl \
        > "$stage_dir/results/${result_stem}.tests.txt"
fi

IFS=',' read -r -a chi_values <<< "$chi_csv"
first_point=1
for n in $(seq "$min_n" "$max_n"); do
    for chi in "${chi_values[@]}"; do
        point_stem="n${n}_chi${chi}"
        point_csv="$point_dir/${point_stem}.csv"
        point_time="$point_dir/${point_stem}.time.txt"
        srun --nodes=1 --ntasks=1 --cpus-per-task="$SLURM_CPUS_PER_TASK" --cpu-bind=cores \
            /usr/bin/time -v -o "$point_time" \
            julia --startup-file=no --project=. scripts/benchmark_point.jl \
            "$n" "$chi" "$repeats" "$warmup" 0 > "$point_csv"
        if [[ $first_point -eq 1 ]]; then
            cp "$point_csv" "$csv_path"
            first_point=0
        else
            tail -n +2 "$point_csv" >> "$csv_path"
        fi
    done
done

use nqueens_peps_naive::{contract_rows_wide_scalar_batched_avx2_with_target, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn percentile(sorted: &[Duration], numerator: usize, denominator: usize) -> Duration {
    sorted[(sorted.len() - 1) * numerator / denominator]
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let repeats = parse(args.next(), "REPEATS")?;
    let warmup = parse(args.next(), "WARMUP")?;
    let target_tasks_per_thread = parse(args.next(), "TARGET_TASKS_PER_THREAD")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err(
            "usage: e49_batched_avx2 MIN_N MAX_N REPEATS WARMUP TARGET_TASKS_PER_THREAD".to_owned(),
        );
    }
    println!(
        "N,count,verified,median_elapsed_s,min_elapsed_s,p10_elapsed_s,p90_elapsed_s,\
         peak_rss_bytes,threads,split_depth,tail_tasks,vector_full_batches,\
         vector_partial_batches,vector_root_rounds,vector_root_active_lanes,\
         vector_root_slot_occupancy,recursive_nodes,recursive_accepted_entries,\
         total_accepted_entries,repeats,warmup,algorithm_class,peak_sparse_support,\
         local_tensor_entries_examined,local_tensor_entries_accepted"
    );
    for n in min_n..=max_n {
        for _ in 0..warmup {
            contract_rows_wide_scalar_batched_avx2_with_target(n, false, target_tasks_per_thread)?;
        }
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_wide_scalar_batched_avx2_with_target(
                n,
                false,
                target_tasks_per_thread,
            )?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
        }
        elapsed.sort_unstable();
        let profile =
            contract_rows_wide_scalar_batched_avx2_with_target(n, true, target_tasks_per_thread)?;
        peak_rss = peak_rss.max(profile.contraction.peak_rss_bytes);
        let verified = known_count(n) == Some(profile.contraction.count);
        if known_count(n).is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        let vector_slots = profile.vector_root_rounds.saturating_mul(4);
        let occupancy = if vector_slots == 0 {
            0.0
        } else {
            profile.vector_root_active_lanes as f64 / vector_slots as f64
        };
        println!(
            "{n},{},{verified},{:.9},{:.9},{:.9},{:.9},{peak_rss},{},{},{},{},{},\
             {},{},{occupancy:.6},{},{},{},{repeats},{warmup},\
             certified_explicit_C_wide_scalar_batched_AVX2_PEPS,{},{},{}",
            profile.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.split_depth,
            profile.tail_tasks,
            profile.vector_full_batches,
            profile.vector_partial_batches,
            profile.vector_root_rounds,
            profile.vector_root_active_lanes,
            profile.recursive_nodes,
            profile.recursive_accepted_entries,
            profile.contraction.row_operator_matched,
            profile.contraction.peak_states,
            profile.contraction.tensor_entries_examined,
            profile.contraction.row_operator_matched,
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

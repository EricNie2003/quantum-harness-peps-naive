use nqueens_peps_naive::{
    contract_rows_wide_scalar_last_k_with_target, e55_hot_code_shape, known_count,
};
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
    let profile_replay = parse(args.next(), "PROFILE_REPLAY")? != 0;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e55_icache_shape MIN_N MAX_N REPEATS WARMUP PROFILE_REPLAY".to_owned());
    }
    let executable_bytes = env::current_exe()
        .map_err(|error| format!("cannot resolve current executable: {error}"))?
        .metadata()
        .map_err(|error| format!("cannot stat current executable: {error}"))?
        .len();
    println!(
        "N,shape,count,verified,median_elapsed_s,min_elapsed_s,p10_elapsed_s,p90_elapsed_s,\
         peak_rss_bytes,threads,split_depth,tail_tasks,executable_bytes,recursive_nodes,\
         recursive_accepted_entries,total_accepted_entries,seed_s,tail_s,profile_replay_s,\
         repeats,warmup,profile_metrics,algorithm_class,peak_sparse_support,\
         local_tensor_entries_examined,local_tensor_entries_accepted"
    );
    for n in min_n..=max_n {
        for _ in 0..warmup {
            contract_rows_wide_scalar_last_k_with_target(n, false, 2048, 6)?;
        }
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_wide_scalar_last_k_with_target(n, false, 2048, 6)?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
        }
        elapsed.sort_unstable();
        let profile = contract_rows_wide_scalar_last_k_with_target(n, profile_replay, 2048, 6)?;
        peak_rss = peak_rss.max(profile.contraction.peak_rss_bytes);
        let verified = known_count(n) == Some(profile.contraction.count);
        if known_count(n).is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        println!(
            "{n},{},{},{verified},{:.9},{:.9},{:.9},{:.9},{peak_rss},{},{},{},\
             {executable_bytes},{},{},{},{:.9},{:.9},{:.9},{repeats},{warmup},\
             {profile_replay},certified_explicit_C_wide_scalar_last6_icache_shape_PEPS,\
             {},{},{}",
            e55_hot_code_shape(),
            profile.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.split_depth,
            profile.tail_tasks,
            profile.recursive_nodes,
            profile.recursive_accepted_entries,
            profile.contraction.row_operator_matched,
            profile.seed_elapsed.as_secs_f64(),
            profile.tail_elapsed.as_secs_f64(),
            profile.profile_replay_elapsed.as_secs_f64(),
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

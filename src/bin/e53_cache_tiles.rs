use nqueens_peps_naive::{
    CacheTileSchedule, contract_rows_wide_scalar_cache_tiled,
    contract_rows_wide_scalar_last_k_with_target, known_count,
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

fn pipe<T: std::fmt::Display>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("|")
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or_else(|| "missing MODE".to_owned())?;
    let cache_lines = parse(args.next(), "CACHE_LINES")?;
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let repeats = parse(args.next(), "REPEATS")?;
    let warmup = parse(args.next(), "WARMUP")?;
    let profile_replay = parse(args.next(), "PROFILE_REPLAY")? != 0;
    let schedule = match mode.as_str() {
        "control" => None,
        "dynamic" => Some(CacheTileSchedule::Dynamic),
        "contiguous" => Some(CacheTileSchedule::Contiguous),
        "cyclic" => Some(CacheTileSchedule::BlockCyclic),
        _ => {
            return Err("MODE must be control, dynamic, contiguous, or cyclic".to_owned());
        }
    };
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err(
            "usage: e53_cache_tiles MODE CACHE_LINES MIN_N MAX_N REPEATS WARMUP \
             PROFILE_REPLAY"
                .to_owned(),
        );
    }
    println!(
        "N,mode,cache_lines,tasks_per_tile,count,verified,median_elapsed_s,min_elapsed_s,\
         p10_elapsed_s,p90_elapsed_s,peak_rss_bytes,threads,split_depth,tail_tasks,\
         worker_task_counts,task_imbalance,worker_recursive_nodes,node_imbalance,\
         recursive_nodes,recursive_accepted_entries,total_accepted_entries,seed_s,tail_s,\
         profile_replay_s,repeats,warmup,profile_metrics,algorithm_class,\
         peak_sparse_support,local_tensor_entries_examined,local_tensor_entries_accepted"
    );
    for n in min_n..=max_n {
        let solve = |profile| {
            if let Some(schedule) = schedule {
                contract_rows_wide_scalar_cache_tiled(n, profile, 2048, schedule, cache_lines).map(
                    |result| {
                        (
                            result.wide,
                            result.tasks_per_tile,
                            result.worker_task_counts,
                            result.worker_recursive_nodes,
                        )
                    },
                )
            } else {
                contract_rows_wide_scalar_last_k_with_target(n, profile, 2048, 6)
                    .map(|wide| (wide, 16, Vec::new(), Vec::new()))
            }
        };
        for _ in 0..warmup {
            solve(false)?;
        }
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = solve(false)?.0;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
        }
        elapsed.sort_unstable();
        let (profile, tasks_per_tile, worker_tasks, worker_nodes) = solve(profile_replay)?;
        peak_rss = peak_rss.max(profile.contraction.peak_rss_bytes);
        let verified = known_count(n) == Some(profile.contraction.count);
        if known_count(n).is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        let task_imbalance = worker_tasks
            .iter()
            .max()
            .zip(worker_tasks.iter().min())
            .map_or(0, |(max, min)| max - min);
        let node_imbalance = worker_nodes
            .iter()
            .max()
            .zip(worker_nodes.iter().min())
            .map_or(0, |(max, min)| max - min);
        println!(
            "{n},{mode},{cache_lines},{tasks_per_tile},{},{verified},{:.9},{:.9},{:.9},\
             {:.9},{peak_rss},{},{},{},\"{}\",{task_imbalance},\"{}\",{node_imbalance},\
             {},{},{},{:.9},{:.9},{:.9},{repeats},{warmup},{profile_replay},\
             certified_explicit_C_wide_scalar_last6_cache_tiled_PEPS,{},{},{}",
            profile.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.split_depth,
            profile.tail_tasks,
            pipe(&worker_tasks),
            pipe(&worker_nodes),
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

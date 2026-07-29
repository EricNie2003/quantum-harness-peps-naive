use nqueens_peps_naive::{
    SuffixCacheMetrics, contract_rows_wide_scalar_last_k_with_target,
    contract_rows_wide_scalar_suffix_cached, known_count,
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
    let mode = args.next().ok_or_else(|| "missing MODE".to_owned())?;
    let cache_kib = parse(args.next(), "CACHE_KIB_PER_WORKER")?;
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let repeats = parse(args.next(), "REPEATS")?;
    let warmup = parse(args.next(), "WARMUP")?;
    let profile_replay = parse(args.next(), "PROFILE_REPLAY")? != 0;
    if args.next().is_some()
        || !matches!(mode.as_str(), "control" | "cache")
        || min_n > max_n
        || repeats == 0
    {
        return Err(
            "usage: e54_suffix_cache MODE[control|cache] CACHE_KIB_PER_WORKER \
             MIN_N MAX_N REPEATS WARMUP PROFILE_REPLAY"
                .to_owned(),
        );
    }
    println!(
        "N,mode,cache_kib_per_worker,count,verified,median_elapsed_s,min_elapsed_s,\
         p10_elapsed_s,p90_elapsed_s,peak_rss_bytes,threads,split_depth,tail_tasks,\
         cache_slots_per_worker,cache_workers,cache_bytes,cache_max_remaining,\
         cache_lookups,cache_hits,cache_hit_rate,cache_inserts,cache_replacements,\
         recursive_nodes,recursive_accepted_entries,total_accepted_entries,seed_s,tail_s,\
         profile_replay_s,repeats,warmup,profile_metrics,algorithm_class,\
         peak_sparse_support,local_tensor_entries_examined,local_tensor_entries_accepted"
    );
    for n in min_n..=max_n {
        let solve = |profile| {
            if mode == "cache" {
                contract_rows_wide_scalar_suffix_cached(n, profile, 2048, cache_kib).map(|result| {
                    (
                        result.wide,
                        result.cache_slots_per_worker,
                        result.cache_workers,
                        result.cache_bytes,
                        result.cache_max_remaining_rows,
                        result.cache_metrics,
                    )
                })
            } else {
                contract_rows_wide_scalar_last_k_with_target(n, profile, 2048, 6)
                    .map(|wide| (wide, 0, 0, 0, 0, SuffixCacheMetrics::default()))
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
        let (profile, slots, workers, bytes, max_remaining, metrics) = solve(profile_replay)?;
        peak_rss = peak_rss.max(profile.contraction.peak_rss_bytes);
        let verified = known_count(n) == Some(profile.contraction.count);
        if known_count(n).is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        let hit_rate = if metrics.lookups == 0 {
            0.0
        } else {
            metrics.hits as f64 / metrics.lookups as f64
        };
        println!(
            "{n},{mode},{cache_kib},{},{verified},{:.9},{:.9},{:.9},{:.9},{peak_rss},\
             {},{},{},{slots},{workers},{bytes},{max_remaining},{},{},{hit_rate:.9},{},{},\
             {},{},{},{:.9},{:.9},{:.9},{repeats},{warmup},{profile_replay},\
             certified_explicit_C_wide_scalar_last6_tagged_suffix_cache_PEPS,{},{},{}",
            profile.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.split_depth,
            profile.tail_tasks,
            metrics.lookups,
            metrics.hits,
            metrics.inserts,
            metrics.replacements,
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

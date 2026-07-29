use nqueens_peps_naive::{
    contract_rows_wide_crt_with_target, known_count, probe_wide_crt_prefix, wide_crt_plan,
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
    let index = (sorted.len() - 1) * numerator / denominator;
    sorted[index]
}

fn parse_bool01(value: Option<String>, label: &str) -> Result<bool, String> {
    match value.as_deref() {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err(format!("{label} must be 0 or 1")),
    }
}

fn plan_mode(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e45_wide_crt plan MIN_N MAX_N".to_owned());
    }
    println!(
        "N,factorial_bound,prime_count,primes,modulus_product,split_depth,\
         target_tail_tasks,tail_tasks,prefix_nodes,prefix_accepted_entries,\
         prefix_kept_entries,seed_s,peak_rss_bytes,status"
    );
    for n in min_n..=max_n {
        let plan = wide_crt_plan(n)?;
        let prefix = probe_wide_crt_prefix(n)?;
        let primes = plan
            .primes
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("|");
        println!(
            "{n},{},{},{},{},{},{},{},{},{},{},{:.9},{},prefix_only_not_QN",
            plan.factorial_bound,
            plan.primes.len(),
            primes,
            plan.modulus_product,
            prefix.split_depth,
            prefix.target_tail_tasks,
            prefix.tail_tasks,
            prefix.prefix_nodes,
            prefix.prefix_accepted_entries,
            prefix.prefix_kept_entries,
            prefix.seed_elapsed.as_secs_f64(),
            prefix.peak_rss_bytes,
        );
    }
    Ok(())
}

fn bench_mode(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let repeats = parse(args.next(), "REPEATS")?;
    let warmup = parse(args.next(), "WARMUP")?;
    let target_tasks_per_thread = parse(args.next(), "TARGET_TASKS_PER_THREAD")?;
    let profile_replay = parse_bool01(args.next(), "PROFILE_REPLAY")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e45_wide_crt bench MIN_N MAX_N REPEATS WARMUP \
             TARGET_TASKS_PER_THREAD PROFILE_REPLAY"
            .to_owned());
    }
    println!(
        "N,count,verified,median_elapsed_s,min_elapsed_s,p10_elapsed_s,p90_elapsed_s,\
         peak_rss_bytes,threads,split_depth,target_tail_tasks,tail_tasks,prime_count,\
         primes,modulus_product,factorial_bound,residues,prefix_nodes,\
         prefix_accepted_entries,prefix_kept_entries,recursive_nodes,\
         recursive_accepted_entries,total_accepted_entries,seed_s,tail_s,\
         profile_replay_s,repeats,warmup,algorithm_class,peak_sparse_support,\
         local_tensor_entries_examined,local_tensor_entries_accepted,metrics_collected"
    );
    for n in min_n..=max_n {
        for _ in 0..warmup {
            contract_rows_wide_crt_with_target(n, false, target_tasks_per_thread)?;
        }
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        let mut last_result = None;
        for _ in 0..repeats {
            let result = contract_rows_wide_crt_with_target(n, false, target_tasks_per_thread)?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
            last_result = Some(result);
        }
        elapsed.sort_unstable();
        let profile = if profile_replay {
            contract_rows_wide_crt_with_target(n, true, target_tasks_per_thread)?
        } else {
            last_result.expect("positive repeat count")
        };
        peak_rss = peak_rss.max(profile.contraction.peak_rss_bytes);
        let verified = known_count(n) == Some(profile.contraction.count);
        if known_count(n).is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        let primes = profile
            .plan
            .primes
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("|");
        let residues = profile
            .residues
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("|");
        println!(
            "{n},{},{verified},{:.9},{:.9},{:.9},{:.9},{peak_rss},{},{},{},{},{},{},\
             {},{},{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{repeats},{warmup},\
             certified_explicit_C_wide_CRT_PEPS,{},{},{},{}",
            profile.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.split_depth,
            profile.target_tail_tasks,
            profile.tail_tasks,
            profile.plan.primes.len(),
            primes,
            profile.plan.modulus_product,
            profile.plan.factorial_bound,
            residues,
            profile.prefix_nodes,
            profile.prefix_accepted_entries,
            profile.prefix_kept_entries,
            profile.recursive_nodes,
            profile.recursive_accepted_entries,
            profile.contraction.row_operator_matched,
            profile.seed_elapsed.as_secs_f64(),
            profile.tail_elapsed.as_secs_f64(),
            profile.profile_replay_elapsed.as_secs_f64(),
            profile.contraction.peak_states,
            profile.contraction.tensor_entries_examined,
            profile.contraction.row_operator_matched,
            profile_replay,
        );
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("plan") => plan_mode(args),
        Some("bench") => bench_mode(args),
        _ => Err("usage: e45_wide_crt <plan|bench> ...".to_owned()),
    }
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

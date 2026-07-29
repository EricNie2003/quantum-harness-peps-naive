use nqueens_peps_naive::{contract_rows_wide_crt_last_six_forced_with_target, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn parse_bool01(value: Option<String>, label: &str) -> Result<bool, String> {
    match value.as_deref() {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err(format!("{label} must be 0 or 1")),
    }
}

fn percentile(sorted: &[Duration], numerator: usize, denominator: usize) -> Duration {
    sorted[(sorted.len() - 1) * numerator / denominator]
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or_else(|| "missing MODE".to_owned())?;
    let avx2 = match mode.as_str() {
        "scalar" => false,
        "avx2" => true,
        _ => return Err("MODE must be scalar or avx2".to_owned()),
    };
    let lanes = parse(args.next(), "LANES")?;
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let repeats = parse(args.next(), "REPEATS")?;
    let warmup = parse(args.next(), "WARMUP")?;
    let target_tasks_per_thread = parse(args.next(), "TARGET_TASKS_PER_THREAD")?;
    let profile_replay = parse_bool01(args.next(), "PROFILE_REPLAY")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e50_crt_avx2 MODE LANES MIN_N MAX_N REPEATS WARMUP \
             TARGET_TASKS_PER_THREAD PROFILE_REPLAY"
            .to_owned());
    }
    println!(
        "N,mode,lanes,count,verified,median_elapsed_s,min_elapsed_s,p10_elapsed_s,\
         p90_elapsed_s,peak_rss_bytes,threads,split_depth,tail_tasks,primes,residues,\
         modulus_product,factorial_bound,recursive_nodes,recursive_accepted_entries,\
         total_accepted_entries,repeats,warmup,algorithm_class,peak_sparse_support,\
         local_tensor_entries_examined,local_tensor_entries_accepted,metrics_collected"
    );
    for n in min_n..=max_n {
        for _ in 0..warmup {
            contract_rows_wide_crt_last_six_forced_with_target(
                n,
                false,
                target_tasks_per_thread,
                lanes,
                avx2,
            )?;
        }
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        let mut last_result = None;
        for _ in 0..repeats {
            let result = contract_rows_wide_crt_last_six_forced_with_target(
                n,
                false,
                target_tasks_per_thread,
                lanes,
                avx2,
            )?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
            last_result = Some(result);
        }
        elapsed.sort_unstable();
        let profile = if profile_replay {
            contract_rows_wide_crt_last_six_forced_with_target(
                n,
                true,
                target_tasks_per_thread,
                lanes,
                avx2,
            )?
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
            "{n},{mode},{lanes},{},{verified},{:.9},{:.9},{:.9},{:.9},{peak_rss},\
             {},{},{},{},{},{},{},{},{},{},{repeats},{warmup},\
             certified_explicit_C_forced_CRT_last6_PEPS,{},{},{},{}",
            profile.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.split_depth,
            profile.tail_tasks,
            primes,
            residues,
            profile.plan.modulus_product,
            profile.plan.factorial_bound,
            profile.recursive_nodes,
            profile.recursive_accepted_entries,
            profile.contraction.row_operator_matched,
            profile.contraction.peak_states,
            profile.contraction.tensor_entries_examined,
            profile.contraction.row_operator_matched,
            profile_replay,
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

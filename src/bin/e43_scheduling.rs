use nqueens_peps_naive::{
    TailTaskBackend, TailTaskOrdering, contract_rows_adaptive_scheduled_tail, known_count,
};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn parse_ordering(value: Option<String>) -> Result<TailTaskOrdering, String> {
    match value.as_deref() {
        Some("natural") => Ok(TailTaskOrdering::Natural),
        Some("hard-first") => Ok(TailTaskOrdering::HardFirst),
        Some("probe3-hard-first") => Ok(TailTaskOrdering::Probe3HardFirst),
        _ => Err("ORDERING must be natural, hard-first, or probe3-hard-first".to_owned()),
    }
}

fn parse_backend(value: Option<String>) -> Result<TailTaskBackend, String> {
    match value.as_deref() {
        Some("atomic") => Ok(TailTaskBackend::NativeAtomic),
        Some("rayon") => Ok(TailTaskBackend::Rayon),
        _ => Err("BACKEND must be atomic or rayon".to_owned()),
    }
}

fn percentile(sorted: &[Duration], numerator: usize, denominator: usize) -> Duration {
    let index = (sorted.len() - 1) * numerator / denominator;
    sorted[index]
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let ordering = parse_ordering(args.next())?;
    let backend = parse_backend(args.next())?;
    let chunk = parse_usize(args.next(), "CHUNK")?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    let warmup = parse_usize(args.next(), "WARMUP")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err(
            "usage: e43_scheduling ORDERING BACKEND CHUNK MIN_N MAX_N REPEATS WARMUP".to_owned(),
        );
    }
    println!(
        "N,count,verified,median_elapsed_s,min_elapsed_s,p10_elapsed_s,p90_elapsed_s,\
         peak_rss_bytes,threads,selected_cut,prefix_support,recursive_nodes,\
         recursive_accepted_entries,total_accepted_entries,ordering,backend,chunk,\
         ordering_s,selection_s,prefix_s,tail_s,profile_replay_s,repeats,warmup,\
         algorithm_class,peak_sparse_support,local_tensor_entries_examined,\
         local_tensor_entries_accepted"
    );
    for n in min_n..=max_n {
        for _ in 0..warmup {
            contract_rows_adaptive_scheduled_tail(n, 256, false, 4, ordering, backend, chunk)?;
        }
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result =
                contract_rows_adaptive_scheduled_tail(n, 256, false, 4, ordering, backend, chunk)?;
            peak_rss = peak_rss.max(result.fast.contraction.peak_rss_bytes);
            elapsed.push(result.fast.contraction.elapsed);
        }
        elapsed.sort_unstable();
        let profile =
            contract_rows_adaptive_scheduled_tail(n, 256, true, 4, ordering, backend, chunk)?;
        peak_rss = peak_rss.max(profile.fast.contraction.peak_rss_bytes);
        let verified = known_count(n) == Some(profile.fast.contraction.count);
        if known_count(n).is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        println!(
            "{n},{},{verified},{:.9},{:.9},{:.9},{:.9},{peak_rss},{},{},{},{},{},{},\
             {:?},{:?},{},{:.9},{:.9},{:.9},{:.9},{:.9},{repeats},{warmup},\
             certified_explicit_C_scheduled_PEPS,{},{},{}",
            profile.fast.contraction.count,
            percentile(&elapsed, 1, 2).as_secs_f64(),
            elapsed[0].as_secs_f64(),
            percentile(&elapsed, 1, 10).as_secs_f64(),
            percentile(&elapsed, 9, 10).as_secs_f64(),
            rayon::current_num_threads(),
            profile.selected_cut,
            profile.fast.prefix_support,
            profile.fast.recursive_nodes,
            profile.fast.recursive_accepted_entries,
            profile.fast.contraction.row_operator_matched,
            ordering,
            backend,
            chunk,
            profile.fast.task_ordering_elapsed.as_secs_f64(),
            profile.selection_elapsed.as_secs_f64(),
            profile.fast.prefix_elapsed.as_secs_f64(),
            profile.fast.tail_elapsed.as_secs_f64(),
            profile.fast.profile_replay_elapsed.as_secs_f64(),
            profile.fast.contraction.peak_states,
            profile.fast.contraction.tensor_entries_examined,
            profile.fast.contraction.row_operator_matched,
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

use nqueens_peps_naive::{RecursiveD4Mode, contract_rows_recursive_d4, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn parse_mode(value: Option<String>) -> Result<RecursiveD4Mode, String> {
    match value.as_deref() {
        Some("none") => Ok(RecursiveD4Mode::None),
        Some("vertical") => Ok(RecursiveD4Mode::Vertical),
        Some("full") => Ok(RecursiveD4Mode::Full),
        _ => Err("MODE must be none, vertical, or full".to_owned()),
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let split_depth = parse(args.next(), "SPLIT_DEPTH")?;
    let mode = parse_mode(args.next())?;
    let repeats = parse(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e39_recursive_d4 MIN_N MAX_N SPLIT_DEPTH MODE REPEATS".to_owned());
    }
    println!(
        "mode,N,split_depth,count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,\
         tail_tasks,recursive_nodes,recursive_accepted_entries,canonical_checks,\
         partial_prunes,complete_representatives,orbit_size_1,orbit_size_2,\
         orbit_size_4,orbit_size_8,task_generation_s,tail_s,repeats"
    );
    for n in min_n..=max_n {
        let actual_split = split_depth.min(n);
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut last = None;
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_recursive_d4(n, actual_split, mode)?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
            last = Some(result);
        }
        elapsed.sort_unstable();
        let result = last.expect("positive repeat count");
        println!(
            "{mode:?},{n},{actual_split},{},{},{:.9},{:.9},{},{},{},{},{},{},{},{},{},{},{},\
             {:.9},{:.9},{}",
            result.contraction.count,
            known_count(n) == Some(result.contraction.count),
            elapsed[elapsed.len() / 2].as_secs_f64(),
            elapsed[0].as_secs_f64(),
            peak_rss,
            result.tail_tasks,
            result.recursive_nodes,
            result.recursive_accepted_entries,
            result.canonical_checks,
            result.partial_prunes,
            result.complete_representatives,
            result.orbit_size_1,
            result.orbit_size_2,
            result.orbit_size_4,
            result.orbit_size_8,
            result.task_generation_elapsed.as_secs_f64(),
            result.tail_elapsed.as_secs_f64(),
            repeats,
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

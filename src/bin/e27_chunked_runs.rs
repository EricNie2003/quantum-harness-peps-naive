use nqueens_peps_naive::{contract_rows_d4_chunked_runs, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let shards = parse_usize(args.next(), "SHARDS")?;
    let chunk = parse_usize(args.next(), "PARENT_CHUNK")?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e27_chunked_runs SHARDS PARENT_CHUNK MIN_N MAX_N REPEATS".to_owned());
    }
    println!(
        "shards,parent_chunk,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,\
         peak_rss_bytes,peak_support,peak_live_candidates,peak_run_entries,max_runs_per_shard,\
         merge_heap_operations,row_operator_candidates,row_operator_matched,repeats"
    );
    for n in min_n..=max_n {
        let mut durations = Vec::with_capacity(repeats);
        let mut last = None;
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_d4_chunked_runs(n, shards, chunk)?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            durations.push(result.contraction.elapsed);
            last = Some(result);
        }
        durations.sort_unstable();
        let minimum = durations[0];
        let median: Duration = durations[durations.len() / 2];
        let result = last.expect("positive repeat count");
        let contraction = result.contraction;
        let expected = known_count(n);
        println!(
            "{shards},{chunk},{n},{},{},{},{:.9},{:.9},{},{},{},{},{},{},{},{},{}",
            contraction.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            expected == Some(contraction.count),
            median.as_secs_f64(),
            minimum.as_secs_f64(),
            peak_rss,
            contraction.peak_states,
            result.peak_live_candidates,
            result.peak_run_entries,
            result.max_runs_per_shard,
            result.merge_heap_operations,
            contraction.row_operator_candidates,
            contraction.row_operator_matched,
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

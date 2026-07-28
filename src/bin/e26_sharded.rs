use nqueens_peps_naive::{
    ContractionResult, ShardMode, contract_rows_d4_sharded_sparse_sort_reduce, known_count,
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

fn median(values: &mut [Duration]) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mode = match args.next().as_deref() {
        Some("prefix") => ShardMode::Prefix,
        Some("mixed") => ShardMode::Mixed,
        _ => return Err("usage: e26_sharded MODE SHARDS MIN_N MAX_N REPEATS".to_owned()),
    };
    let shards = parse_usize(args.next(), "SHARDS")?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e26_sharded MODE SHARDS MIN_N MAX_N REPEATS".to_owned());
    }

    println!(
        "mode,shards,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,\
         peak_rss_bytes,peak_support,tensor_entries_examined,tensor_entries_matched,\
         row_operator_candidates,row_operator_matched,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::with_capacity(repeats);
        let mut final_result: Option<ContractionResult> = None;
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_d4_sharded_sparse_sort_reduce(n, shards, mode)?;
            peak_rss = peak_rss.max(result.peak_rss_bytes);
            elapsed.push(result.elapsed);
            final_result = Some(result);
        }
        let result = final_result.expect("positive repeat count");
        let minimum = *elapsed.iter().min().expect("positive repeat count");
        let median = median(&mut elapsed);
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        println!(
            "{mode:?},{shards},{n},{},{},{verified},{:.9},{:.9},{},{},{},{},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            median.as_secs_f64(),
            minimum.as_secs_f64(),
            peak_rss,
            result.peak_states,
            result.tensor_entries_examined,
            result.tensor_entries_matched,
            result.row_operator_candidates,
            result.row_operator_matched,
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

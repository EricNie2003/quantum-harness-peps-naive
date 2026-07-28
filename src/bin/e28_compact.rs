use nqueens_peps_naive::{contract_rows_d4_compact_sharded_sort_reduce, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let shards = parse(args.next(), "SHARDS")?;
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let repeats = parse(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e28_compact SHARDS MIN_N MAX_N REPEATS".to_owned());
    }
    println!(
        "shards,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,\
         peak_support,row_operator_candidates,row_operator_matched,entry_bytes,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut last = None;
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_d4_compact_sharded_sort_reduce(n, shards)?;
            elapsed.push(result.elapsed);
            peak_rss = peak_rss.max(result.peak_rss_bytes);
            last = Some(result);
        }
        elapsed.sort_unstable();
        let result = last.expect("positive repeat count");
        let expected = known_count(n);
        println!(
            "{shards},{n},{},{},{},{:.9},{:.9},{},{},{},{},24,{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            expected == Some(result.count),
            elapsed[elapsed.len() / 2].as_secs_f64(),
            elapsed[0].as_secs_f64(),
            peak_rss,
            result.peak_states,
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

use nqueens_peps_naive::path_search::contract_bidirectional_separator;
use nqueens_peps_naive::{known_count, peak_rss_bytes};
use std::env;
use std::process::ExitCode;
use std::time::Instant;

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e20_separator_join MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_top_support,peak_bottom_support,peak_live_support,aggregate_left_join_keys,aggregate_right_join_keys,join_matching_pairs,peak_intermediate_support,peak_rank,tensor_entries_examined,tensor_entries_accepted,all_matching_pairs,d4_sectors"
    );
    for n in min_n..=max_n {
        let start = Instant::now();
        let result = contract_bidirectional_separator(n)?;
        let elapsed = start.elapsed();
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        println!(
            "{n},{},{},{verified},{:.9},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            elapsed.as_secs_f64(),
            peak_rss_bytes(),
            result.peak_top_support,
            result.peak_bottom_support,
            result.peak_live_support,
            result.aggregate_left_join_keys,
            result.aggregate_right_join_keys,
            result.join_matching_pairs,
            result.path.peak_support,
            result.path.peak_rank,
            result.path.local_tensor_entries_examined,
            result.path.local_tensor_entries_accepted,
            result.path.matching_entry_pairs,
            result.d4_sectors,
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

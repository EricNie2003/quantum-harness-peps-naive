use nqueens_peps_naive::{contract_rows_d4_two_row_macro, known_count};
use std::env;
use std::process::ExitCode;

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
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e29_two_row_macro SHARDS MIN_N MAX_N".to_owned());
    }
    println!(
        "shards,N,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_macro_support,\
         peak_macro_candidates,first_row_transitions,second_row_transitions,\
         total_operator_matched"
    );
    for n in min_n..=max_n {
        let result = contract_rows_d4_two_row_macro(n, shards)?;
        let contraction = result.contraction;
        let expected = known_count(n);
        println!(
            "{shards},{n},{},{},{},{:.9},{},{},{},{},{},{}",
            contraction.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            expected == Some(contraction.count),
            contraction.elapsed.as_secs_f64(),
            contraction.peak_rss_bytes,
            contraction.peak_states,
            result.peak_macro_candidates,
            result.first_row_transitions,
            result.second_row_transitions,
            contraction.row_operator_matched,
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

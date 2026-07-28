use nqueens_peps_naive::path_search::{PathKind, contract_with_d4_macro_path};
use nqueens_peps_naive::{known_count, peak_rss_bytes};
use std::env;
use std::process::ExitCode;
use std::time::Instant;

fn parse_path(value: &str) -> Result<PathKind, String> {
    match value {
        "row" => Ok(PathKind::RowBlocks),
        "half-row" => Ok(PathKind::HalfRowBlocks),
        _ => Err("PATH must be row or half-row".to_owned()),
    }
}

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = parse_path(
        &args
            .next()
            .ok_or_else(|| "usage: e19_d4_macro_tree PATH MIN_N MAX_N".to_owned())?,
    )?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e19_d4_macro_tree PATH MIN_N MAX_N".to_owned());
    }
    println!(
        "path,N,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_support,peak_rank,tensor_entries_examined,tensor_entries_accepted,cartesian_pair_upper_bound,matching_entry_pairs,contractions,d4_sectors"
    );
    for n in min_n..=max_n {
        let start = Instant::now();
        let metrics = contract_with_d4_macro_path(n, path)?;
        let elapsed = start.elapsed();
        let expected = known_count(n);
        let verified = expected == Some(metrics.count);
        println!(
            "{path:?},{n},{},{},{verified},{:.9},{},{},{},{},{},{},{},{},{}",
            metrics.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            elapsed.as_secs_f64(),
            peak_rss_bytes(),
            metrics.peak_support,
            metrics.peak_rank,
            metrics.local_tensor_entries_examined,
            metrics.local_tensor_entries_accepted,
            metrics.cartesian_pair_upper_bound,
            metrics.matching_entry_pairs,
            metrics.contractions,
            n.div_ceil(2),
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

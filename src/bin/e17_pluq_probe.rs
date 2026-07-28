use nqueens_peps_naive::rank_diagnostic::diagnose_peak_layer_rank;
use std::env;
use std::process::ExitCode;

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
        return Err("usage: e17_pluq_probe MIN_N MAX_N".to_owned());
    }
    println!(
        "N,selected_row,support,rank,left_factor_nnz,right_factor_nnz,reconstruction_products,reconstruction_support_ratio,elapsed_s,peak_rss_bytes,verified_two_primes"
    );
    for n in min_n..=max_n {
        let result = diagnose_peak_layer_rank(n)?;
        let verified = result.ranks[0] == result.ranks[1];
        println!(
            "{n},{},{},{},{},{},{},{:.9},{:.9},{},{verified}",
            result.selected_row,
            result.support,
            result.ranks[0],
            result.left_factor_nnz[0],
            result.right_factor_nnz[0],
            result.reconstruction_products[0],
            result.reconstruction_products[0] as f64 / result.support as f64,
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
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

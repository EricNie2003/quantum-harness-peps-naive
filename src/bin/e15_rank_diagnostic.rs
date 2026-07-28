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
        return Err("usage: e15_rank_diagnostic MIN_N MAX_N".to_owned());
    }
    println!(
        "N,selected_row,support,left_patterns,right_patterns,rank_p1000000007,rank_p1000000009,rank_support_ratio,peak_elimination_row_nnz_p1,peak_elimination_row_nnz_p2,elapsed_s,peak_rss_bytes,tensor_entries_examined,tensor_entries_matched,row_operator_candidates,row_operator_matched"
    );
    for n in min_n..=max_n {
        let result = diagnose_peak_layer_rank(n)?;
        if result.ranks[0] != result.ranks[1] {
            return Err(format!("finite-field ranks disagree at N={n}"));
        }
        println!(
            "{n},{},{},{},{},{},{},{:.9},{},{},{:.9},{},{},{},{},{}",
            result.selected_row,
            result.support,
            result.left_patterns,
            result.right_patterns,
            result.ranks[0],
            result.ranks[1],
            result.ranks[0] as f64 / result.support as f64,
            result.peak_elimination_row_nnz[0],
            result.peak_elimination_row_nnz[1],
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            17,
            17,
            result.row_operator_candidates,
            result.row_operator_matched
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

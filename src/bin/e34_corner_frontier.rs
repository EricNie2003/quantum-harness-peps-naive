use nqueens_peps_naive::{ExplicitFrontierOrder, contract_explicit_c_frontier, known_count};
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
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let support_limit = parse(args.next(), "SUPPORT_LIMIT")?;
    if args.next().is_some() || min_n > max_n || support_limit == 0 {
        return Err("usage: e34_corner_frontier MIN_N MAX_N SUPPORT_LIMIT".to_owned());
    }
    println!(
        "order,N,count,known_count,verified,complete,elapsed_s,peak_rss_bytes,peak_support,\
         peak_open_bonds,tensor_entries_examined,tensor_entries_accepted,contracted_sites,\
         total_sites,support_limit"
    );
    for n in min_n..=max_n {
        for order in [
            ExplicitFrontierOrder::RowMajor,
            ExplicitFrontierOrder::TopLeftDiamond,
        ] {
            let result = contract_explicit_c_frontier(n, order, support_limit)?;
            let expected = known_count(n);
            println!(
                "{order:?},{n},{},{},{},{},{:.9},{},{},{},{},{},{},{},{}",
                result
                    .count
                    .map_or_else(String::new, |value| value.to_string()),
                expected.map_or_else(String::new, |value| value.to_string()),
                result.count.is_some() && result.count == expected,
                result.complete,
                result.elapsed.as_secs_f64(),
                result.peak_rss_bytes,
                result.peak_states,
                result.peak_open_bonds,
                result.tensor_entries_examined,
                result.tensor_entries_accepted,
                result.contracted_sites,
                n * n,
                support_limit,
            );
        }
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

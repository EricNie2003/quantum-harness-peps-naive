use nqueens_peps_naive::known_count;
use nqueens_peps_naive::weighted_dd::contract_weighted_dd;
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
        return Err("usage: e21_weighted_dd MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_live_nodes,peak_boundary_nodes,peak_relation_nodes,allocated_nodes,terminal_count,tensor_entries_examined,tensor_entries_accepted,unique_lookups,unique_hits,apply_lookups,apply_hits,relprod_lookups,relprod_hits"
    );
    for n in min_n..=max_n {
        let result = contract_weighted_dd(n)?;
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        println!(
            "{n},{},{},{verified},{:.9},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_live_nodes,
            result.peak_boundary_nodes,
            result.peak_relation_nodes,
            result.allocated_nodes,
            result.terminal_count,
            result.tensor_entries_examined,
            result.tensor_entries_accepted,
            result.unique_lookups,
            result.unique_hits,
            result.apply_lookups,
            result.apply_hits,
            result.relprod_lookups,
            result.relprod_hits,
        );
        for layer in result.layers {
            eprintln!(
                "layer,N={n},row={},boundary_nodes={}",
                layer.row + 1,
                layer.boundary_nodes
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

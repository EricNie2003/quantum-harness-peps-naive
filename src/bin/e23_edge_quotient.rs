use nqueens_peps_naive::known_count;
use nqueens_peps_naive::weighted_dd::{
    DdColumnOrder, DdOrderMode, DdOrderSpec, contract_weighted_dd_with_order,
};
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
        return Err("usage: e23_edge_quotient MIN_N MAX_N".to_owned());
    }
    let order = DdOrderSpec {
        families: [2, 0, 1],
        columns: DdColumnOrder::Reverse,
        mode: DdOrderMode::FamilyPaired,
    };
    println!(
        "N,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_boundary_nodes,peak_weighted_nodes_p1,peak_weighted_nodes_p2,weighted_boundary_ratio_p1,weighted_boundary_ratio_p2,field_multiplications_p1,field_multiplications_p2,inversions_p1,inversions_p2,allocated_add_nodes"
    );
    for n in min_n..=max_n {
        let result = contract_weighted_dd_with_order(n, order)?;
        let expected = known_count(n);
        let verified = expected == Some(result.count)
            && result.peak_edge_quotient_nodes[0] == result.peak_edge_quotient_nodes[1];
        println!(
            "{n},{},{},{verified},{:.9},{},{},{},{},{:.9},{:.9},{},{},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_boundary_nodes,
            result.peak_edge_quotient_nodes[0],
            result.peak_edge_quotient_nodes[1],
            result.peak_edge_quotient_nodes[0] as f64 / result.peak_boundary_nodes as f64,
            result.peak_edge_quotient_nodes[1] as f64 / result.peak_boundary_nodes as f64,
            result.edge_quotient_field_multiplications[0],
            result.edge_quotient_field_multiplications[1],
            result.edge_quotient_inversions[0],
            result.edge_quotient_inversions[1],
            result.allocated_nodes,
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

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

fn candidates() -> Vec<DdOrderSpec> {
    let families = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let columns = [
        DdColumnOrder::Forward,
        DdColumnOrder::Reverse,
        DdColumnOrder::CenterOut,
    ];
    let modes = [
        DdOrderMode::SiteBlocked,
        DdOrderMode::Paired,
        DdOrderMode::FamilyPaired,
    ];
    let mut result = Vec::with_capacity(54);
    for family_order in families {
        for column_order in columns {
            for mode in modes {
                result.push(DdOrderSpec {
                    families: family_order,
                    columns: column_order,
                    mode,
                });
            }
        }
    }
    result
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let n = parse_usize(args.next(), "N")?;
    if args.next().is_some() {
        return Err("usage: e22_dd_order_search N".to_owned());
    }
    println!(
        "N,order,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_live_nodes,peak_boundary_nodes,peak_relation_nodes,allocated_nodes,terminal_count,relprod_lookups,relprod_hits"
    );
    let expected = known_count(n);
    let mut best = None::<(usize, usize, DdOrderSpec)>;
    for order in candidates() {
        let result = contract_weighted_dd_with_order(n, order)?;
        let verified = expected == Some(result.count);
        if expected.is_some() && !verified {
            return Err(format!("known-count mismatch for {}", result.order));
        }
        println!(
            "{n},{},{},{},{verified},{:.9},{},{},{},{},{},{},{},{}",
            result.order,
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_live_nodes,
            result.peak_boundary_nodes,
            result.peak_relation_nodes,
            result.allocated_nodes,
            result.terminal_count,
            result.relprod_lookups,
            result.relprod_hits,
        );
        let candidate = (result.peak_boundary_nodes, result.peak_live_nodes, order);
        if best.is_none_or(|current| {
            candidate.0 < current.0 || (candidate.0 == current.0 && candidate.1 < current.1)
        }) {
            best = Some(candidate);
        }
    }
    if let Some((boundary, live, order)) = best {
        eprintln!(
            "best,N={n},order={},peak_boundary_nodes={boundary},peak_live_nodes={live}",
            order.label()
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

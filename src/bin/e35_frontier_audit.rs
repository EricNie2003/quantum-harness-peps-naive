use nqueens_peps_naive::frontier_audit::audit_frontier_distinguishability;
use nqueens_peps_naive::known_count;
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
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e35_frontier_audit MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count,known_count,verified,certified,elapsed_s,peak_rss_bytes,\
         peak_reachable_states,peak_certified_classes,peak_class_ratio,\
         forward_transitions,signature_transitions,witness_replay_transitions,\
         quotient_edges,exact_signature_comparisons,fingerprint_collision_witnesses"
    );
    for n in min_n..=max_n {
        let result = audit_frontier_distinguishability(n)?;
        let expected = known_count(n);
        println!(
            "{n},{},{},{},{},{:.9},{},{},{},{:.9},{},{},{},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            expected == Some(result.count),
            result.certified,
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_reachable_states,
            result.peak_certified_classes,
            result.peak_certified_classes as f64 / result.peak_reachable_states as f64,
            result.forward_transitions,
            result.signature_transitions,
            result.witness_replay_transitions,
            result.quotient_edges,
            result.exact_signature_comparisons,
            result.fingerprint_collision_witnesses,
        );
        eprintln!("layers for N={n}:");
        for layer in result.layers {
            eprintln!(
                "{},{},{},{}",
                n, layer.row, layer.reachable_states, layer.certified_classes
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

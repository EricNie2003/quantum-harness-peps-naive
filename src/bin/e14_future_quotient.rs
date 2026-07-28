use nqueens_peps_naive::future_quotient::analyze_future_equivalence;
use nqueens_peps_naive::known_count;
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
        return Err("usage: e14_future_quotient MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count,known_count,verified,elapsed_s,quotient_replay_s,peak_rss_bytes,peak_reachable_states,peak_future_classes,peak_class_ratio,forward_transitions,backward_signature_transitions,quotient_edges"
    );
    for n in min_n..=max_n {
        let result = analyze_future_equivalence(n)?;
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        if expected.is_some() && !verified {
            return Err(format!("known-count mismatch at N={n}"));
        }
        println!(
            "{n},{},{},{verified},{:.9},{:.9},{},{},{},{:.9},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            result.elapsed.as_secs_f64(),
            result.quotient_replay_elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_reachable_states,
            result.peak_future_classes,
            result.peak_future_classes as f64 / result.peak_reachable_states as f64,
            result.forward_transitions,
            result.backward_signature_transitions,
            result.quotient_edges
        );
        for layer in &result.layers {
            eprintln!(
                "layer,N={},row={},reachable={},classes={},ratio={:.9}",
                n,
                layer.row,
                layer.reachable_states,
                layer.future_classes,
                layer.future_classes as f64 / layer.reachable_states as f64
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

use nqueens_peps_naive::future_quotient::analyze_online_future_equivalence;
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
        return Err("usage: e18_online_future_quotient MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count,known_count,verified,elapsed_s,peak_rss_bytes,peak_concrete_states,peak_future_classes,total_concrete_states,total_future_classes,transitions"
    );
    for n in min_n..=max_n {
        let result = analyze_online_future_equivalence(n)?;
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        println!(
            "{n},{},{},{verified},{:.9},{},{},{},{},{},{}",
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_concrete_states,
            result.peak_future_classes,
            result.concrete_states,
            result.future_classes,
            result.transitions,
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

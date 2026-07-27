use nqueens_peps_naive::{contract_rows, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn usage() {
    eprintln!(
        "Usage:\n  nqueens-peps-naive solve N [--layers]\n  \
         nqueens-peps-naive bench MAX_N [--min N] [--repeats R] [--csv]"
    );
}

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn median(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn solve(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let n = parse_usize(args.next(), "N")?;
    let layers = args.any(|arg| arg == "--layers");
    let result = contract_rows(n)?;
    let expected = known_count(n);
    let verified = expected == Some(result.count);

    println!(
        "N={} Q(N)={} elapsed_s={:.6} peak_states={} accepted_transitions={} verified={}",
        n,
        result.count,
        result.elapsed.as_secs_f64(),
        result.peak_states,
        result.total_accepted_transitions,
        verified
    );
    if layers {
        println!(
            "row,input_states,candidate_transitions,accepted_transitions,output_states,output_weight,elapsed_s"
        );
        for layer in result.layers {
            println!(
                "{},{},{},{},{},{},{:.6}",
                layer.row + 1,
                layer.input_states,
                layer.candidate_transitions,
                layer.accepted_transitions,
                layer.output_states,
                layer.output_weight,
                layer.elapsed.as_secs_f64()
            );
        }
    }
    if expected.is_some() && !verified {
        return Err(format!("known-count verification failed for N={n}"));
    }
    Ok(())
}

fn bench(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let rest: Vec<String> = args.collect();
    let mut min_n = 1_usize;
    let mut repeats = 3_usize;
    let mut csv = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--min" => {
                min_n = parse_usize(rest.get(index + 1).cloned(), "--min value")?;
                index += 2;
            }
            "--repeats" => {
                repeats = parse_usize(rest.get(index + 1).cloned(), "--repeats value")?;
                index += 2;
            }
            "--csv" => {
                csv = true;
                index += 1;
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    if repeats == 0 {
        return Err("--repeats must be positive".to_owned());
    }
    if min_n > max_n {
        return Err("--min must not exceed MAX_N".to_owned());
    }

    if csv {
        println!(
            "N,count,known_count,verified,median_elapsed_s,min_elapsed_s,peak_states,candidate_transitions,accepted_transitions,repeats"
        );
    }
    for n in min_n..=max_n {
        let mut elapsed = Vec::with_capacity(repeats);
        let mut last = None;
        for _ in 0..repeats {
            let result = contract_rows(n)?;
            elapsed.push(result.elapsed);
            last = Some(result);
        }
        let result = last.unwrap();
        let minimum = *elapsed.iter().min().unwrap();
        let med = median(&mut elapsed);
        let known = known_count(n);
        let verified = known == Some(result.count);
        if known.is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        if csv {
            println!(
                "{},{},{},{},{:.9},{:.9},{},{},{},{}",
                n,
                result.count,
                known.map_or_else(String::new, |v| v.to_string()),
                verified,
                med.as_secs_f64(),
                minimum.as_secs_f64(),
                result.peak_states,
                result.total_candidate_transitions,
                result.total_accepted_transitions,
                repeats
            );
        } else {
            println!(
                "N={n:>2} Q(N)={:<10} median={:.6}s peak_states={:<8} verified={verified}",
                result.count,
                med.as_secs_f64(),
                result.peak_states
            );
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("solve") => solve(args),
        Some("bench") => bench(args),
        _ => {
            usage();
            Err("missing or invalid command".to_owned())
        }
    }
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

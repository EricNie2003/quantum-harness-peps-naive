use nqueens_peps_naive::{
    ContractionResult, contract_rows, contract_rows_sort_reduce, known_count,
};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Backend {
    Hash,
    SortReduce,
}

impl Backend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "hash" => Ok(Self::Hash),
            "sort-reduce" => Ok(Self::SortReduce),
            _ => Err("BACKEND must be hash or sort-reduce".to_owned()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::SortReduce => "sort_reduce",
        }
    }

    fn run(self, n: usize) -> Result<ContractionResult, String> {
        match self {
            Self::Hash => contract_rows(n),
            Self::SortReduce => contract_rows_sort_reduce(n),
        }
    }
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

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let backend = Backend::parse(
        &args
            .next()
            .ok_or_else(|| "usage: sort_reduce BACKEND MIN_N MAX_N REPEATS".to_owned())?,
    )?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: sort_reduce BACKEND MIN_N MAX_N REPEATS".to_owned());
    }

    println!(
        "backend,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,peak_support,row_operator_candidates,row_operator_matched,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::with_capacity(repeats);
        let mut last = None;
        for _ in 0..repeats {
            let result = backend.run(n)?;
            elapsed.push(result.elapsed);
            last = Some(result);
        }
        let result = last.expect("repeats is positive");
        let minimum = *elapsed.iter().min().expect("repeats is positive");
        let med = median(&mut elapsed);
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        if expected.is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        println!(
            "{},{},{},{},{},{:.9},{:.9},{},{},{},{},{}",
            backend.label(),
            n,
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            verified,
            med.as_secs_f64(),
            minimum.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_states,
            result.row_operator_candidates,
            result.row_operator_matched,
            repeats
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

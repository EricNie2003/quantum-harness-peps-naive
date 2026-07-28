use nqueens_peps_naive::{
    ContractionResult, contract_rows_hash_materialization, contract_rows_parallel_sort_reduce,
    contract_rows_sort_reduce, contract_rows_sparse_hash_materialization,
    contract_rows_sparse_parallel_sort_reduce, contract_rows_sparse_sort_reduce, known_count,
};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Backend {
    DenseHash,
    SparseHash,
    DenseSort,
    SparseSort,
    DenseParallel,
    SparseParallel,
}

impl Backend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "dense-hash" => Ok(Self::DenseHash),
            "sparse-hash" => Ok(Self::SparseHash),
            "dense-sort" => Ok(Self::DenseSort),
            "sparse-sort" => Ok(Self::SparseSort),
            "dense-parallel" => Ok(Self::DenseParallel),
            "sparse-parallel" => Ok(Self::SparseParallel),
            _ => Err("invalid BACKEND".to_owned()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::DenseHash => "dense_scan_hash",
            Self::SparseHash => "sparse_iterator_hash",
            Self::DenseSort => "dense_scan_sort_reduce",
            Self::SparseSort => "sparse_iterator_sort_reduce",
            Self::DenseParallel => "dense_scan_parallel_sort_reduce",
            Self::SparseParallel => "sparse_iterator_parallel_sort_reduce",
        }
    }

    fn run(self, n: usize, threads: usize) -> Result<ContractionResult, String> {
        match self {
            Self::DenseHash => contract_rows_hash_materialization(n),
            Self::SparseHash => contract_rows_sparse_hash_materialization(n),
            Self::DenseSort => contract_rows_sort_reduce(n),
            Self::SparseSort => contract_rows_sparse_sort_reduce(n),
            Self::DenseParallel => contract_rows_parallel_sort_reduce(n, threads),
            Self::SparseParallel => contract_rows_sparse_parallel_sort_reduce(n, threads),
        }
    }

    fn validate_threads(self, threads: usize) -> Result<(), String> {
        match self {
            Self::DenseParallel | Self::SparseParallel if threads > 0 => Ok(()),
            Self::DenseParallel | Self::SparseParallel => {
                Err("parallel backends require positive THREADS".to_owned())
            }
            _ if threads == 1 => Ok(()),
            _ => Err("serial/hash backends require THREADS=1".to_owned()),
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
    let backend = Backend::parse(&args.next().ok_or_else(|| {
        "usage: e11_sparse_iterator BACKEND THREADS MIN_N MAX_N REPEATS".to_owned()
    })?)?;
    let threads = parse_usize(args.next(), "THREADS")?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e11_sparse_iterator BACKEND THREADS MIN_N MAX_N REPEATS".to_owned());
    }
    backend.validate_threads(threads)?;

    println!(
        "backend,threads,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,peak_support,tensor_entries_examined,tensor_entries_matched,row_operator_candidates,row_operator_matched,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::with_capacity(repeats);
        let mut last = None;
        for _ in 0..repeats {
            let result = backend.run(n, threads)?;
            elapsed.push(result.elapsed);
            last = Some(result);
        }
        let result = last.expect("positive repeats");
        let minimum = *elapsed.iter().min().expect("positive repeats");
        let med = median(&mut elapsed);
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        if expected.is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        println!(
            "{},{},{},{},{},{},{:.9},{:.9},{},{},{},{},{},{},{}",
            backend.label(),
            threads,
            n,
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            verified,
            med.as_secs_f64(),
            minimum.as_secs_f64(),
            result.peak_rss_bytes,
            result.peak_states,
            result.tensor_entries_examined,
            result.tensor_entries_matched,
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

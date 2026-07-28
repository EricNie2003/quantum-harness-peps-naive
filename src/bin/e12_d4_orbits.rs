use nqueens_peps_naive::{
    ContractionResult, contract_rows_d4_orbit_parallel_sort_reduce,
    contract_rows_d4_orbit_sort_reduce, contract_rows_d4_sparse_parallel_sort_reduce,
    contract_rows_d4_sparse_sort_reduce, contract_rows_parallel_sort_reduce,
    contract_rows_sort_reduce, contract_rows_sparse_parallel_sort_reduce,
    contract_rows_sparse_sort_reduce, known_count,
};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Backend {
    DenseSerial,
    D4Serial,
    DenseParallel,
    D4Parallel,
    SparseSerial,
    D4SparseSerial,
    SparseParallel,
    D4SparseParallel,
}

impl Backend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "dense-serial" => Ok(Self::DenseSerial),
            "d4-serial" => Ok(Self::D4Serial),
            "dense-parallel" => Ok(Self::DenseParallel),
            "d4-parallel" => Ok(Self::D4Parallel),
            "sparse-serial" => Ok(Self::SparseSerial),
            "d4-sparse-serial" => Ok(Self::D4SparseSerial),
            "sparse-parallel" => Ok(Self::SparseParallel),
            "d4-sparse-parallel" => Ok(Self::D4SparseParallel),
            _ => Err(
                "BACKEND must be dense-serial, d4-serial, dense-parallel, d4-parallel, \
                 sparse-serial, d4-sparse-serial, sparse-parallel, or d4-sparse-parallel"
                    .to_owned(),
            ),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::DenseSerial => "dense_serial_sort_reduce",
            Self::D4Serial => "d4_orbit_serial_sort_reduce",
            Self::DenseParallel => "dense_parallel_sort_reduce",
            Self::D4Parallel => "d4_orbit_parallel_sort_reduce",
            Self::SparseSerial => "sparse_serial_sort_reduce",
            Self::D4SparseSerial => "d4_sparse_serial_sort_reduce",
            Self::SparseParallel => "sparse_parallel_sort_reduce",
            Self::D4SparseParallel => "d4_sparse_parallel_sort_reduce",
        }
    }

    fn run(self, n: usize, threads: usize) -> Result<ContractionResult, String> {
        match self {
            Self::DenseSerial => contract_rows_sort_reduce(n),
            Self::D4Serial => contract_rows_d4_orbit_sort_reduce(n),
            Self::DenseParallel => contract_rows_parallel_sort_reduce(n, threads),
            Self::D4Parallel => contract_rows_d4_orbit_parallel_sort_reduce(n, threads),
            Self::SparseSerial => contract_rows_sparse_sort_reduce(n),
            Self::D4SparseSerial => contract_rows_d4_sparse_sort_reduce(n),
            Self::SparseParallel => contract_rows_sparse_parallel_sort_reduce(n, threads),
            Self::D4SparseParallel => contract_rows_d4_sparse_parallel_sort_reduce(n, threads),
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
    let backend =
        Backend::parse(&args.next().ok_or_else(|| {
            "usage: e12_d4_orbits BACKEND THREADS MIN_N MAX_N REPEATS".to_owned()
        })?)?;
    let threads = parse_usize(args.next(), "THREADS")?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    if args.next().is_some() || threads == 0 || min_n > max_n || repeats == 0 {
        return Err("usage: e12_d4_orbits BACKEND THREADS MIN_N MAX_N REPEATS".to_owned());
    }

    println!(
        "backend,threads,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,peak_support,tensor_entries_examined,tensor_entries_matched,row_operator_candidates,row_operator_matched,d4_group_size,cut_stabilizer_size,top_row_orbit_representatives,top_row_fixed_points,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::with_capacity(repeats);
        let mut last = None;
        for _ in 0..repeats {
            let result = backend.run(n, threads)?;
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
            "{},{},{},{},{},{},{:.9},{:.9},{},{},{},{},{},{},{},{},{},{},{}",
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
            8,
            if n > 1 { 2 } else { 8 },
            n.div_ceil(2),
            n % 2,
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

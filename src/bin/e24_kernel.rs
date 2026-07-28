use nqueens_peps_naive::{
    ContractionResult, contract_rows_d4_arena_sort_reduce, contract_rows_d4_batched_radix,
    contract_rows_d4_batched_sort_reduce, contract_rows_d4_batched_sparse_parallel_sort,
    contract_rows_d4_batched_sparse_sort_reduce, contract_rows_d4_deferred_sparse_sort_reduce,
    contract_rows_d4_orbit_sort_reduce, known_count,
};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Variant {
    Baseline,
    Arena,
    Batched,
    BatchedSparse,
    BatchedSparseParallelSort,
    DeferredSparse,
    Radix,
}

impl Variant {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "baseline" => Ok(Self::Baseline),
            "arena" => Ok(Self::Arena),
            "batched" => Ok(Self::Batched),
            "batched-sparse" => Ok(Self::BatchedSparse),
            "batched-sparse-parsort" => Ok(Self::BatchedSparseParallelSort),
            "deferred-sparse" => Ok(Self::DeferredSparse),
            "radix" => Ok(Self::Radix),
            _ => Err("VARIANT must be baseline, arena, batched, batched-sparse, \
                 batched-sparse-parsort, deferred-sparse, or radix"
                .to_owned()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Arena => "arena",
            Self::Batched => "arena_batched",
            Self::BatchedSparse => "arena_batched_sparse",
            Self::BatchedSparseParallelSort => "arena_batched_sparse_parallel_sort",
            Self::DeferredSparse => "deferred_sparse",
            Self::Radix => "arena_batched_radix",
        }
    }

    fn run(self, n: usize) -> Result<ContractionResult, String> {
        match self {
            Self::Baseline => contract_rows_d4_orbit_sort_reduce(n),
            Self::Arena => contract_rows_d4_arena_sort_reduce(n),
            Self::Batched => contract_rows_d4_batched_sort_reduce(n),
            Self::BatchedSparse => contract_rows_d4_batched_sparse_sort_reduce(n),
            Self::BatchedSparseParallelSort => contract_rows_d4_batched_sparse_parallel_sort(n),
            Self::DeferredSparse => contract_rows_d4_deferred_sparse_sort_reduce(n),
            Self::Radix => contract_rows_d4_batched_radix(n),
        }
    }
}

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn median(values: &mut [Duration]) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let variant = Variant::parse(
        &args
            .next()
            .ok_or_else(|| "usage: e24_kernel VARIANT MIN_N MAX_N REPEATS".to_owned())?,
    )?;
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let repeats = parse_usize(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e24_kernel VARIANT MIN_N MAX_N REPEATS".to_owned());
    }
    println!(
        "variant,N,count,known_count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,peak_support,tensor_entries_examined,tensor_entries_matched,row_operator_candidates,row_operator_matched,repeats"
    );
    for n in min_n..=max_n {
        let mut durations = Vec::with_capacity(repeats);
        let mut final_result = None;
        let mut min_elapsed = Duration::MAX;
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = variant.run(n)?;
            min_elapsed = min_elapsed.min(result.elapsed);
            peak_rss = peak_rss.max(result.peak_rss_bytes);
            durations.push(result.elapsed);
            final_result = Some(result);
        }
        let result = final_result.expect("at least one repetition");
        let median_elapsed = median(&mut durations);
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        println!(
            "{},{n},{},{},{verified},{:.9},{:.9},{},{},{},{},{},{},{}",
            variant.label(),
            result.count,
            expected.map_or_else(String::new, |value| value.to_string()),
            median_elapsed.as_secs_f64(),
            min_elapsed.as_secs_f64(),
            peak_rss,
            result.peak_states,
            result.tensor_entries_examined,
            result.tensor_entries_matched,
            result.row_operator_candidates,
            result.row_operator_matched,
            repeats,
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

use nqueens_peps_naive::{contract_rows_d4_recursive_tail, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let shards = parse(args.next(), "SHARDS")?;
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let min_cut = parse(args.next(), "MIN_CUT")?;
    let max_cut = parse(args.next(), "MAX_CUT")?;
    let repeats = parse(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || min_cut > max_cut || repeats == 0 {
        return Err(
            "usage: e38_recursive_tail SHARDS MIN_N MAX_N MIN_CUT MAX_CUT REPEATS".to_owned(),
        );
    }
    println!(
        "shards,N,cut,count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,\
         peak_support,prefix_support,tail_tasks,recursive_nodes,recursive_accepted_entries,\
         prefix_s,tail_s,generation_s,sort_s,reduce_s,coefficient_bits,\
         max_prefix_coefficient,peak_thread_local_bytes,total_accepted_entries,repeats"
    );
    for n in min_n..=max_n {
        for cut in min_cut..=max_cut.min(n) {
            let mut elapsed = Vec::<Duration>::with_capacity(repeats);
            let mut last = None;
            let mut peak_rss = 0_u64;
            for _ in 0..repeats {
                let result = contract_rows_d4_recursive_tail(n, shards, cut)?;
                peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
                elapsed.push(result.contraction.elapsed);
                last = Some(result);
            }
            elapsed.sort_unstable();
            let result = last.expect("positive repeat count");
            println!(
                "{shards},{n},{cut},{},{},{:.9},{:.9},{},{},{},{},{},{},{:.9},{:.9},\
                 {:.9},{:.9},{:.9},{},{},{},{},{}",
                result.contraction.count,
                known_count(n) == Some(result.contraction.count),
                elapsed[elapsed.len() / 2].as_secs_f64(),
                elapsed[0].as_secs_f64(),
                peak_rss,
                result.contraction.peak_states,
                result.prefix_support,
                result.tail_tasks,
                result.recursive_nodes,
                result.recursive_accepted_entries,
                result.prefix_elapsed.as_secs_f64(),
                result.tail_elapsed.as_secs_f64(),
                result.generation_elapsed.as_secs_f64(),
                result.sort_elapsed.as_secs_f64(),
                result.reduce_elapsed.as_secs_f64(),
                result.coefficient_bits,
                result.max_prefix_coefficient,
                result.peak_thread_local_bytes,
                result.contraction.row_operator_matched,
                repeats,
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

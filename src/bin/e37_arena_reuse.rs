use nqueens_peps_naive::{contract_rows_d4_joint_u64_arena_reuse, known_count};
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
    let repeats = parse(args.next(), "REPEATS")?;
    if args.next().is_some() || min_n > max_n || repeats == 0 {
        return Err("usage: e37_arena_reuse SHARDS MIN_N MAX_N REPEATS".to_owned());
    }
    println!(
        "shards,N,count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,peak_support,\
         generation_s,sort_s,reduce_s,total_reused_capacity_bytes,\
         total_destination_growth_bytes,peak_spare_capacity_bytes,\
         peak_thread_local_bytes,row_operator_candidates,row_operator_matched,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut last = None;
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_d4_joint_u64_arena_reuse(n, shards)?;
            peak_rss = peak_rss.max(result.joint.contraction.peak_rss_bytes);
            elapsed.push(result.joint.contraction.elapsed);
            last = Some(result);
        }
        elapsed.sort_unstable();
        let result = last.expect("positive repeat count");
        println!(
            "{shards},{n},{},{},{:.9},{:.9},{},{},{:.9},{:.9},{:.9},{},{},{},{},{},{},{}",
            result.joint.contraction.count,
            known_count(n) == Some(result.joint.contraction.count),
            elapsed[elapsed.len() / 2].as_secs_f64(),
            elapsed[0].as_secs_f64(),
            peak_rss,
            result.joint.contraction.peak_states,
            result.joint.generation_elapsed.as_secs_f64(),
            result.joint.sort_elapsed.as_secs_f64(),
            result.joint.reduce_elapsed.as_secs_f64(),
            result.total_reused_capacity_bytes,
            result.total_destination_growth_bytes,
            result.peak_spare_capacity_bytes,
            result.joint.peak_thread_local_bytes,
            result.joint.contraction.row_operator_candidates,
            result.joint.contraction.row_operator_matched,
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

use nqueens_peps_naive::{contract_rows_certified_fast_tail, known_count};
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
    let n = parse(args.next(), "N")?;
    let min_cut = parse(args.next(), "MIN_CUT")?;
    let max_cut = parse(args.next(), "MAX_CUT")?;
    let repeats = parse(args.next(), "REPEATS")?;
    if args.next().is_some() || min_cut > max_cut || repeats == 0 {
        return Err("usage: e40_fast_tail SHARDS N MIN_CUT MAX_CUT REPEATS".to_owned());
    }
    println!(
        "shards,N,cut,count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,\
         prefix_support,used_u64_fast_path,prefix_s,tail_s,profile_replay_s,\
         recursive_nodes,recursive_accepted_entries,total_accepted_entries,repeats"
    );
    for cut in min_cut..=max_cut.min(n) {
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_certified_fast_tail(n, shards, cut, false)?;
            peak_rss = peak_rss.max(result.contraction.peak_rss_bytes);
            elapsed.push(result.contraction.elapsed);
        }
        elapsed.sort_unstable();
        let profile = contract_rows_certified_fast_tail(n, shards, cut, true)?;
        peak_rss = peak_rss.max(profile.contraction.peak_rss_bytes);
        println!(
            "{shards},{n},{cut},{},{},{:.9},{:.9},{},{},{},{:.9},{:.9},{:.9},{},{},{},{}",
            profile.contraction.count,
            known_count(n) == Some(profile.contraction.count),
            elapsed[elapsed.len() / 2].as_secs_f64(),
            elapsed[0].as_secs_f64(),
            peak_rss,
            profile.prefix_support,
            profile.used_u64_fast_path,
            profile.prefix_elapsed.as_secs_f64(),
            profile.tail_elapsed.as_secs_f64(),
            profile.profile_replay_elapsed.as_secs_f64(),
            profile.recursive_nodes,
            profile.recursive_accepted_entries,
            profile.contraction.row_operator_matched,
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

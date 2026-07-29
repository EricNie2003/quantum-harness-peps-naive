use nqueens_peps_naive::{contract_rows_adaptive_fast_tail, known_count};
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
        return Err("usage: e40_adaptive SHARDS MIN_N MAX_N REPEATS".to_owned());
    }
    println!(
        "shards,N,selected_cut,count,verified,median_elapsed_s,min_elapsed_s,peak_rss_bytes,\
         target_tail_tasks,prefix_support,used_u64_fast_path,selection_s,prefix_s,tail_s,\
         profile_replay_s,recursive_nodes,recursive_accepted_entries,total_accepted_entries,\
         probes,repeats"
    );
    for n in min_n..=max_n {
        let mut elapsed = Vec::<Duration>::with_capacity(repeats);
        let mut peak_rss = 0_u64;
        for _ in 0..repeats {
            let result = contract_rows_adaptive_fast_tail(n, shards, false)?;
            peak_rss = peak_rss.max(result.fast.contraction.peak_rss_bytes);
            elapsed.push(result.fast.contraction.elapsed);
        }
        elapsed.sort_unstable();
        let profile = contract_rows_adaptive_fast_tail(n, shards, true)?;
        peak_rss = peak_rss.max(profile.fast.contraction.peak_rss_bytes);
        let probes = profile
            .probes
            .iter()
            .map(|probe| {
                format!(
                    "{}:{}:{:.6}",
                    probe.cut,
                    probe.prefix_support,
                    probe.prefix_elapsed.as_secs_f64()
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        println!(
            "{shards},{n},{},{},{},{:.9},{:.9},{},{},{},{},{:.9},{:.9},{:.9},{:.9},\
             {},{},{},{},{}",
            profile.selected_cut,
            profile.fast.contraction.count,
            known_count(n) == Some(profile.fast.contraction.count),
            elapsed[elapsed.len() / 2].as_secs_f64(),
            elapsed[0].as_secs_f64(),
            peak_rss,
            profile.target_tail_tasks,
            profile.fast.prefix_support,
            profile.fast.used_u64_fast_path,
            profile.selection_elapsed.as_secs_f64(),
            profile.fast.prefix_elapsed.as_secs_f64(),
            profile.fast.tail_elapsed.as_secs_f64(),
            profile.fast.profile_replay_elapsed.as_secs_f64(),
            profile.fast.recursive_nodes,
            profile.fast.recursive_accepted_entries,
            profile.fast.contraction.row_operator_matched,
            probes,
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

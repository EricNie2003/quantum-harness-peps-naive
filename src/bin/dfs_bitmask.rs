use nqueens_peps_naive::{
    dfs_bitmask::{count_dfs_bitmask, profile_dfs_bitmask},
    known_count, peak_rss_bytes,
};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn usage() {
    eprintln!(
        "Usage:\n  dfs_bitmask solve N [--threads T]\n  \
         dfs_bitmask bench MAX_N [--min N] [--threads T] [--repeats R] \
         [--warmup R] [--csv]"
    );
}

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

#[derive(Clone, Copy)]
struct Options {
    threads: usize,
    repeats: usize,
    warmup: usize,
    csv: bool,
}

fn parse_options(rest: &[String], allow_benchmark_options: bool) -> Result<Options, String> {
    let mut options = Options {
        threads: 1,
        repeats: 9,
        warmup: 1,
        csv: false,
    };
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--threads" => {
                options.threads = parse_usize(rest.get(index + 1).cloned(), "--threads value")?;
                index += 2;
            }
            "--repeats" if allow_benchmark_options => {
                options.repeats = parse_usize(rest.get(index + 1).cloned(), "--repeats value")?;
                index += 2;
            }
            "--warmup" if allow_benchmark_options => {
                options.warmup = parse_usize(rest.get(index + 1).cloned(), "--warmup value")?;
                index += 2;
            }
            "--csv" if allow_benchmark_options => {
                options.csv = true;
                index += 1;
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    if options.threads == 0 {
        return Err("--threads must be positive".to_owned());
    }
    if options.repeats == 0 {
        return Err("--repeats must be positive".to_owned());
    }
    Ok(options)
}

fn percentile(sorted: &[Duration], numerator: usize, denominator: usize) -> Duration {
    let index = (sorted.len() - 1) * numerator / denominator;
    sorted[index]
}

fn solve(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let n = parse_usize(args.next(), "N")?;
    let options = parse_options(&args.collect::<Vec<_>>(), false)?;
    let result = profile_dfs_bitmask(n, options.threads)?;
    let expected = known_count(n);
    let verified = expected == Some(result.count);
    if expected.is_some() && !verified {
        return Err(format!("known-count verification failed for N={n}"));
    }
    println!(
        "method=dfs_bitmask_comparator N={} Q(N)={} elapsed_s={:.9} \
         threads={} split_depth={} tasks={} recursive_nodes={} \
             candidate_placements={} peak_rss_bytes={} verified={} metrics_collected={}",
        n,
        result.count,
        result.elapsed.as_secs_f64(),
        result.threads,
        result.split_depth,
        result.tasks,
        result.recursive_nodes,
        result.candidate_placements,
        peak_rss_bytes(),
        verified,
        result.metrics_collected
    );
    Ok(())
}

fn bench(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let max_n = parse_usize(args.next(), "MAX_N")?;
    let rest = args.collect::<Vec<_>>();
    let mut min_n = 1_usize;
    let mut filtered = Vec::with_capacity(rest.len());
    let mut index = 0;
    while index < rest.len() {
        if rest[index] == "--min" {
            min_n = parse_usize(rest.get(index + 1).cloned(), "--min value")?;
            index += 2;
        } else {
            filtered.push(rest[index].clone());
            index += 1;
        }
    }
    let options = parse_options(&filtered, true)?;
    if min_n > max_n {
        return Err("--min must not exceed MAX_N".to_owned());
    }

    if options.csv {
        println!(
            "N,count,known_count,verified,median_elapsed_s,min_elapsed_s,p10_elapsed_s,\
             p90_elapsed_s,peak_rss_bytes,threads,split_depth,tasks,recursive_nodes,\
             candidate_placements,metrics_elapsed_s,repeats,warmup,algorithm_class,peak_sparse_support,\
             local_tensor_entries_examined,local_tensor_entries_accepted"
        );
    }

    for n in min_n..=max_n {
        for _ in 0..options.warmup {
            count_dfs_bitmask(n, options.threads)?;
        }
        let mut elapsed = Vec::with_capacity(options.repeats);
        let mut last = None;
        for _ in 0..options.repeats {
            let result = count_dfs_bitmask(n, options.threads)?;
            elapsed.push(result.elapsed);
            last = Some(result);
        }
        elapsed.sort_unstable();
        let result = last.expect("positive repeat count");
        let profile = profile_dfs_bitmask(n, options.threads)?;
        if profile.count != result.count {
            return Err(format!(
                "instrumented/uninstrumented DFS mismatch for N={n}"
            ));
        }
        let minimum = elapsed[0];
        let median = percentile(&elapsed, 1, 2);
        let p10 = percentile(&elapsed, 1, 10);
        let p90 = percentile(&elapsed, 9, 10);
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        if expected.is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        let rss = peak_rss_bytes();

        if options.csv {
            println!(
                "{},{},{},{},{:.9},{:.9},{:.9},{:.9},{},{},{},{},{},{},{:.9},{},{},\
                 conventional_dfs_comparator,NA,NA,NA",
                n,
                result.count,
                expected.map_or_else(String::new, |value| value.to_string()),
                verified,
                median.as_secs_f64(),
                minimum.as_secs_f64(),
                p10.as_secs_f64(),
                p90.as_secs_f64(),
                rss,
                result.threads,
                result.split_depth,
                result.tasks,
                profile.recursive_nodes,
                profile.candidate_placements,
                profile.elapsed.as_secs_f64(),
                options.repeats,
                options.warmup
            );
        } else {
            println!(
                "N={n:>2} Q(N)={:<20} median={:.6}s min={:.6}s threads={} \
                 nodes={} verified={verified}",
                result.count,
                median.as_secs_f64(),
                minimum.as_secs_f64(),
                result.threads,
                result.recursive_nodes
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

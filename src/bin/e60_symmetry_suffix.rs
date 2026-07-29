use nqueens_peps_naive::audit_wide_vertical_suffix_tasks;
use std::env;
use std::process::ExitCode;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    let target_tasks_per_thread = parse(args.next(), "TARGET_TASKS_PER_THREAD")?;
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e60_symmetry_suffix MIN_N MAX_N TARGET_TASKS_PER_THREAD".to_owned());
    }
    println!(
        "N,split_depth,remaining_rows,tasks,exact_unique,canonical_unique,exact_duplicates,\
         additional_symmetry_duplicates,self_symmetric_states,exact_duplicate_rate,\
         additional_symmetry_duplicate_rate,total_canonical_duplicate_rate,elapsed_s,\
         peak_rss_bytes,threads,algorithm_class"
    );
    for n in min_n..=max_n {
        let audit = audit_wide_vertical_suffix_tasks(n, target_tasks_per_thread)?;
        println!(
            "{},{},{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{:.9},{},{},\
             explicit_C_vertical_canonical_suffix_audit",
            audit.n,
            audit.split_depth,
            audit.remaining_rows,
            audit.tasks,
            audit.exact_unique,
            audit.canonical_unique,
            audit.exact_duplicates,
            audit.additional_symmetry_duplicates,
            audit.self_symmetric_states,
            audit.exact_duplicate_rate,
            audit.additional_symmetry_duplicate_rate,
            audit.total_canonical_duplicate_rate,
            audit.elapsed.as_secs_f64(),
            audit.peak_rss_bytes,
            rayon::current_num_threads(),
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

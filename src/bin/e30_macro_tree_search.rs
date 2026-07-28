use nqueens_peps_naive::{contract_rows_d4_macro_pattern, known_count};
use std::env;
use std::process::ExitCode;
use std::time::Instant;

fn parse(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn enumerate_masks(row: usize, n: usize, mask: u64, output: &mut Vec<u64>) {
    if row >= n {
        output.push(mask);
        return;
    }
    enumerate_masks(row + 1, n, mask, output);
    if row + 1 < n {
        enumerate_masks(row + 2, n, mask | (1_u64 << row), output);
    }
}

fn blocks(mask: u64, n: usize) -> String {
    let mut row = 0_usize;
    let mut result = Vec::new();
    while row < n {
        if row + 1 < n && ((mask >> row) & 1) != 0 {
            result.push("2");
            row += 2;
        } else {
            result.push("1");
            row += 1;
        }
    }
    result.join("-")
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let shards = parse(args.next(), "SHARDS")?;
    let min_n = parse(args.next(), "MIN_N")?;
    let max_n = parse(args.next(), "MAX_N")?;
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e30_macro_tree_search SHARDS MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count,verified,candidates,search_elapsed_s,best_mask,best_blocks,\
         baseline_work,best_work,work_reduction,best_peak_candidates,best_elapsed_s,peak_rss_bytes"
    );
    for n in min_n..=max_n {
        let mut masks = Vec::new();
        enumerate_masks(0, n, 0, &mut masks);
        let search_start = Instant::now();
        let mut baseline_work = None;
        let mut best = None;
        for mask in masks.iter().copied() {
            let result = contract_rows_d4_macro_pattern(n, shards, mask)?;
            if mask == 0 {
                baseline_work = Some(result.contraction.row_operator_matched);
            }
            let score = (
                result.contraction.row_operator_matched,
                result.peak_macro_candidates,
                result.contraction.elapsed,
            );
            if best
                .as_ref()
                .is_none_or(|(_, _, best_score)| score < *best_score)
            {
                best = Some((mask, result, score));
            }
        }
        let search_elapsed = search_start.elapsed();
        let baseline_work = baseline_work.expect("all-single mask is enumerated");
        let (mask, result, _) = best.expect("at least one composition");
        let work = result.contraction.row_operator_matched;
        let reduction = 1.0 - work as f64 / baseline_work as f64;
        println!(
            "{n},{},{},{},{:.9},{mask:#x},{},{baseline_work},{work},{reduction:.9},{},{:.9},{}",
            result.contraction.count,
            known_count(n) == Some(result.contraction.count),
            masks.len(),
            search_elapsed.as_secs_f64(),
            blocks(mask, n),
            result.peak_macro_candidates,
            result.contraction.elapsed.as_secs_f64(),
            result.contraction.peak_rss_bytes,
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

use nqueens_peps_naive::{known_count, profile_boundary_diagrams};
use std::env;
use std::process::ExitCode;

fn run() -> Result<(), String> {
    let n: usize = env::args()
        .nth(1)
        .ok_or_else(|| "usage: zdd_profile N".to_owned())?
        .parse()
        .map_err(|_| "N must be a non-negative integer".to_owned())?;
    let profile = profile_boundary_diagrams(n)?;
    let verified = known_count(n) == Some(profile.count);
    println!(
        "N={} Q(N)={} verified={} total_elapsed_s={:.6}",
        n,
        profile.count,
        verified,
        profile.total_elapsed.as_secs_f64()
    );
    println!(
        "row,support,unique_coefficients,grouped_add_nodes,grouped_zdd_nodes,interleaved_add_nodes,interleaved_zdd_nodes,grouped_add_s,grouped_zdd_s,interleaved_add_s,interleaved_zdd_s,peak_rss_bytes"
    );
    for layer in profile.layers {
        println!(
            "{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{:.9},{}",
            layer.row + 1,
            layer.support,
            layer.unique_coefficients,
            layer.grouped_add_nodes,
            layer.grouped_zdd_nodes,
            layer.interleaved_add_nodes,
            layer.interleaved_zdd_nodes,
            layer.grouped_add_s,
            layer.grouped_zdd_s,
            layer.interleaved_add_s,
            layer.interleaved_zdd_s,
            layer.peak_rss_bytes
        );
    }
    if known_count(n).is_some() && !verified {
        return Err("known-count verification failed".to_owned());
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

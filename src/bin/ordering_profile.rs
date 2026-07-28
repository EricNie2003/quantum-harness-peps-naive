use nqueens_peps_naive::{
    known_count,
    ordering_oracle::{SiteOrdering, profile_ordering},
};
use std::env;
use std::process::ExitCode;

fn run() -> Result<(), String> {
    let n: usize = env::args()
        .nth(1)
        .ok_or_else(|| "usage: ordering_profile N".to_owned())?
        .parse()
        .map_err(|_| "N must be a non-negative integer".to_owned())?;
    println!(
        "N,ordering,count,verified,elapsed_s,peak_rss_bytes,peak_support,peak_frontier_variables,candidate_pairs,matched_pairs"
    );
    for ordering in [
        SiteOrdering::RowMajor,
        SiteOrdering::Snake,
        SiteOrdering::DiagonalWavefront,
    ] {
        let profile = profile_ordering(n, ordering)?;
        let verified = known_count(n) == Some(profile.count);
        println!(
            "{},{},{},{},{:.9},{},{},{},{},{}",
            n,
            ordering.name(),
            profile.count,
            verified,
            profile.elapsed.as_secs_f64(),
            profile.peak_rss_bytes,
            profile.peak_support,
            profile.peak_frontier_variables,
            profile.candidate_pairs,
            profile.matched_pairs
        );
        if known_count(n).is_some() && !verified {
            return Err(format!(
                "known-count verification failed for ordering {}",
                ordering.name()
            ));
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

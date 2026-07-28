use nqueens_peps_naive::exact_mps::{FIELD_PRIME, contract_exact_mps};
use nqueens_peps_naive::known_count;
use std::env;
use std::process::ExitCode;

fn parse_usize(value: Option<String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let min_n = parse_usize(args.next(), "MIN_N")?;
    let max_n = parse_usize(args.next(), "MAX_N")?;
    if args.next().is_some() || min_n > max_n {
        return Err("usage: e16_exact_mps MIN_N MAX_N".to_owned());
    }
    println!(
        "N,count_mod_prime,known_count_mod_prime,verified,elapsed_s,peak_rss_bytes,peak_mpo_bond_rank,peak_shifted_bond_rank,tensor_entries_examined,tensor_entries_matched,prime"
    );
    for n in min_n..=max_n {
        let result = contract_exact_mps(n)?;
        let expected = known_count(n).map(|count| (count % u128::from(FIELD_PRIME)) as u64);
        let verified = expected == Some(result.count_mod_prime);
        if expected.is_some() && !verified {
            return Err(format!("known-count mismatch at N={n}"));
        }
        println!(
            "{n},{},{},{verified},{:.9},{},{},{},{},{},{}",
            result.count_mod_prime,
            expected.map_or_else(String::new, |value| value.to_string()),
            result.elapsed.as_secs_f64(),
            result.peak_rss_bytes,
            result
                .layers
                .iter()
                .map(|layer| layer.max_bond_rank_after_mpo)
                .max()
                .unwrap_or(1),
            result
                .layers
                .iter()
                .map(|layer| layer.max_bond_rank_after_shift)
                .max()
                .unwrap_or(1),
            result.tensor_entries_examined,
            result.tensor_entries_accepted,
            FIELD_PRIME
        );
        for layer in result.layers {
            eprintln!(
                "layer,N={n},row={},mpo_rank={},shift_rank={}",
                layer.row + 1,
                layer.max_bond_rank_after_mpo,
                layer.max_bond_rank_after_shift
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

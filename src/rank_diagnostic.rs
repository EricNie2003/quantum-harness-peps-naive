//! Exact finite-field flattening-rank diagnostic for PEPS boundary tensors.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    BoundaryState, CompiledRowOperator, PackedBoundary, RowCounters, SiteTensorC,
    apply_top_row_symmetry, contract_one_row_compiled, peak_rss_bytes,
};

pub const RANK_PRIMES: [u64; 2] = [1_000_000_007, 1_000_000_009];

#[derive(Clone, Debug)]
pub struct RankDiagnosticResult {
    pub n: usize,
    pub selected_row: usize,
    pub support: usize,
    pub left_patterns: usize,
    pub right_patterns: usize,
    pub ranks: [usize; 2],
    pub peak_elimination_row_nnz: [usize; 2],
    pub left_factor_nnz: [usize; 2],
    pub right_factor_nnz: [usize; 2],
    pub reconstruction_products: [u128; 2],
    pub row_operator_candidates: u128,
    pub row_operator_matched: u128,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SparseFactorMetrics {
    rank: usize,
    peak_row_nnz: usize,
    left_factor_nnz: usize,
    right_factor_nnz: usize,
    reconstruction_products: u128,
}

fn mod_pow(mut base: u64, mut exponent: u64, prime: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = ((u128::from(result) * u128::from(base)) % u128::from(prime)) as u64;
        }
        base = ((u128::from(base) * u128::from(base)) % u128::from(prime)) as u64;
        exponent >>= 1;
    }
    result
}

fn sparse_factor_metrics(
    entries: &[(u64, u64, u128)],
    prime: u64,
) -> Result<SparseFactorMetrics, String> {
    let mut rows = HashMap::<u64, HashMap<u64, u64>>::new();
    for &(row_key, column_key, coefficient) in entries {
        let value = (coefficient % u128::from(prime)) as u64;
        if value == 0 {
            continue;
        }
        let matrix_entry = rows
            .entry(row_key)
            .or_default()
            .entry(column_key)
            .or_insert(0);
        *matrix_entry = (*matrix_entry + value) % prime;
        if *matrix_entry == 0 {
            rows.get_mut(&row_key)
                .expect("row exists")
                .remove(&column_key);
        }
    }
    let mut rows = rows.into_values().collect::<Vec<_>>();
    rows.sort_unstable_by_key(HashMap::len);
    let mut pivots = HashMap::<u64, HashMap<u64, u64>>::new();
    let mut pivot_left_counts = HashMap::<u64, usize>::new();
    let mut left_factor_nnz = 0_usize;
    let mut peak_row_nnz = rows.iter().map(HashMap::len).max().unwrap_or(0);

    for mut row in rows {
        let mut row_factor_nnz = 0_usize;
        while let Some(pivot_column) = row.keys().copied().min() {
            if let Some(pivot_row) = pivots.get(&pivot_column) {
                let factor = row[&pivot_column];
                row_factor_nnz += 1;
                *pivot_left_counts.entry(pivot_column).or_default() += 1;
                for (&column, &pivot_value) in pivot_row {
                    let subtraction =
                        ((u128::from(factor) * u128::from(pivot_value)) % u128::from(prime)) as u64;
                    let old = row.get(&column).copied().unwrap_or(0);
                    let updated = (old + prime - subtraction) % prime;
                    if updated == 0 {
                        row.remove(&column);
                    } else {
                        row.insert(column, updated);
                    }
                }
                peak_row_nnz = peak_row_nnz.max(row.len());
            } else {
                let scale = row[&pivot_column];
                let inverse = mod_pow(row[&pivot_column], prime - 2, prime);
                for value in row.values_mut() {
                    *value =
                        ((u128::from(*value) * u128::from(inverse)) % u128::from(prime)) as u64;
                }
                peak_row_nnz = peak_row_nnz.max(row.len());
                row_factor_nnz += usize::from(scale != 0);
                *pivot_left_counts.entry(pivot_column).or_default() += usize::from(scale != 0);
                pivots.insert(pivot_column, row);
                break;
            }
        }
        left_factor_nnz += row_factor_nnz;
    }
    let right_factor_nnz = pivots.values().map(HashMap::len).sum();
    let reconstruction_products = pivots
        .iter()
        .map(|(pivot, row)| {
            (*pivot_left_counts.get(pivot).unwrap_or(&0) as u128) * row.len() as u128
        })
        .sum();
    Ok(SparseFactorMetrics {
        rank: pivots.len(),
        peak_row_nnz,
        left_factor_nnz,
        right_factor_nnz,
        reconstruction_products,
    })
}

#[cfg(test)]
fn sparse_rank(entries: &[(u64, u64, u128)], prime: u64) -> Result<(usize, usize), String> {
    let metrics = sparse_factor_metrics(entries, prime)?;
    Ok((metrics.rank, metrics.peak_row_nnz))
}

fn spatial_flatten_key(state: BoundaryState, n: usize) -> (u64, u64) {
    let left_width = n / 2;
    let right_width = n - left_width;
    let mut left = 0_u64;
    let mut right = 0_u64;
    for (family, mask) in [state.columns, state.diag_dr, state.diag_dl]
        .into_iter()
        .enumerate()
    {
        for column in 0..left_width {
            left |= ((mask >> column) & 1) << (family * left_width + column);
        }
        for column in left_width..n {
            right |= ((mask >> column) & 1) << (family * right_width + column - left_width);
        }
    }
    (left, right)
}

pub fn diagnose_peak_layer_rank(n: usize) -> Result<RankDiagnosticResult, String> {
    if n == 0 || n > 21 {
        return Err("rank diagnostic supports 1 <= N <= 21".to_owned());
    }
    let start = Instant::now();
    let operator = CompiledRowOperator::compile(&SiteTensorC::sec_vi())?;
    let initial = PackedBoundary::pack(
        BoundaryState {
            columns: 0,
            diag_dr: 0,
            diag_dl: 0,
        },
        n,
    );
    let mut boundary = vec![(initial, 1_u128)];
    let mut selected_row = 0;
    let mut selected_boundary = boundary.clone();
    let mut total_candidates = 0_u128;
    let mut total_matched = 0_u128;

    for row in 0..n {
        let mut counters = RowCounters::default();
        let mut candidates = Vec::<(PackedBoundary, u128)>::new();
        for &(parent, parent_weight) in &boundary {
            let mut terms = contract_one_row_compiled(
                n,
                &operator,
                parent.unpack(n),
                parent_weight,
                &mut counters,
            )?;
            if row == 0 {
                terms = apply_top_row_symmetry(n, terms)?;
            }
            candidates.extend(
                terms
                    .into_iter()
                    .map(|(state, weight)| (PackedBoundary::pack(state, n), weight)),
            );
        }
        candidates.sort_unstable_by_key(|(state, _)| state.0);
        let mut write = 0_usize;
        for read in 0..candidates.len() {
            let (state, weight) = candidates[read];
            if write > 0 && candidates[write - 1].0 == state {
                candidates[write - 1].1 = candidates[write - 1]
                    .1
                    .checked_add(weight)
                    .ok_or_else(|| "rank diagnostic coefficient overflow".to_owned())?;
            } else {
                candidates[write] = (state, weight);
                write += 1;
            }
        }
        candidates.truncate(write);
        boundary = candidates;
        total_candidates += counters.operator_candidates;
        total_matched += counters.operator_matched;
        if boundary.len() > selected_boundary.len() {
            selected_row = row + 1;
            selected_boundary = boundary.clone();
        }
    }

    let entries = selected_boundary
        .iter()
        .map(|&(packed, coefficient)| {
            let (left, right) = spatial_flatten_key(packed.unpack(n), n);
            (left, right, coefficient)
        })
        .collect::<Vec<_>>();
    let left_patterns = entries
        .iter()
        .map(|&(left, _, _)| left)
        .collect::<std::collections::HashSet<_>>()
        .len();
    let right_patterns = entries
        .iter()
        .map(|&(_, right, _)| right)
        .collect::<std::collections::HashSet<_>>()
        .len();
    let mut ranks = [0_usize; 2];
    let mut peak_elimination_row_nnz = [0_usize; 2];
    let mut left_factor_nnz = [0_usize; 2];
    let mut right_factor_nnz = [0_usize; 2];
    let mut reconstruction_products = [0_u128; 2];
    for (index, prime) in RANK_PRIMES.into_iter().enumerate() {
        let metrics = sparse_factor_metrics(&entries, prime)?;
        ranks[index] = metrics.rank;
        peak_elimination_row_nnz[index] = metrics.peak_row_nnz;
        left_factor_nnz[index] = metrics.left_factor_nnz;
        right_factor_nnz[index] = metrics.right_factor_nnz;
        reconstruction_products[index] = metrics.reconstruction_products;
    }

    Ok(RankDiagnosticResult {
        n,
        selected_row,
        support: selected_boundary.len(),
        left_patterns,
        right_patterns,
        ranks,
        peak_elimination_row_nnz,
        left_factor_nnz,
        right_factor_nnz,
        reconstruction_products,
        row_operator_candidates: total_candidates,
        row_operator_matched: total_matched,
        elapsed: start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::{RANK_PRIMES, diagnose_peak_layer_rank, sparse_rank};

    #[test]
    fn sparse_elimination_finds_known_ranks_over_both_primes() {
        let full_rank = [(0, 0, 1), (0, 1, 2), (1, 0, 3), (1, 1, 5)];
        let rank_one = [(0, 0, 1), (0, 1, 2), (1, 0, 2), (1, 1, 4)];
        for prime in RANK_PRIMES {
            assert_eq!(sparse_rank(&full_rank, prime).unwrap().0, 2);
            assert_eq!(sparse_rank(&rank_one, prime).unwrap().0, 1);
        }
    }

    #[test]
    fn boundary_flattening_rank_is_field_stable_through_n7() {
        for n in 1..=7 {
            let result = diagnose_peak_layer_rank(n).unwrap();
            assert_eq!(result.ranks[0], result.ranks[1], "N={n}");
            assert!(result.ranks[0] <= result.support);
        }
    }
}

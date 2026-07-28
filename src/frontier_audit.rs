//! Deterministically certified distinguishability audit for row-frontier PEPS states.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    BoundaryState, CompiledRowOperator, PackedBoundary, RowCounters, SiteTensorC,
    apply_top_row_symmetry, contract_one_row_compiled, peak_rss_bytes,
};

pub const AUDIT_PRIMES: [u64; 2] = [18_446_744_073_709_551_557, 18_446_744_073_709_551_533];

#[derive(Clone, Debug)]
pub struct AuditLayerMetric {
    pub row: usize,
    pub reachable_states: usize,
    pub certified_classes: usize,
}

#[derive(Clone, Debug)]
pub struct FrontierAuditResult {
    pub n: usize,
    pub count: u128,
    pub certified: bool,
    pub peak_reachable_states: usize,
    pub peak_certified_classes: usize,
    pub forward_transitions: u128,
    pub signature_transitions: u128,
    pub witness_replay_transitions: u128,
    pub quotient_edges: usize,
    pub exact_signature_comparisons: u128,
    pub fingerprint_collision_witnesses: u128,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
    pub layers: Vec<AuditLayerMetric>,
}

fn successors(
    n: usize,
    row: usize,
    operator: &CompiledRowOperator,
    state: PackedBoundary,
) -> Result<Vec<(PackedBoundary, u128)>, String> {
    let mut counters = RowCounters::default();
    let mut terms = contract_one_row_compiled(n, operator, state.unpack(n), 1, &mut counters)?;
    if row == 0 {
        terms = apply_top_row_symmetry(n, terms)?;
    }
    Ok(terms
        .into_iter()
        .map(|(successor, weight)| (PackedBoundary::pack(successor, n), weight))
        .collect())
}

fn modular_fingerprint(signature: &[(usize, u128)], prime: u64) -> u64 {
    const BASE: u128 = 1_000_003;
    let modulus = u128::from(prime);
    let mut hash = 97_u128;
    for &(target, weight) in signature {
        hash = (hash * BASE + target as u128 + 1) % modulus;
        hash = (hash * BASE + u128::from(weight as u64) + 1) % modulus;
        hash = (hash * BASE + (weight >> 64) + 1) % modulus;
    }
    ((hash * BASE + signature.len() as u128 + 1) % modulus) as u64
}

fn fingerprints(signature: &[(usize, u128)]) -> [u64; 2] {
    AUDIT_PRIMES.map(|prime| modular_fingerprint(signature, prime))
}

fn exact_signature(
    n: usize,
    row: usize,
    operator: &CompiledRowOperator,
    state: PackedBoundary,
    next_classes: &HashMap<PackedBoundary, usize>,
    transition_counter: &mut u128,
) -> Result<Vec<(usize, u128)>, String> {
    let mut targets = HashMap::<usize, u128>::new();
    for (successor, weight) in successors(n, row, operator, state)? {
        *transition_counter = transition_counter
            .checked_add(1)
            .ok_or_else(|| "frontier-audit transition counter overflow".to_owned())?;
        let target = *next_classes
            .get(&successor)
            .ok_or_else(|| format!("row {} successor is not forward reachable", row + 1))?;
        let accumulated = targets.entry(target).or_insert(0);
        *accumulated = accumulated
            .checked_add(weight)
            .ok_or_else(|| "frontier-audit signature multiplicity overflow".to_owned())?;
    }
    let mut signature = targets.into_iter().collect::<Vec<_>>();
    signature.sort_unstable_by_key(|&(target, _)| target);
    Ok(signature)
}

pub fn audit_frontier_distinguishability(n: usize) -> Result<FrontierAuditResult, String> {
    if n > 42 {
        return Err("frontier audit uses the packed u128 N<=42 row boundary".to_owned());
    }
    if n == 0 {
        return Ok(FrontierAuditResult {
            n,
            count: 1,
            certified: true,
            peak_reachable_states: 1,
            peak_certified_classes: 1,
            forward_transitions: 0,
            signature_transitions: 0,
            witness_replay_transitions: 0,
            quotient_edges: 0,
            exact_signature_comparisons: 0,
            fingerprint_collision_witnesses: 0,
            elapsed: Duration::ZERO,
            peak_rss_bytes: peak_rss_bytes(),
            layers: vec![AuditLayerMetric {
                row: 0,
                reachable_states: 1,
                certified_classes: 1,
            }],
        });
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
    let mut reachable_layers = Vec::<Vec<PackedBoundary>>::with_capacity(n + 1);
    reachable_layers.push(vec![initial]);
    let mut forward_transitions = 0_u128;
    for row in 0..n {
        let mut next = Vec::<PackedBoundary>::new();
        for &state in &reachable_layers[row] {
            let terms = successors(n, row, &operator, state)?;
            forward_transitions = forward_transitions
                .checked_add(terms.len() as u128)
                .ok_or_else(|| "frontier-audit forward counter overflow".to_owned())?;
            next.extend(terms.into_iter().map(|(successor, _)| successor));
        }
        next.sort_unstable_by_key(|state| state.0);
        next.dedup();
        reachable_layers.push(next);
    }

    let mut class_maps = (0..=n)
        .map(|_| HashMap::<PackedBoundary, usize>::new())
        .collect::<Vec<_>>();
    let mut class_signatures = (0..=n)
        .map(|_| Vec::<Vec<(usize, u128)>>::new())
        .collect::<Vec<_>>();
    let mut class_values = (0..=n).map(|_| Vec::<u128>::new()).collect::<Vec<_>>();
    let board_mask = (1_u64 << n) - 1;
    let mut terminal_classes = HashMap::<bool, usize>::new();
    for &state in &reachable_layers[n] {
        let accepted = state.columns(n) == board_mask;
        let next_class = terminal_classes.len();
        let class = *terminal_classes.entry(accepted).or_insert(next_class);
        class_maps[n].insert(state, class);
    }
    class_values[n] = vec![0; terminal_classes.len()];
    for (accepted, class) in terminal_classes {
        class_values[n][class] = u128::from(accepted);
    }

    let mut signature_transitions = 0_u128;
    let mut exact_signature_comparisons = 0_u128;
    let mut fingerprint_collision_witnesses = 0_u128;
    for row in (0..n).rev() {
        let mut fingerprint_buckets = HashMap::<[u64; 2], Vec<usize>>::new();
        let mut signatures = Vec::<Vec<(usize, u128)>>::new();
        for &state in &reachable_layers[row] {
            let signature = exact_signature(
                n,
                row,
                &operator,
                state,
                &class_maps[row + 1],
                &mut signature_transitions,
            )?;
            let fingerprint = fingerprints(&signature);
            let candidates = fingerprint_buckets.entry(fingerprint).or_default();
            let mut matching_class = None;
            for &class in candidates.iter() {
                exact_signature_comparisons = exact_signature_comparisons
                    .checked_add(1)
                    .ok_or_else(|| "frontier-audit comparison counter overflow".to_owned())?;
                if signatures[class] == signature {
                    matching_class = Some(class);
                    break;
                }
                fingerprint_collision_witnesses = fingerprint_collision_witnesses
                    .checked_add(1)
                    .ok_or_else(|| "frontier-audit witness counter overflow".to_owned())?;
            }
            let class = matching_class.unwrap_or_else(|| {
                let class = signatures.len();
                signatures.push(signature);
                candidates.push(class);
                class
            });
            class_maps[row].insert(state, class);
        }
        class_values[row] = signatures
            .iter()
            .map(|signature| {
                signature.iter().try_fold(0_u128, |sum, &(target, weight)| {
                    let contribution = weight
                        .checked_mul(class_values[row + 1][target])
                        .ok_or_else(|| "frontier-audit class multiplication overflow".to_owned())?;
                    sum.checked_add(contribution)
                        .ok_or_else(|| "frontier-audit class sum overflow".to_owned())
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        class_signatures[row] = signatures;
    }

    let initial_class = class_maps[0][&initial];
    let count = class_values[0][initial_class];

    // Deterministic certificate replay: regenerate every concrete signature,
    // compare it with its stored exact class witness, and verify both modular
    // fingerprints. Hash equality is never accepted as equivalence.
    let mut witness_replay_transitions = 0_u128;
    for row in (0..n).rev() {
        for &state in &reachable_layers[row] {
            let replayed = exact_signature(
                n,
                row,
                &operator,
                state,
                &class_maps[row + 1],
                &mut witness_replay_transitions,
            )?;
            let class = class_maps[row][&state];
            if replayed != class_signatures[row][class] {
                return Err(format!(
                    "deterministic signature witness mismatch at row {row}"
                ));
            }
            if fingerprints(&replayed) != fingerprints(&class_signatures[row][class]) {
                return Err(format!(
                    "two-prime fingerprint replay mismatch at row {row}"
                ));
            }
        }
    }
    let mut replay_values = (0..=n).map(|_| Vec::<u128>::new()).collect::<Vec<_>>();
    replay_values[n] = class_values[n].clone();
    for row in (0..n).rev() {
        replay_values[row] = class_signatures[row]
            .iter()
            .map(|signature| {
                signature.iter().try_fold(0_u128, |sum, &(target, weight)| {
                    let contribution = weight
                        .checked_mul(replay_values[row + 1][target])
                        .ok_or_else(|| {
                            "frontier-audit replay multiplication overflow".to_owned()
                        })?;
                    sum.checked_add(contribution)
                        .ok_or_else(|| "frontier-audit replay sum overflow".to_owned())
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
    }
    if replay_values[0][initial_class] != count {
        return Err("frontier-audit quotient value replay mismatch".to_owned());
    }

    let layers = (0..=n)
        .map(|row| AuditLayerMetric {
            row,
            reachable_states: reachable_layers[row].len(),
            certified_classes: class_values[row].len(),
        })
        .collect::<Vec<_>>();
    let quotient_edges = class_signatures
        .iter()
        .flat_map(|layer| layer.iter())
        .map(Vec::len)
        .sum();
    Ok(FrontierAuditResult {
        n,
        count,
        certified: true,
        peak_reachable_states: layers
            .iter()
            .map(|layer| layer.reachable_states)
            .max()
            .unwrap_or(1),
        peak_certified_classes: layers
            .iter()
            .map(|layer| layer.certified_classes)
            .max()
            .unwrap_or(1),
        forward_transitions,
        signature_transitions,
        witness_replay_transitions,
        quotient_edges,
        exact_signature_comparisons,
        fingerprint_collision_witnesses,
        elapsed: start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

#[cfg(test)]
mod tests {
    use super::{AUDIT_PRIMES, audit_frontier_distinguishability};
    use crate::known_count;

    #[test]
    fn audit_primes_are_distinct_and_large() {
        assert_ne!(AUDIT_PRIMES[0], AUDIT_PRIMES[1]);
        assert!(AUDIT_PRIMES.iter().all(|&prime| prime > u64::MAX / 2));
    }

    #[test]
    fn certified_audit_matches_known_counts_through_n9() {
        for n in 0..=9 {
            let result = audit_frontier_distinguishability(n).unwrap();
            assert!(result.certified);
            assert_eq!(result.count, known_count(n).unwrap(), "N={n}");
            assert_eq!(
                result.signature_transitions, result.witness_replay_transitions,
                "N={n}"
            );
            assert!(result.peak_certified_classes <= result.peak_reachable_states.max(2));
        }
    }
}

//! Exact small-N future-equivalence diagnostic for the production PEPS frontier.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    BoundaryState, CompiledRowOperator, PackedBoundary, RowCounters, SiteTensorC,
    apply_top_row_symmetry, contract_one_row_compiled, peak_rss_bytes,
};

#[derive(Clone, Debug)]
pub struct QuotientLayerMetric {
    pub row: usize,
    pub reachable_states: usize,
    pub future_classes: usize,
}

#[derive(Clone, Debug)]
pub struct FutureQuotientResult {
    pub n: usize,
    pub count: u128,
    pub peak_reachable_states: usize,
    pub peak_future_classes: usize,
    pub forward_transitions: u128,
    pub backward_signature_transitions: u128,
    pub quotient_edges: usize,
    pub quotient_replay_elapsed: Duration,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
    pub layers: Vec<QuotientLayerMetric>,
}

fn successors(
    n: usize,
    row: usize,
    operator: &CompiledRowOperator,
    state: PackedBoundary,
    apply_d4_top_slice: bool,
) -> Result<Vec<(PackedBoundary, u128)>, String> {
    let mut counters = RowCounters::default();
    let mut terms = contract_one_row_compiled(n, operator, state.unpack(n), 1, &mut counters)?;
    if row == 0 && apply_d4_top_slice {
        terms = apply_top_row_symmetry(n, terms)?;
    }
    Ok(terms
        .into_iter()
        .map(|(successor, weight)| (PackedBoundary::pack(successor, n), weight))
        .collect())
}

pub fn analyze_future_equivalence(n: usize) -> Result<FutureQuotientResult, String> {
    if n > 42 {
        return Err("future quotient uses the packed u128 N<=42 frontier".to_owned());
    }
    if n == 0 {
        return Ok(FutureQuotientResult {
            n,
            count: 1,
            peak_reachable_states: 1,
            peak_future_classes: 1,
            forward_transitions: 0,
            backward_signature_transitions: 0,
            quotient_edges: 0,
            quotient_replay_elapsed: Duration::ZERO,
            elapsed: Duration::ZERO,
            peak_rss_bytes: peak_rss_bytes(),
            layers: vec![QuotientLayerMetric {
                row: 0,
                reachable_states: 1,
                future_classes: 1,
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
            let terms = successors(n, row, &operator, state, true)?;
            forward_transitions = forward_transitions
                .checked_add(terms.len() as u128)
                .ok_or_else(|| "forward transition counter overflow".to_owned())?;
            next.extend(terms.into_iter().map(|(successor, _)| successor));
        }
        next.sort_unstable_by_key(|state| state.0);
        next.dedup();
        reachable_layers.push(next);
    }

    let board_mask = (1_u64 << n) - 1;
    let mut class_maps = (0..=n)
        .map(|_| HashMap::<PackedBoundary, usize>::new())
        .collect::<Vec<_>>();
    let mut class_values = (0..=n).map(|_| Vec::<u128>::new()).collect::<Vec<_>>();
    let mut class_signatures = (0..=n)
        .map(|_| Vec::<Vec<(usize, u128)>>::new())
        .collect::<Vec<_>>();
    let mut final_classes = HashMap::<bool, usize>::new();
    for &state in &reachable_layers[n] {
        let accepted = state.columns(n) == board_mask;
        let next_id = final_classes.len();
        let class_id = *final_classes.entry(accepted).or_insert(next_id);
        class_maps[n].insert(state, class_id);
    }
    class_values[n] = vec![0; final_classes.len()];
    for (&accepted, &class_id) in &final_classes {
        class_values[n][class_id] = u128::from(accepted);
    }

    let mut backward_signature_transitions = 0_u128;
    for row in (0..n).rev() {
        let mut signature_classes = HashMap::<Vec<(usize, u128)>, usize>::new();
        let mut signatures_by_class = Vec::<Vec<(usize, u128)>>::new();
        for &state in &reachable_layers[row] {
            let mut target_weights = HashMap::<usize, u128>::new();
            for (successor, weight) in successors(n, row, &operator, state, true)? {
                backward_signature_transitions = backward_signature_transitions
                    .checked_add(1)
                    .ok_or_else(|| "backward transition counter overflow".to_owned())?;
                let target = *class_maps[row + 1]
                    .get(&successor)
                    .ok_or_else(|| format!("row {} successor is not forward reachable", row + 1))?;
                let multiplicity = target_weights.entry(target).or_insert(0);
                *multiplicity = multiplicity
                    .checked_add(weight)
                    .ok_or_else(|| "future-signature multiplicity overflow".to_owned())?;
            }
            let mut signature = target_weights.into_iter().collect::<Vec<_>>();
            signature.sort_unstable_by_key(|&(target, _)| target);
            let next_id = signature_classes.len();
            let class_id = *signature_classes
                .entry(signature.clone())
                .or_insert_with(|| {
                    signatures_by_class.push(signature);
                    next_id
                });
            class_maps[row].insert(state, class_id);
        }
        class_values[row] = signatures_by_class
            .iter()
            .map(|signature| {
                signature.iter().try_fold(0_u128, |sum, &(target, weight)| {
                    let contribution = weight
                        .checked_mul(class_values[row + 1][target])
                        .ok_or_else(|| "future-class value multiplication overflow".to_owned())?;
                    sum.checked_add(contribution)
                        .ok_or_else(|| "future-class value sum overflow".to_owned())
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        class_signatures[row] = signatures_by_class;
    }

    let initial_class = class_maps[0][&initial];
    let count = class_values[0][initial_class];
    let replay_start = Instant::now();
    let mut replay_values = (0..=n).map(|_| Vec::<u128>::new()).collect::<Vec<_>>();
    replay_values[n] = class_values[n].clone();
    for row in (0..n).rev() {
        replay_values[row] = class_signatures[row]
            .iter()
            .map(|signature| {
                signature.iter().try_fold(0_u128, |sum, &(target, weight)| {
                    let contribution = weight
                        .checked_mul(replay_values[row + 1][target])
                        .ok_or_else(|| "quotient replay multiplication overflow".to_owned())?;
                    sum.checked_add(contribution)
                        .ok_or_else(|| "quotient replay sum overflow".to_owned())
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
    }
    let quotient_replay_elapsed = replay_start.elapsed();
    if replay_values[0][initial_class] != count {
        return Err("quotient replay count mismatch".to_owned());
    }
    let quotient_edges = class_signatures
        .iter()
        .flat_map(|classes| classes.iter())
        .map(Vec::len)
        .sum();
    let layers = (0..=n)
        .map(|row| QuotientLayerMetric {
            row,
            reachable_states: reachable_layers[row].len(),
            future_classes: class_values[row].len(),
        })
        .collect::<Vec<_>>();
    Ok(FutureQuotientResult {
        n,
        count,
        peak_reachable_states: layers
            .iter()
            .map(|layer| layer.reachable_states)
            .max()
            .unwrap_or(1),
        peak_future_classes: layers
            .iter()
            .map(|layer| layer.future_classes)
            .max()
            .unwrap_or(1),
        forward_transitions,
        backward_signature_transitions,
        quotient_edges,
        quotient_replay_elapsed,
        elapsed: start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

#[cfg(test)]
mod tests {
    use super::analyze_future_equivalence;
    use crate::known_count;

    #[test]
    fn future_quotient_reconstructs_known_counts_through_n9() {
        for n in 0..=9 {
            let result = analyze_future_equivalence(n).unwrap();
            assert_eq!(result.count, known_count(n).unwrap(), "N={n}");
            assert!(result.peak_future_classes <= result.peak_reachable_states);
        }
    }
}

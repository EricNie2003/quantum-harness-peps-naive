//! Exact row-by-row contraction of the N-Queens constraint PEPS.
//!
//! The open boundary after a row is represented by three bit sets:
//! occupied columns and the two diagonal attack sets for the next row.
//! The hash-map value is the exact multiplicity of that boundary tensor
//! entry. No floating point arithmetic or bond truncation is used.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BoundaryState {
    pub columns: u64,
    pub diagonal_left: u64,
    pub diagonal_right: u64,
}

#[derive(Clone, Debug)]
pub struct LayerMetric {
    pub row: usize,
    pub input_states: usize,
    pub candidate_transitions: u128,
    pub accepted_transitions: u128,
    pub output_states: usize,
    pub output_weight: u128,
    pub elapsed: Duration,
}

#[derive(Clone, Debug)]
pub struct ContractionResult {
    pub n: usize,
    pub count: u128,
    pub elapsed: Duration,
    pub peak_states: usize,
    pub total_candidate_transitions: u128,
    pub total_accepted_transitions: u128,
    pub layers: Vec<LayerMetric>,
}

/// Contract one complete row at a time.
///
/// For every non-zero boundary entry, all `n` local physical choices in the
/// next row are visited. Invalid choices multiply by a zero constraint tensor
/// entry; valid choices contribute their exact integer weight to the next
/// boundary. Equal outgoing virtual boundaries are summed by the hash map.
pub fn contract_rows(n: usize) -> Result<ContractionResult, String> {
    if n > 63 {
        return Err("this packed u64 baseline supports N <= 63".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            total_candidate_transitions: 0,
            total_accepted_transitions: 0,
            layers: Vec::new(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let initial = BoundaryState {
        columns: 0,
        diagonal_left: 0,
        diagonal_right: 0,
    };
    let mut boundary = HashMap::from([(initial, 1_u128)]);
    let mut layers = Vec::with_capacity(n);
    let mut peak_states = boundary.len();
    let mut total_candidate_transitions = 0_u128;
    let mut total_accepted_transitions = 0_u128;
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.len();
        let candidate_transitions = input_states as u128 * n as u128;
        let mut accepted_transitions = 0_u128;
        let mut next = HashMap::<BoundaryState, u128>::new();

        for (state, weight) in boundary.drain() {
            let attacked = state.columns | state.diagonal_left | state.diagonal_right;
            for column in 0..n {
                let queen = 1_u64 << column;
                if attacked & queen != 0 {
                    continue;
                }
                accepted_transitions += 1;
                let successor = BoundaryState {
                    columns: state.columns | queen,
                    diagonal_left: ((state.diagonal_left | queen) << 1) & board_mask,
                    diagonal_right: (state.diagonal_right | queen) >> 1,
                };
                let entry = next.entry(successor).or_insert(0);
                *entry = entry
                    .checked_add(weight)
                    .ok_or_else(|| format!("coefficient overflow while contracting row {row}"))?;
            }
        }

        let output_weight = next.values().copied().sum();
        peak_states = peak_states.max(next.len());
        total_candidate_transitions += candidate_transitions;
        total_accepted_transitions += accepted_transitions;
        layers.push(LayerMetric {
            row,
            input_states,
            candidate_transitions,
            accepted_transitions,
            output_states: next.len(),
            output_weight,
            elapsed: layer_start.elapsed(),
        });
        boundary = next;
    }

    let count = boundary.values().copied().sum();
    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        total_candidate_transitions,
        total_accepted_transitions,
        layers,
    })
}

pub fn known_count(n: usize) -> Option<u128> {
    const COUNTS: [u128; 17] = [
        1, 1, 0, 0, 2, 10, 4, 40, 92, 352, 724, 2_680, 14_200, 73_712, 365_596, 2_279_184,
        14_772_512,
    ];
    COUNTS.get(n).copied()
}

#[cfg(test)]
mod tests {
    use super::{contract_rows, known_count};

    #[test]
    fn matches_known_counts_through_ten() {
        for n in 0..=10 {
            let result = contract_rows(n).unwrap();
            assert_eq!(result.count, known_count(n).unwrap(), "N={n}");
        }
    }

    #[test]
    fn final_layer_weight_is_the_answer() {
        let result = contract_rows(8).unwrap();
        assert_eq!(result.layers.last().unwrap().output_weight, 92);
        assert_eq!(result.count, 92);
    }

    #[test]
    fn rejects_masks_wider_than_u64_baseline() {
        assert!(contract_rows(64).is_err());
    }
}

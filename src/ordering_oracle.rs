//! Geometry-independent exact sparse contraction oracle for small boards.
//!
//! This module contracts explicit local C factors in a requested site order.
//! It is intentionally generic and slow; its purpose is to compare actual
//! frontier support without baking the row-transfer recurrence into the code.

use crate::{SiteTensorC, peak_rss_bytes};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const MAX_ORACLE_N: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiteOrdering {
    RowMajor,
    Snake,
    DiagonalWavefront,
}

impl SiteOrdering {
    pub fn name(self) -> &'static str {
        match self {
            Self::RowMajor => "row_major",
            Self::Snake => "snake",
            Self::DiagonalWavefront => "diagonal_wavefront",
        }
    }
}

#[derive(Clone, Debug)]
pub struct OrderingStepMetric {
    pub step: usize,
    pub row: usize,
    pub column: usize,
    pub frontier_variables: usize,
    pub input_support: usize,
    pub local_support: usize,
    pub candidate_pairs: u128,
    pub matched_pairs: u128,
    pub output_support: usize,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct OrderingProfile {
    pub n: usize,
    pub ordering: SiteOrdering,
    pub count: u128,
    pub elapsed: Duration,
    pub peak_support: usize,
    pub peak_frontier_variables: usize,
    pub candidate_pairs: u128,
    pub matched_pairs: u128,
    pub peak_rss_bytes: u64,
    pub steps: Vec<OrderingStepMetric>,
}

#[derive(Clone)]
struct SiteFactor {
    row: usize,
    column: usize,
    variable_mask: u128,
    entries: Vec<(u128, u128)>,
}

#[derive(Clone)]
struct BondLayout {
    horizontal: Vec<Vec<Option<usize>>>,
    vertical: Vec<Vec<Option<usize>>>,
    diag_dr: Vec<Vec<Option<usize>>>,
    diag_dl: Vec<Vec<Option<usize>>>,
    variable_count: usize,
}

impl BondLayout {
    #[allow(clippy::needless_range_loop)]
    fn new(n: usize) -> Self {
        let mut next = 0;
        let mut horizontal = vec![vec![None; n]; n];
        let mut vertical = vec![vec![None; n]; n];
        let mut diag_dr = vec![vec![None; n]; n];
        let mut diag_dl = vec![vec![None; n]; n];

        for row in 0..n {
            for column in 0..n.saturating_sub(1) {
                horizontal[row][column] = Some(next);
                next += 1;
            }
        }
        for row in 0..n.saturating_sub(1) {
            for column in 0..n {
                vertical[row][column] = Some(next);
                next += 1;
            }
        }
        for row in 0..n.saturating_sub(1) {
            for column in 0..n.saturating_sub(1) {
                diag_dr[row][column] = Some(next);
                next += 1;
            }
        }
        for row in 0..n.saturating_sub(1) {
            for column in 1..n {
                diag_dl[row][column] = Some(next);
                next += 1;
            }
        }

        Self {
            horizontal,
            vertical,
            diag_dr,
            diag_dl,
            variable_count: next,
        }
    }
}

fn add_leg(key: &mut u128, variable_mask: &mut u128, variable: Option<usize>, value: u8) {
    if let Some(variable) = variable {
        let selected = 1_u128 << variable;
        *variable_mask |= selected;
        if value == 1 {
            *key |= selected;
        }
    }
}

fn build_site_factor(
    n: usize,
    row: usize,
    column: usize,
    layout: &BondLayout,
    tensor: &SiteTensorC,
) -> Result<SiteFactor, String> {
    let mut accumulated = HashMap::<u128, u128>::new();
    let mut factor_mask = None;

    for entry in tensor.entries() {
        let legs = entry.legs;
        if (row == 0 && legs.column_in != 0)
            || (row + 1 == n && legs.column_out != 1)
            || (column == 0 && legs.row_in != 0)
            || (column + 1 == n && legs.row_out != 1)
            || ((row == 0 || column == 0) && legs.diag_dr_in != 0)
            || ((row == 0 || column + 1 == n) && legs.diag_dl_in != 0)
        {
            continue;
        }

        let mut key = 0_u128;
        let mut variable_mask = 0_u128;
        add_leg(
            &mut key,
            &mut variable_mask,
            if row == 0 {
                None
            } else {
                layout.vertical[row - 1][column]
            },
            legs.column_in,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if row + 1 == n {
                None
            } else {
                layout.vertical[row][column]
            },
            legs.column_out,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if column == 0 {
                None
            } else {
                layout.horizontal[row][column - 1]
            },
            legs.row_in,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if column + 1 == n {
                None
            } else {
                layout.horizontal[row][column]
            },
            legs.row_out,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if row == 0 || column == 0 {
                None
            } else {
                layout.diag_dr[row - 1][column - 1]
            },
            legs.diag_dr_in,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if row + 1 == n || column + 1 == n {
                None
            } else {
                layout.diag_dr[row][column]
            },
            legs.diag_dr_out,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if row == 0 || column + 1 == n {
                None
            } else {
                layout.diag_dl[row - 1][column + 1]
            },
            legs.diag_dl_in,
        );
        add_leg(
            &mut key,
            &mut variable_mask,
            if row + 1 == n || column == 0 {
                None
            } else {
                layout.diag_dl[row][column]
            },
            legs.diag_dl_out,
        );

        if let Some(expected) = factor_mask {
            if expected != variable_mask {
                return Err("site entries disagree on virtual-leg geometry".to_owned());
            }
        } else {
            factor_mask = Some(variable_mask);
        }
        let coefficient = accumulated.entry(key).or_insert(0);
        *coefficient = coefficient
            .checked_add(entry.value)
            .ok_or_else(|| "site-factor coefficient overflow".to_owned())?;
    }

    let mut entries: Vec<_> = accumulated.into_iter().collect();
    entries.sort_unstable_by_key(|(key, _)| *key);
    Ok(SiteFactor {
        row,
        column,
        variable_mask: factor_mask.unwrap_or(0),
        entries,
    })
}

fn ordered_sites(n: usize, ordering: SiteOrdering) -> Vec<(usize, usize)> {
    let mut sites = Vec::with_capacity(n * n);
    match ordering {
        SiteOrdering::RowMajor => {
            for row in 0..n {
                for column in 0..n {
                    sites.push((row, column));
                }
            }
        }
        SiteOrdering::Snake => {
            for row in 0..n {
                if row % 2 == 0 {
                    for column in 0..n {
                        sites.push((row, column));
                    }
                } else {
                    for column in (0..n).rev() {
                        sites.push((row, column));
                    }
                }
            }
        }
        SiteOrdering::DiagonalWavefront => {
            for diagonal in 0..=(2 * n.saturating_sub(1)) {
                for row in 0..n {
                    if let Some(column) = diagonal.checked_sub(row)
                        && column < n
                    {
                        sites.push((row, column));
                    }
                }
            }
        }
    }
    sites
}

pub fn profile_ordering(n: usize, ordering: SiteOrdering) -> Result<OrderingProfile, String> {
    if n == 0 {
        return Ok(OrderingProfile {
            n,
            ordering,
            count: 1,
            elapsed: Duration::ZERO,
            peak_support: 1,
            peak_frontier_variables: 0,
            candidate_pairs: 0,
            matched_pairs: 0,
            peak_rss_bytes: peak_rss_bytes(),
            steps: Vec::new(),
        });
    }
    if n > MAX_ORACLE_N {
        return Err(format!(
            "generic direct-TN oracle is limited to N <= {MAX_ORACLE_N}"
        ));
    }

    let layout = BondLayout::new(n);
    if layout.variable_count > 128 {
        return Err("direct-TN oracle needs more than 128 internal bonds".to_owned());
    }
    let tensor = SiteTensorC::sec_vi();
    let mut factors = HashMap::new();
    for row in 0..n {
        for column in 0..n {
            factors.insert(
                (row, column),
                build_site_factor(n, row, column, &layout, &tensor)?,
            );
        }
    }
    let order = ordered_sites(n, ordering);
    if order.len() != n * n {
        return Err("ordering does not contain every site exactly once".to_owned());
    }

    let mut future_mask = factors
        .values()
        .fold(0_u128, |mask, factor| mask | factor.variable_mask);
    let mut remaining_uses = vec![0_u8; layout.variable_count];
    for factor in factors.values() {
        for (variable, uses) in remaining_uses.iter_mut().enumerate() {
            if factor.variable_mask & (1_u128 << variable) != 0 {
                *uses += 1;
            }
        }
    }

    let mut current_mask = 0_u128;
    let mut current = HashMap::from([(0_u128, 1_u128)]);
    let mut peak_support = 1;
    let mut peak_frontier_variables = 0;
    let mut total_candidates = 0_u128;
    let mut total_matched = 0_u128;
    let mut steps = Vec::with_capacity(n * n);
    let total_start = Instant::now();

    for (step, coordinates) in order.into_iter().enumerate() {
        let factor = factors
            .remove(&coordinates)
            .ok_or_else(|| "ordering contains a duplicate site".to_owned())?;
        for (variable, uses) in remaining_uses.iter_mut().enumerate() {
            if factor.variable_mask & (1_u128 << variable) != 0 {
                *uses -= 1;
                if *uses == 0 {
                    future_mask &= !(1_u128 << variable);
                }
            }
        }

        let step_start = Instant::now();
        let shared_mask = current_mask & factor.variable_mask;
        let output_mask = (current_mask | factor.variable_mask) & future_mask;
        let input_support = current.len();
        let candidate_pairs = input_support as u128 * factor.entries.len() as u128;
        let mut matched_pairs = 0_u128;
        let mut next = HashMap::<u128, u128>::new();

        for (&boundary_key, &boundary_value) in &current {
            for &(factor_key, factor_value) in &factor.entries {
                if (boundary_key ^ factor_key) & shared_mask != 0 {
                    continue;
                }
                matched_pairs += 1;
                let output_key = (boundary_key | factor_key) & output_mask;
                let product = boundary_value
                    .checked_mul(factor_value)
                    .ok_or_else(|| "direct-TN product overflow".to_owned())?;
                let coefficient = next.entry(output_key).or_insert(0);
                *coefficient = coefficient
                    .checked_add(product)
                    .ok_or_else(|| "direct-TN sum overflow".to_owned())?;
            }
        }

        current = next;
        current_mask = output_mask;
        peak_support = peak_support.max(current.len());
        peak_frontier_variables = peak_frontier_variables.max(current_mask.count_ones() as usize);
        total_candidates += candidate_pairs;
        total_matched += matched_pairs;
        steps.push(OrderingStepMetric {
            step,
            row: factor.row,
            column: factor.column,
            frontier_variables: current_mask.count_ones() as usize,
            input_support,
            local_support: factor.entries.len(),
            candidate_pairs,
            matched_pairs,
            output_support: current.len(),
            elapsed: step_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    if current_mask != 0 || current.len() > 1 {
        return Err("direct-TN contraction did not finish at a scalar".to_owned());
    }
    let count = current.get(&0).copied().unwrap_or(0);
    Ok(OrderingProfile {
        n,
        ordering,
        count,
        elapsed: total_start.elapsed(),
        peak_support,
        peak_frontier_variables,
        candidate_pairs: total_candidates,
        matched_pairs: total_matched,
        peak_rss_bytes: peak_rss_bytes(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::{SiteOrdering, ordered_sites, profile_ordering};
    use crate::known_count;
    use std::collections::HashSet;

    #[test]
    fn every_ordering_is_a_site_permutation() {
        for n in 1..=6 {
            for ordering in [
                SiteOrdering::RowMajor,
                SiteOrdering::Snake,
                SiteOrdering::DiagonalWavefront,
            ] {
                let sites = ordered_sites(n, ordering);
                assert_eq!(sites.len(), n * n);
                assert_eq!(sites.iter().copied().collect::<HashSet<_>>().len(), n * n);
            }
        }
    }

    #[test]
    fn all_orderings_match_known_counts_through_n4() {
        for n in 0..=4 {
            for ordering in [
                SiteOrdering::RowMajor,
                SiteOrdering::Snake,
                SiteOrdering::DiagonalWavefront,
            ] {
                assert_eq!(
                    profile_ordering(n, ordering).unwrap().count,
                    known_count(n).unwrap(),
                    "N={n}, ordering={}",
                    ordering.name()
                );
            }
        }
    }
}

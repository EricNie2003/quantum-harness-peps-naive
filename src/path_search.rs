//! Small-N direct sparse-tensor oracle for contraction-path diagnostics.
//!
//! This module consumes the explicit 17-entry `C` tensor. It is deliberately
//! generic and slower than the production row transfer; its purpose is to
//! measure actual nonzero support for candidate contraction trees.

use std::collections::{HashMap, HashSet};

use crate::{SiteTensorC, VirtualLegs};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathKind {
    RowBlocks,
    ColumnBlocks,
    BalancedRectangles,
    SupportAwareGreedy,
}

#[derive(Clone, Debug)]
struct SparseTensor {
    indices: Vec<u16>,
    entries: Vec<(u128, u128)>,
}

#[derive(Clone, Debug, Default)]
pub struct PathMetrics {
    pub count: u128,
    pub peak_support: usize,
    pub peak_rank: usize,
    pub local_tensor_entries_examined: u128,
    pub local_tensor_entries_accepted: u128,
    pub cartesian_pair_upper_bound: u128,
    pub matching_entry_pairs: u128,
    pub contractions: usize,
}

#[derive(Clone)]
struct Cluster {
    tensor: SparseTensor,
    sites: Vec<usize>,
}

fn horizontal_id(n: usize, row: usize, column: usize) -> u16 {
    (row * (n - 1) + column) as u16
}

fn vertical_id(n: usize, row: usize, column: usize) -> u16 {
    (n * (n - 1) + row * n + column) as u16
}

fn diag_dr_id(n: usize, row: usize, column: usize) -> u16 {
    (2 * n * (n - 1) + row * (n - 1) + column) as u16
}

fn diag_dl_id(n: usize, row: usize, column: usize) -> u16 {
    (2 * n * (n - 1) + (n - 1) * (n - 1) + row * (n - 1) + column - 1) as u16
}

fn push_leg(legs: &mut Vec<(u16, u8)>, id: u16, value: u8) {
    legs.push((id, value));
}

fn site_tensor(n: usize, row: usize, column: usize) -> Result<(SparseTensor, u128), String> {
    let tensor = SiteTensorC::sec_vi();
    let mut accumulated = HashMap::<u128, u128>::new();
    let mut canonical_indices = None::<Vec<u16>>;
    let mut accepted_entries = 0_u128;

    for entry in tensor.entries() {
        let VirtualLegs {
            column_in,
            column_out,
            row_in,
            row_out,
            diag_dr_in,
            diag_dr_out,
            diag_dl_in,
            diag_dl_out,
        } = entry.legs;

        if (row == 0 && column_in != 0)
            || (row + 1 == n && column_out != 1)
            || (column == 0 && row_in != 0)
            || (column + 1 == n && row_out != 1)
            || ((row == 0 || column == 0) && diag_dr_in != 0)
            || ((row == 0 || column + 1 == n) && diag_dl_in != 0)
        {
            continue;
        }
        accepted_entries += 1;

        let mut legs = Vec::<(u16, u8)>::with_capacity(8);
        if row > 0 {
            push_leg(&mut legs, vertical_id(n, row - 1, column), column_in);
        }
        if row + 1 < n {
            push_leg(&mut legs, vertical_id(n, row, column), column_out);
        }
        if column > 0 {
            push_leg(&mut legs, horizontal_id(n, row, column - 1), row_in);
        }
        if column + 1 < n {
            push_leg(&mut legs, horizontal_id(n, row, column), row_out);
        }
        if row > 0 && column > 0 {
            push_leg(&mut legs, diag_dr_id(n, row - 1, column - 1), diag_dr_in);
        }
        if row + 1 < n && column + 1 < n {
            push_leg(&mut legs, diag_dr_id(n, row, column), diag_dr_out);
        }
        if row > 0 && column + 1 < n {
            push_leg(&mut legs, diag_dl_id(n, row - 1, column + 1), diag_dl_in);
        }
        if row + 1 < n && column > 0 {
            push_leg(&mut legs, diag_dl_id(n, row, column), diag_dl_out);
        }
        legs.sort_unstable_by_key(|&(id, _)| id);
        let indices = legs.iter().map(|&(id, _)| id).collect::<Vec<_>>();
        if let Some(expected) = &canonical_indices {
            if expected != &indices {
                return Err("local C entries produced inconsistent virtual indices".to_owned());
            }
        } else {
            canonical_indices = Some(indices);
        }
        let key = legs
            .iter()
            .enumerate()
            .fold(0_u128, |key, (position, &(_, value))| {
                key | (u128::from(value) << position)
            });
        let coefficient = accumulated.entry(key).or_insert(0);
        *coefficient = coefficient
            .checked_add(entry.value)
            .ok_or_else(|| "local boundary contraction overflow".to_owned())?;
    }

    let mut entries = accumulated.into_iter().collect::<Vec<_>>();
    entries.sort_unstable_by_key(|&(key, _)| key);
    Ok((
        SparseTensor {
            indices: canonical_indices.unwrap_or_default(),
            entries,
        },
        accepted_entries,
    ))
}

fn project_key(key: u128, positions: &[usize]) -> u128 {
    positions
        .iter()
        .enumerate()
        .fold(0_u128, |projected, (target, &source)| {
            projected | (((key >> source) & 1) << target)
        })
}

fn contract_pair(
    left: SparseTensor,
    right: SparseTensor,
    metrics: &mut PathMetrics,
) -> Result<SparseTensor, String> {
    let right_positions = right
        .indices
        .iter()
        .enumerate()
        .map(|(position, &id)| (id, position))
        .collect::<HashMap<_, _>>();
    let shared = left
        .indices
        .iter()
        .enumerate()
        .filter_map(|(left_position, id)| {
            right_positions
                .get(id)
                .map(|&right_position| (left_position, right_position))
        })
        .collect::<Vec<_>>();
    let left_shared = shared
        .iter()
        .map(|&(position, _)| position)
        .collect::<Vec<_>>();
    let right_shared = shared
        .iter()
        .map(|&(_, position)| position)
        .collect::<Vec<_>>();
    let shared_ids = shared
        .iter()
        .map(|&(position, _)| left.indices[position])
        .collect::<HashSet<_>>();

    let mut output_indices = left
        .indices
        .iter()
        .chain(&right.indices)
        .copied()
        .filter(|id| !shared_ids.contains(id))
        .collect::<Vec<_>>();
    output_indices.sort_unstable();
    output_indices.dedup();
    if output_indices.len() > 128 {
        return Err("diagnostic sparse tensor rank exceeds u128 key capacity".to_owned());
    }
    let left_positions = left
        .indices
        .iter()
        .enumerate()
        .map(|(position, &id)| (id, position))
        .collect::<HashMap<_, _>>();
    let output_sources = output_indices
        .iter()
        .map(|id| {
            left_positions
                .get(id)
                .copied()
                .map(|position| (true, position))
                .or_else(|| {
                    right_positions
                        .get(id)
                        .copied()
                        .map(|position| (false, position))
                })
                .expect("output index belongs to one input")
        })
        .collect::<Vec<_>>();

    let mut right_buckets = HashMap::<u128, Vec<(u128, u128)>>::new();
    for &(key, value) in &right.entries {
        right_buckets
            .entry(project_key(key, &right_shared))
            .or_default()
            .push((key, value));
    }
    let mut accumulated = HashMap::<u128, u128>::new();
    metrics.cartesian_pair_upper_bound = metrics
        .cartesian_pair_upper_bound
        .checked_add((left.entries.len() as u128) * (right.entries.len() as u128))
        .ok_or_else(|| "diagnostic pair-product counter overflow".to_owned())?;

    for &(left_key, left_value) in &left.entries {
        let shared_key = project_key(left_key, &left_shared);
        let Some(bucket) = right_buckets.get(&shared_key) else {
            continue;
        };
        metrics.matching_entry_pairs = metrics
            .matching_entry_pairs
            .checked_add(bucket.len() as u128)
            .ok_or_else(|| "diagnostic match counter overflow".to_owned())?;
        for &(right_key, right_value) in bucket {
            let mut output_key = 0_u128;
            for (target, &(from_left, source)) in output_sources.iter().enumerate() {
                let source_key = if from_left { left_key } else { right_key };
                output_key |= ((source_key >> source) & 1) << target;
            }
            let product = left_value
                .checked_mul(right_value)
                .ok_or_else(|| "diagnostic tensor product overflow".to_owned())?;
            let coefficient = accumulated.entry(output_key).or_insert(0);
            *coefficient = coefficient
                .checked_add(product)
                .ok_or_else(|| "diagnostic tensor reduction overflow".to_owned())?;
        }
    }

    let mut entries = accumulated.into_iter().collect::<Vec<_>>();
    entries.sort_unstable_by_key(|&(key, _)| key);
    metrics.peak_support = metrics.peak_support.max(entries.len());
    metrics.peak_rank = metrics.peak_rank.max(output_indices.len());
    metrics.contractions += 1;
    Ok(SparseTensor {
        indices: output_indices,
        entries,
    })
}

fn contract_clusters(
    left: Cluster,
    right: Cluster,
    metrics: &mut PathMetrics,
) -> Result<Cluster, String> {
    let tensor = contract_pair(left.tensor, right.tensor, metrics)?;
    let mut sites = left.sites;
    sites.extend(right.sites);
    sites.sort_unstable();
    Ok(Cluster { tensor, sites })
}

fn contract_sequence(
    mut clusters: Vec<Cluster>,
    metrics: &mut PathMetrics,
) -> Result<Cluster, String> {
    let mut accumulator = clusters.remove(0);
    for cluster in clusters {
        accumulator = contract_clusters(accumulator, cluster, metrics)?;
    }
    Ok(accumulator)
}

fn balanced_region(
    grid: &[Vec<Cluster>],
    row_start: usize,
    row_end: usize,
    column_start: usize,
    column_end: usize,
    metrics: &mut PathMetrics,
) -> Result<Cluster, String> {
    let height = row_end - row_start;
    let width = column_end - column_start;
    if height == 1 && width == 1 {
        return Ok(grid[row_start][column_start].clone());
    }
    if width >= height {
        let middle = column_start + width / 2;
        let left = balanced_region(grid, row_start, row_end, column_start, middle, metrics)?;
        let right = balanced_region(grid, row_start, row_end, middle, column_end, metrics)?;
        contract_clusters(left, right, metrics)
    } else {
        let middle = row_start + height / 2;
        let top = balanced_region(grid, row_start, middle, column_start, column_end, metrics)?;
        let bottom = balanced_region(grid, middle, row_end, column_start, column_end, metrics)?;
        contract_clusters(top, bottom, metrics)
    }
}

fn shared_index_count(left: &SparseTensor, right: &SparseTensor) -> usize {
    let right_ids = right.indices.iter().copied().collect::<HashSet<_>>();
    left.indices
        .iter()
        .filter(|id| right_ids.contains(id))
        .count()
}

fn support_aware_greedy(
    mut clusters: Vec<Cluster>,
    metrics: &mut PathMetrics,
) -> Result<Cluster, String> {
    while clusters.len() > 1 {
        let mut best = None::<(u128, usize, usize, usize)>;
        for left in 0..clusters.len() {
            for right in left + 1..clusters.len() {
                let shared = shared_index_count(&clusters[left].tensor, &clusters[right].tensor);
                if shared == 0 && clusters.len() > 2 {
                    continue;
                }
                let output_rank = clusters[left].tensor.indices.len()
                    + clusters[right].tensor.indices.len()
                    - 2 * shared;
                let support_product = (clusters[left].tensor.entries.len() as u128)
                    * (clusters[right].tensor.entries.len() as u128);
                let estimated_support = support_product >> shared.min(127);
                let score = estimated_support.saturating_mul((output_rank + 1) as u128);
                let candidate = (score, output_rank, left, right);
                if best.is_none_or(|current| candidate < current) {
                    best = Some(candidate);
                }
            }
        }
        let (_, _, left, right) =
            best.ok_or_else(|| "greedy path search found no legal pair".to_owned())?;
        let right_cluster = clusters.swap_remove(right);
        let left_cluster = clusters.swap_remove(left);
        clusters.push(contract_clusters(left_cluster, right_cluster, metrics)?);
    }
    Ok(clusters.remove(0))
}

pub fn contract_with_path(n: usize, path: PathKind) -> Result<PathMetrics, String> {
    if n == 0 {
        return Ok(PathMetrics {
            count: 1,
            peak_support: 1,
            ..PathMetrics::default()
        });
    }
    let mut grid = Vec::with_capacity(n);
    let mut all = Vec::with_capacity(n * n);
    let mut metrics = PathMetrics::default();
    for row in 0..n {
        let mut grid_row = Vec::with_capacity(n);
        for column in 0..n {
            let (tensor, accepted_entries) = site_tensor(n, row, column)?;
            metrics.local_tensor_entries_examined += 17;
            metrics.local_tensor_entries_accepted += accepted_entries;
            metrics.peak_support = metrics.peak_support.max(tensor.entries.len());
            metrics.peak_rank = metrics.peak_rank.max(tensor.indices.len());
            let cluster = Cluster {
                tensor,
                sites: vec![row * n + column],
            };
            grid_row.push(cluster.clone());
            all.push(cluster);
        }
        grid.push(grid_row);
    }

    let final_cluster = match path {
        PathKind::RowBlocks => {
            let rows = grid
                .iter()
                .map(|row| contract_sequence(row.clone(), &mut metrics))
                .collect::<Result<Vec<_>, _>>()?;
            contract_sequence(rows, &mut metrics)?
        }
        PathKind::ColumnBlocks => {
            let columns = (0..n)
                .map(|column| {
                    contract_sequence(
                        (0..n).map(|row| grid[row][column].clone()).collect(),
                        &mut metrics,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            contract_sequence(columns, &mut metrics)?
        }
        PathKind::BalancedRectangles => balanced_region(&grid, 0, n, 0, n, &mut metrics)?,
        PathKind::SupportAwareGreedy => support_aware_greedy(all, &mut metrics)?,
    };
    if !final_cluster.tensor.indices.is_empty() {
        return Err("final diagnostic tensor retained open virtual bonds".to_owned());
    }
    metrics.count = final_cluster
        .tensor
        .entries
        .iter()
        .try_fold(0_u128, |sum, &(_, value)| {
            sum.checked_add(value)
                .ok_or_else(|| "final diagnostic count overflow".to_owned())
        })?;
    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::{PathKind, contract_with_path};
    use crate::known_count;

    #[test]
    fn direct_sparse_paths_match_known_counts_through_n4() {
        for path in [
            PathKind::RowBlocks,
            PathKind::ColumnBlocks,
            PathKind::BalancedRectangles,
            PathKind::SupportAwareGreedy,
        ] {
            for n in 0..=4 {
                assert_eq!(
                    contract_with_path(n, path).unwrap().count,
                    known_count(n).unwrap(),
                    "path={path:?}, N={n}"
                );
            }
        }
    }
}

//! Experimental streaming exact finite-field MPS contraction.
//!
//! The row MPO is generated directly from the explicit 17-entry `C`. No
//! production step materializes the full `8^N` boundary coefficient vector.

use std::time::{Duration, Instant};

use crate::{SiteTensorC, peak_rss_bytes};

pub const FIELD_PRIME: u64 = 1_000_000_007;

fn add(a: u64, b: u64) -> u64 {
    let sum = a + b;
    if sum >= FIELD_PRIME {
        sum - FIELD_PRIME
    } else {
        sum
    }
}

fn sub(a: u64, b: u64) -> u64 {
    if a >= b { a - b } else { a + FIELD_PRIME - b }
}

fn mul(a: u64, b: u64) -> u64 {
    ((u128::from(a) * u128::from(b)) % u128::from(FIELD_PRIME)) as u64
}

fn pow(mut base: u64, mut exponent: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul(result, base);
        }
        base = mul(base, base);
        exponent >>= 1;
    }
    result
}

fn inverse(value: u64) -> u64 {
    debug_assert_ne!(value, 0);
    pow(value, FIELD_PRIME - 2)
}

#[derive(Clone, Debug)]
struct Tensor3 {
    left: usize,
    physical: usize,
    right: usize,
    data: Vec<u64>,
}

impl Tensor3 {
    fn zeros(left: usize, physical: usize, right: usize) -> Self {
        Self {
            left,
            physical,
            right,
            data: vec![0; left * physical * right],
        }
    }

    fn index(&self, left: usize, physical: usize, right: usize) -> usize {
        (left * self.physical + physical) * self.right + right
    }

    fn get(&self, left: usize, physical: usize, right: usize) -> u64 {
        self.data[self.index(left, physical, right)]
    }

    fn add_at(&mut self, left: usize, physical: usize, right: usize, value: u64) {
        let index = self.index(left, physical, right);
        self.data[index] = add(self.data[index], value);
    }
}

#[derive(Clone, Debug)]
struct Mps {
    sites: Vec<Tensor3>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QubitLabel {
    Column(usize),
    DiagRight(usize),
    DiagLeft(usize),
    NewDiagRight,
    NewDiagLeft,
}

/// Return `matrix = left * right` using a deterministic exact column basis.
fn rank_factor(
    matrix: &[u64],
    rows: usize,
    columns: usize,
) -> Result<(Vec<u64>, Vec<u64>, usize), String> {
    if matrix.len() != rows * columns {
        return Err("rank-factor matrix shape mismatch".to_owned());
    }
    let mut basis = Vec::<Vec<u64>>::new();
    let mut pivots = Vec::<usize>::new();
    let mut right_rows = Vec::<Vec<u64>>::new();

    for column in 0..columns {
        let mut residual = (0..rows)
            .map(|row| matrix[row * columns + column])
            .collect::<Vec<_>>();
        let mut coefficients = vec![0_u64; basis.len()];
        for (basis_index, basis_vector) in basis.iter().enumerate() {
            let factor = residual[pivots[basis_index]];
            coefficients[basis_index] = factor;
            if factor != 0 {
                for row in 0..rows {
                    residual[row] = sub(residual[row], mul(factor, basis_vector[row]));
                }
            }
        }
        if let Some(pivot) = residual.iter().position(|&value| value != 0) {
            let scale = residual[pivot];
            let scale_inverse = inverse(scale);
            for value in &mut residual {
                *value = mul(*value, scale_inverse);
            }
            basis.push(residual);
            pivots.push(pivot);
            right_rows.push(vec![0; columns]);
            coefficients.push(scale);
        }
        for (basis_index, coefficient) in coefficients.into_iter().enumerate() {
            right_rows[basis_index][column] = coefficient;
        }
    }

    if basis.is_empty() {
        return Ok((vec![0; rows], vec![0; columns], 1));
    }
    let rank = basis.len();
    let mut left = vec![0_u64; rows * rank];
    let mut right = vec![0_u64; rank * columns];
    for basis_index in 0..rank {
        for row in 0..rows {
            left[row * rank + basis_index] = basis[basis_index][row];
        }
        right[basis_index * columns..(basis_index + 1) * columns]
            .copy_from_slice(&right_rows[basis_index]);
    }
    Ok((left, right, rank))
}

fn compress_bond(left_site: &mut Tensor3, right_site: &mut Tensor3) -> Result<(), String> {
    if left_site.right != right_site.left {
        return Err("MPS bond mismatch before compression".to_owned());
    }
    let rows = left_site.left * left_site.physical;
    let columns = left_site.right;
    let (left_factor, transfer, rank) = rank_factor(&left_site.data, rows, columns)?;
    let old_right = right_site.clone();
    let mut new_right = Tensor3::zeros(rank, old_right.physical, old_right.right);
    for new_left in 0..rank {
        for old_left in 0..old_right.left {
            let coefficient = transfer[new_left * old_right.left + old_left];
            if coefficient == 0 {
                continue;
            }
            for physical in 0..old_right.physical {
                for right in 0..old_right.right {
                    new_right.add_at(
                        new_left,
                        physical,
                        right,
                        mul(coefficient, old_right.get(old_left, physical, right)),
                    );
                }
            }
        }
    }
    left_site.right = rank;
    left_site.data = left_factor;
    *right_site = new_right;
    Ok(())
}

fn compress_all(mps: &mut Mps) -> Result<(), String> {
    for bond in 0..mps.sites.len().saturating_sub(1) {
        let (left, right) = mps.sites.split_at_mut(bond + 1);
        compress_bond(&mut left[bond], &mut right[0])?;
    }
    Ok(())
}

fn is_occupied_entry(legs: crate::VirtualLegs) -> bool {
    legs.column_in == 0
        && legs.column_out == 1
        && legs.row_in == 0
        && legs.row_out == 1
        && legs.diag_dr_in == 0
        && legs.diag_dr_out == 1
        && legs.diag_dl_in == 0
        && legs.diag_dl_out == 1
}

fn apply_row_mpo(mps: &Mps, n: usize, row: usize) -> Result<(Mps, u128, u128), String> {
    if mps.sites.len() != n || mps.sites.iter().any(|site| site.physical != 8) {
        return Err("row MPO expects N physical-dimension-8 MPS sites".to_owned());
    }
    let tensor = SiteTensorC::sec_vi();
    let mut output = Vec::with_capacity(n);
    let mut entries_examined = 0_u128;
    let mut entries_accepted = 0_u128;

    for column in 0..n {
        let input = &mps.sites[column];
        let left_states: &[u8] = if column == 0 { &[0] } else { &[0, 1] };
        let right_states: &[u8] = if column + 1 == n { &[1] } else { &[0, 1] };
        let mut site = Tensor3::zeros(
            input.left * left_states.len(),
            8,
            input.right * right_states.len(),
        );
        for entry in tensor.entries() {
            entries_examined += 1;
            let occupied = is_occupied_entry(entry.legs);
            let orbit_weight = if row == 0 && occupied {
                let mirror = n - 1 - column;
                if column > mirror {
                    continue;
                } else if column == mirror {
                    1
                } else {
                    2
                }
            } else {
                1
            };
            let Some(left_mpo) = left_states
                .iter()
                .position(|&state| state == entry.legs.row_in)
            else {
                continue;
            };
            let Some(right_mpo) = right_states
                .iter()
                .position(|&state| state == entry.legs.row_out)
            else {
                continue;
            };
            entries_accepted += 1;
            let physical_in = usize::from(entry.legs.column_in)
                | (usize::from(entry.legs.diag_dr_in) << 1)
                | (usize::from(entry.legs.diag_dl_in) << 2);
            let physical_out = usize::from(entry.legs.column_out)
                | (usize::from(entry.legs.diag_dr_out) << 1)
                | (usize::from(entry.legs.diag_dl_out) << 2);
            let local_value =
                (entry.value % u128::from(FIELD_PRIME)) as u64 * orbit_weight % FIELD_PRIME;
            for input_left in 0..input.left {
                for input_right in 0..input.right {
                    let amplitude = input.get(input_left, physical_in, input_right);
                    if amplitude == 0 {
                        continue;
                    }
                    site.add_at(
                        input_left * left_states.len() + left_mpo,
                        physical_out,
                        input_right * right_states.len() + right_mpo,
                        mul(amplitude, local_value),
                    );
                }
            }
        }
        output.push(site);
    }
    let mut result = Mps { sites: output };
    compress_all(&mut result)?;
    Ok((result, entries_examined, entries_accepted))
}

fn split_site(site: Tensor3) -> Result<[Tensor3; 3], String> {
    if site.physical != 8 {
        return Err("split_site expects physical dimension 8".to_owned());
    }
    let first_rows = site.left * 2;
    let first_columns = 4 * site.right;
    let mut first_matrix = vec![0_u64; first_rows * first_columns];
    for left in 0..site.left {
        for column_bit in 0..2 {
            for rest in 0..4 {
                let physical = column_bit | (rest << 1);
                for right in 0..site.right {
                    first_matrix
                        [(left * 2 + column_bit) * first_columns + rest * site.right + right] =
                        site.get(left, physical, right);
                }
            }
        }
    }
    let (first, remainder, rank_one) = rank_factor(&first_matrix, first_rows, first_columns)?;
    let column_site = Tensor3 {
        left: site.left,
        physical: 2,
        right: rank_one,
        data: first,
    };
    let second_rows = rank_one * 2;
    let second_columns = 2 * site.right;
    let mut second_matrix = vec![0_u64; second_rows * second_columns];
    for left in 0..rank_one {
        for diag_right in 0..2 {
            for diag_left in 0..2 {
                for right in 0..site.right {
                    second_matrix[(left * 2 + diag_right) * second_columns
                        + diag_left * site.right
                        + right] = remainder[left * first_columns
                        + (diag_right | (diag_left << 1)) * site.right
                        + right];
                }
            }
        }
    }
    let (second, third, rank_two) = rank_factor(&second_matrix, second_rows, second_columns)?;
    Ok([
        column_site,
        Tensor3 {
            left: rank_one,
            physical: 2,
            right: rank_two,
            data: second,
        },
        Tensor3 {
            left: rank_two,
            physical: 2,
            right: site.right,
            data: third,
        },
    ])
}

fn split_all(mps: Mps, n: usize) -> Result<(Mps, Vec<QubitLabel>), String> {
    let mut sites = Vec::with_capacity(3 * n);
    let mut labels = Vec::with_capacity(3 * n);
    for (column, site) in mps.sites.into_iter().enumerate() {
        sites.extend(split_site(site)?);
        labels.extend([
            QubitLabel::Column(column),
            QubitLabel::DiagRight(column),
            QubitLabel::DiagLeft(column),
        ]);
    }
    Ok((Mps { sites }, labels))
}

fn swap_adjacent(mps: &mut Mps, left_index: usize) -> Result<(), String> {
    let left_site = mps.sites[left_index].clone();
    let right_site = mps.sites[left_index + 1].clone();
    if left_site.right != right_site.left || left_site.physical != 2 || right_site.physical != 2 {
        return Err("adjacent swap expects two bonded qubit sites".to_owned());
    }
    let rows = left_site.left * 2;
    let columns = 2 * right_site.right;
    let mut matrix = vec![0_u64; rows * columns];
    for outer_left in 0..left_site.left {
        for old_left_physical in 0..2 {
            for old_right_physical in 0..2 {
                for outer_right in 0..right_site.right {
                    let mut value = 0_u64;
                    for middle in 0..left_site.right {
                        value = add(
                            value,
                            mul(
                                left_site.get(outer_left, old_left_physical, middle),
                                right_site.get(middle, old_right_physical, outer_right),
                            ),
                        );
                    }
                    matrix[(outer_left * 2 + old_right_physical) * columns
                        + old_left_physical * right_site.right
                        + outer_right] = value;
                }
            }
        }
    }
    let (left, right, rank) = rank_factor(&matrix, rows, columns)?;
    mps.sites[left_index] = Tensor3 {
        left: left_site.left,
        physical: 2,
        right: rank,
        data: left,
    };
    mps.sites[left_index + 1] = Tensor3 {
        left: rank,
        physical: 2,
        right: right_site.right,
        data: right,
    };
    Ok(())
}

fn remove_with_ones(mps: &mut Mps, index: usize) -> Result<(), String> {
    let removed = mps.sites[index].clone();
    if removed.physical != 2 {
        return Err("diagonal boundary removal expects a qubit".to_owned());
    }
    let mut matrix = vec![0_u64; removed.left * removed.right];
    for left in 0..removed.left {
        for right in 0..removed.right {
            matrix[left * removed.right + right] =
                add(removed.get(left, 0, right), removed.get(left, 1, right));
        }
    }
    if index + 1 < mps.sites.len() {
        let next = mps.sites[index + 1].clone();
        if removed.right != next.left {
            return Err("right bond mismatch while removing qubit".to_owned());
        }
        let mut merged = Tensor3::zeros(removed.left, next.physical, next.right);
        for left in 0..removed.left {
            for middle in 0..removed.right {
                let coefficient = matrix[left * removed.right + middle];
                for physical in 0..next.physical {
                    for right in 0..next.right {
                        merged.add_at(
                            left,
                            physical,
                            right,
                            mul(coefficient, next.get(middle, physical, right)),
                        );
                    }
                }
            }
        }
        mps.sites[index + 1] = merged;
        mps.sites.remove(index);
    } else if index > 0 {
        let previous = mps.sites[index - 1].clone();
        if previous.right != removed.left || removed.right != 1 {
            return Err("left bond mismatch while removing final qubit".to_owned());
        }
        let mut merged = Tensor3::zeros(previous.left, previous.physical, 1);
        for left in 0..previous.left {
            for physical in 0..previous.physical {
                let mut value = 0_u64;
                for middle in 0..previous.right {
                    value = add(
                        value,
                        mul(
                            previous.get(left, physical, middle),
                            matrix[middle * removed.right],
                        ),
                    );
                }
                merged.add_at(left, physical, 0, value);
            }
        }
        mps.sites[index - 1] = merged;
        mps.sites.remove(index);
    } else {
        return Err("cannot remove the only MPS site".to_owned());
    }
    Ok(())
}

fn append_fixed_zero(mps: &mut Mps) -> Result<(), String> {
    let bond = mps
        .sites
        .last()
        .ok_or_else(|| "cannot append to empty MPS".to_owned())?
        .right;
    if bond != 1 {
        return Err("open-boundary MPS must end in rank one".to_owned());
    }
    let mut site = Tensor3::zeros(1, 2, 1);
    site.add_at(0, 0, 0, 1);
    mps.sites.push(site);
    Ok(())
}

fn group_three(first: &Tensor3, second: &Tensor3, third: &Tensor3) -> Result<Tensor3, String> {
    if first.physical != 2
        || second.physical != 2
        || third.physical != 2
        || first.right != second.left
        || second.right != third.left
    {
        return Err("group_three expects three consecutive bonded qubits".to_owned());
    }
    let mut grouped = Tensor3::zeros(first.left, 8, third.right);
    for left in 0..first.left {
        for column in 0..2 {
            for diag_right in 0..2 {
                for diag_left in 0..2 {
                    for right in 0..third.right {
                        let mut value = 0_u64;
                        for middle_one in 0..first.right {
                            for middle_two in 0..second.right {
                                value = add(
                                    value,
                                    mul(
                                        mul(
                                            first.get(left, column, middle_one),
                                            second.get(middle_one, diag_right, middle_two),
                                        ),
                                        third.get(middle_two, diag_left, right),
                                    ),
                                );
                            }
                        }
                        grouped.add_at(
                            left,
                            column | (diag_right << 1) | (diag_left << 2),
                            right,
                            value,
                        );
                    }
                }
            }
        }
    }
    Ok(grouped)
}

fn shift_diagonals(mps: Mps, n: usize) -> Result<Mps, String> {
    let (mut qubits, mut labels) = split_all(mps, n)?;
    let mut removals = [
        labels
            .iter()
            .position(|label| *label == QubitLabel::DiagRight(n - 1))
            .expect("last down-right leg exists"),
        labels
            .iter()
            .position(|label| *label == QubitLabel::DiagLeft(0))
            .expect("first down-left leg exists"),
    ];
    removals.sort_unstable_by(|left, right| right.cmp(left));
    for index in removals {
        remove_with_ones(&mut qubits, index)?;
        labels.remove(index);
    }
    append_fixed_zero(&mut qubits)?;
    labels.push(QubitLabel::NewDiagRight);
    append_fixed_zero(&mut qubits)?;
    labels.push(QubitLabel::NewDiagLeft);

    let mut target = Vec::with_capacity(3 * n);
    for column in 0..n {
        target.push(QubitLabel::Column(column));
        target.push(if column == 0 {
            QubitLabel::NewDiagRight
        } else {
            QubitLabel::DiagRight(column - 1)
        });
        target.push(if column + 1 == n {
            QubitLabel::NewDiagLeft
        } else {
            QubitLabel::DiagLeft(column + 1)
        });
    }
    for (target_index, target_label) in target.iter().enumerate() {
        let mut current = labels
            .iter()
            .position(|label| label == target_label)
            .ok_or_else(|| "diagonal permutation lost a qubit label".to_owned())?;
        while current > target_index {
            swap_adjacent(&mut qubits, current - 1)?;
            labels.swap(current - 1, current);
            current -= 1;
        }
    }

    let mut grouped = Vec::with_capacity(n);
    for column in 0..n {
        grouped.push(group_three(
            &qubits.sites[3 * column],
            &qubits.sites[3 * column + 1],
            &qubits.sites[3 * column + 2],
        )?);
    }
    let mut result = Mps { sites: grouped };
    compress_all(&mut result)?;
    Ok(result)
}

fn final_boundary_contraction(mps: &Mps) -> Result<u64, String> {
    let mut vector = vec![1_u64];
    for site in &mps.sites {
        if vector.len() != site.left || site.physical != 8 {
            return Err("final MPS contraction shape mismatch".to_owned());
        }
        let mut next = vec![0_u64; site.right];
        for (left, &left_amplitude) in vector.iter().enumerate().take(site.left) {
            for physical in 0..8 {
                if physical & 1 == 0 {
                    continue;
                }
                for (right, right_amplitude) in next.iter_mut().enumerate().take(site.right) {
                    *right_amplitude = add(
                        *right_amplitude,
                        mul(left_amplitude, site.get(left, physical, right)),
                    );
                }
            }
        }
        vector = next;
    }
    if vector.len() != 1 {
        return Err("final MPS right boundary is not scalar".to_owned());
    }
    Ok(vector[0])
}

#[derive(Clone, Debug)]
pub struct MpsLayerMetric {
    pub row: usize,
    pub max_bond_rank_after_mpo: usize,
    pub max_bond_rank_after_shift: usize,
}

#[derive(Clone, Debug)]
pub struct ExactMpsResult {
    pub n: usize,
    pub count_mod_prime: u64,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
    pub tensor_entries_examined: u128,
    pub tensor_entries_accepted: u128,
    pub layers: Vec<MpsLayerMetric>,
}

fn max_bond_rank(mps: &Mps) -> usize {
    mps.sites.iter().map(|site| site.right).max().unwrap_or(1)
}

pub fn contract_exact_mps(n: usize) -> Result<ExactMpsResult, String> {
    if n == 0 {
        return Ok(ExactMpsResult {
            n,
            count_mod_prime: 1,
            elapsed: Duration::ZERO,
            peak_rss_bytes: peak_rss_bytes(),
            tensor_entries_examined: 0,
            tensor_entries_accepted: 0,
            layers: Vec::new(),
        });
    }
    let start = Instant::now();
    let mut boundary = Mps {
        sites: (0..n)
            .map(|_| {
                let mut site = Tensor3::zeros(1, 8, 1);
                site.add_at(0, 0, 0, 1);
                site
            })
            .collect(),
    };
    let mut layers = Vec::with_capacity(n);
    let mut count = 0_u64;
    let mut tensor_entries_examined = 0_u128;
    let mut tensor_entries_accepted = 0_u128;
    for row in 0..n {
        let (after_mpo, row_examined, row_accepted) = apply_row_mpo(&boundary, n, row)?;
        tensor_entries_examined += row_examined;
        tensor_entries_accepted += row_accepted;
        let mpo_rank = max_bond_rank(&after_mpo);
        if row + 1 == n {
            count = final_boundary_contraction(&after_mpo)?;
            layers.push(MpsLayerMetric {
                row,
                max_bond_rank_after_mpo: mpo_rank,
                max_bond_rank_after_shift: 1,
            });
        } else {
            boundary = shift_diagonals(after_mpo, n)?;
            layers.push(MpsLayerMetric {
                row,
                max_bond_rank_after_mpo: mpo_rank,
                max_bond_rank_after_shift: max_bond_rank(&boundary),
            });
        }
    }
    Ok(ExactMpsResult {
        n,
        count_mod_prime: count,
        elapsed: start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
        tensor_entries_examined,
        tensor_entries_accepted,
        layers,
    })
}

#[cfg(test)]
mod tests {
    use super::{FIELD_PRIME, contract_exact_mps, rank_factor};
    use crate::known_count;

    #[test]
    fn exact_rank_factorization_reconstructs_rectangular_matrices() {
        let matrix = vec![1, 2, 3, 2, 4, 6, 4, 5, 6];
        let (left, right, rank) = rank_factor(&matrix, 3, 3).unwrap();
        assert_eq!(rank, 2);
        for row in 0..3 {
            for column in 0..3 {
                let reconstructed = (0..rank).fold(0_u64, |sum, middle| {
                    super::add(
                        sum,
                        super::mul(left[row * rank + middle], right[middle * 3 + column]),
                    )
                });
                assert_eq!(reconstructed, matrix[row * 3 + column]);
            }
        }
    }

    #[test]
    fn streaming_exact_mps_matches_known_counts_through_n5() {
        for n in 0..=5 {
            assert_eq!(
                contract_exact_mps(n).unwrap().count_mod_prime,
                (known_count(n).unwrap() % u128::from(FIELD_PRIME)) as u64,
                "N={n}"
            );
        }
    }
}

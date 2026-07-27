//! Exact contraction of the N-Queens PEPS from Liu--Liao--Wang, Sec. VI.
//!
//! The rank-9 tensor `B` is constructed explicitly from its 17 non-zero
//! elements. Summing its physical index produces the rank-8 counting tensor
//! `C`. The solver applies sparse entries of `C` site by site and contracts a
//! complete row before moving the boundary down by one lattice spacing.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VirtualLegs {
    pub column_in: u8,
    pub column_out: u8,
    pub row_in: u8,
    pub row_out: u8,
    pub diag_dr_in: u8,
    pub diag_dr_out: u8,
    pub diag_dl_in: u8,
    pub diag_dl_out: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BEntry {
    pub alpha: u8,
    pub legs: VirtualLegs,
    pub value: u128,
}

#[derive(Clone, Debug)]
pub struct SiteTensorB {
    entries: Vec<BEntry>,
}

impl SiteTensorB {
    /// Construct Eq. (16): 16 empty pass-through entries and one occupied
    /// signal-emission entry.
    pub fn sec_vi() -> Self {
        let mut entries = Vec::with_capacity(17);
        for signals in 0_u8..16 {
            let column = signals & 1;
            let row = (signals >> 1) & 1;
            let diag_dr = (signals >> 2) & 1;
            let diag_dl = (signals >> 3) & 1;
            entries.push(BEntry {
                alpha: 0,
                legs: VirtualLegs {
                    column_in: column,
                    column_out: column,
                    row_in: row,
                    row_out: row,
                    diag_dr_in: diag_dr,
                    diag_dr_out: diag_dr,
                    diag_dl_in: diag_dl,
                    diag_dl_out: diag_dl,
                },
                value: 1,
            });
        }
        entries.push(BEntry {
            alpha: 1,
            legs: VirtualLegs {
                column_in: 0,
                column_out: 1,
                row_in: 0,
                row_out: 1,
                diag_dr_in: 0,
                diag_dr_out: 1,
                diag_dl_in: 0,
                diag_dl_out: 1,
            },
            value: 1,
        });
        Self { entries }
    }

    pub fn entries(&self) -> &[BEntry] {
        &self.entries
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CEntry {
    pub legs: VirtualLegs,
    pub value: u128,
}

#[derive(Clone, Debug)]
pub struct SiteTensorC {
    entries: Vec<CEntry>,
    entries_by_input: [Vec<CEntry>; 16],
}

impl SiteTensorC {
    /// Contract the physical leg with `(1,1)^T`, as in Eq. (18).
    pub fn from_b(tensor_b: &SiteTensorB) -> Self {
        let mut accumulated = HashMap::<VirtualLegs, u128>::new();
        for entry in tensor_b.entries() {
            let value = accumulated.entry(entry.legs).or_insert(0);
            *value = value
                .checked_add(entry.value)
                .expect("local tensor coefficient overflow");
        }
        let mut entries: Vec<_> = accumulated
            .into_iter()
            .map(|(legs, value)| CEntry { legs, value })
            .collect();
        entries.sort_by_key(|entry| legs_key(entry.legs));
        let mut entries_by_input: [Vec<CEntry>; 16] = std::array::from_fn(|_| Vec::new());
        for &entry in &entries {
            entries_by_input[input_key(
                entry.legs.column_in,
                entry.legs.row_in,
                entry.legs.diag_dr_in,
                entry.legs.diag_dl_in,
            )]
            .push(entry);
        }
        Self {
            entries,
            entries_by_input,
        }
    }

    pub fn sec_vi() -> Self {
        Self::from_b(&SiteTensorB::sec_vi())
    }

    pub fn entries(&self) -> &[CEntry] {
        &self.entries
    }

    fn matching_entries(
        &self,
        column_in: u8,
        row_in: u8,
        diag_dr_in: u8,
        diag_dl_in: u8,
    ) -> &[CEntry] {
        &self.entries_by_input[input_key(column_in, row_in, diag_dr_in, diag_dl_in)]
    }
}

fn input_key(column_in: u8, row_in: u8, diag_dr_in: u8, diag_dl_in: u8) -> usize {
    usize::from(column_in)
        | (usize::from(row_in) << 1)
        | (usize::from(diag_dr_in) << 2)
        | (usize::from(diag_dl_in) << 3)
}

fn legs_key(legs: VirtualLegs) -> u16 {
    u16::from(legs.column_in)
        | (u16::from(legs.column_out) << 1)
        | (u16::from(legs.row_in) << 2)
        | (u16::from(legs.row_out) << 3)
        | (u16::from(legs.diag_dr_in) << 4)
        | (u16::from(legs.diag_dr_out) << 5)
        | (u16::from(legs.diag_dl_in) << 6)
        | (u16::from(legs.diag_dl_out) << 7)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BoundaryState {
    /// Signals entering the next row from the column channels.
    pub columns: u64,
    /// Signals entering along diagonals directed top-left to bottom-right.
    pub diag_dr: u64,
    /// Signals entering along diagonals directed top-right to bottom-left.
    pub diag_dl: u64,
}

#[derive(Clone, Copy, Debug)]
struct PartialRow {
    columns_out: u64,
    diag_dr_out: u64,
    diag_dl_out: u64,
    row_signal: u8,
    weight: u128,
}

#[derive(Clone, Debug)]
pub struct LayerMetric {
    pub row: usize,
    pub input_states: usize,
    pub tensor_entries_examined: u128,
    pub tensor_entries_matched: u128,
    pub completed_row_terms: u128,
    pub output_states: usize,
    pub output_weight: u128,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ContractionResult {
    pub n: usize,
    pub count: u128,
    pub elapsed: Duration,
    pub peak_states: usize,
    pub tensor_entries_examined: u128,
    pub tensor_entries_matched: u128,
    pub peak_rss_bytes: u64,
    pub layers: Vec<LayerMetric>,
}

#[derive(Default)]
struct RowCounters {
    examined: u128,
    matched: u128,
}

fn bit(mask: u64, index: usize) -> u8 {
    ((mask >> index) & 1) as u8
}

fn set_bit(mask: &mut u64, index: usize, value: u8) {
    if value == 1 {
        *mask |= 1_u64 << index;
    }
}

/// Contract one row by applying the explicit sparse `C` tensor at every site.
///
/// Channel orientation:
/// - row: left -> right;
/// - column: top -> bottom;
/// - `diag_dr`: top-left -> bottom-right;
/// - `diag_dl`: top-right -> bottom-left.
///
/// Reversing the paper's drawing convention for a diagonal, if any, is an
/// isomorphic reorientation: `v0` remains at the incoming endpoint and `v2`
/// at the outgoing endpoint.
fn contract_one_row(
    n: usize,
    tensor: &SiteTensorC,
    parent: BoundaryState,
    parent_weight: u128,
    counters: &mut RowCounters,
) -> Result<Vec<(BoundaryState, u128)>, String> {
    // The left row boundary is v0=(1,0): incoming row signal is exactly zero.
    let mut partials = vec![PartialRow {
        columns_out: 0,
        diag_dr_out: 0,
        diag_dl_out: 0,
        row_signal: 0,
        weight: parent_weight,
    }];

    for column in 0..n {
        let mut next_partials = Vec::with_capacity(partials.len() + 1);
        let column_in = bit(parent.columns, column);
        let diag_dr_in = bit(parent.diag_dr, column);
        let diag_dl_in = bit(parent.diag_dl, column);

        for partial in partials {
            let matching =
                tensor.matching_entries(column_in, partial.row_signal, diag_dr_in, diag_dl_in);
            counters.examined += matching.len() as u128;
            for entry in matching {
                counters.matched += 1;
                let mut successor = PartialRow {
                    columns_out: partial.columns_out,
                    diag_dr_out: partial.diag_dr_out,
                    diag_dl_out: partial.diag_dl_out,
                    row_signal: entry.legs.row_out,
                    weight: partial
                        .weight
                        .checked_mul(entry.value)
                        .ok_or_else(|| "coefficient overflow in local C contraction".to_owned())?,
                };
                set_bit(&mut successor.columns_out, column, entry.legs.column_out);

                // A signal leaving the board is contracted with v2=(1,1), so
                // either value is accepted and no frontier bit is retained.
                if column + 1 < n {
                    set_bit(
                        &mut successor.diag_dr_out,
                        column + 1,
                        entry.legs.diag_dr_out,
                    );
                }
                if column > 0 {
                    set_bit(
                        &mut successor.diag_dl_out,
                        column - 1,
                        entry.legs.diag_dl_out,
                    );
                }
                next_partials.push(successor);
            }
        }
        partials = next_partials;
    }

    // The right row boundary is v1=(0,1): exactly one signal must emerge.
    Ok(partials
        .into_iter()
        .filter(|partial| partial.row_signal == 1)
        .map(|partial| {
            (
                BoundaryState {
                    columns: partial.columns_out,
                    diag_dr: partial.diag_dr_out,
                    diag_dl: partial.diag_dl_out,
                },
                partial.weight,
            )
        })
        .collect())
}

/// Exactly contract the rank-8 `C` network row by row.
pub fn contract_rows(n: usize) -> Result<ContractionResult, String> {
    if n > 63 {
        return Err("the packed virtual-boundary backend supports N <= 63".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    debug_assert_eq!(tensor.entries().len(), 17);
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let mut boundary = HashMap::from([(initial, 1_u128)]);
    let mut peak_states = 1;
    let mut total_examined = 0;
    let mut total_matched = 0;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.len();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0;
        let mut next = HashMap::<BoundaryState, u128>::new();

        for (parent, parent_weight) in boundary.drain() {
            for (successor, weight) in
                contract_one_row(n, &tensor, parent, parent_weight, &mut counters)?
            {
                completed_row_terms += 1;
                let coefficient = next.entry(successor).or_insert(0);
                *coefficient = coefficient
                    .checked_add(weight)
                    .ok_or_else(|| format!("coefficient overflow after row {}", row + 1))?;
            }
        }

        let output_weight = next.values().try_fold(0_u128, |sum, value| {
            sum.checked_add(*value)
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(next.len());
        total_examined += counters.examined;
        total_matched += counters.matched;
        let layer_peak_rss = peak_rss_bytes();
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.examined,
            tensor_entries_matched: counters.matched,
            completed_row_terms,
            output_states: next.len(),
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: layer_peak_rss,
        });
        boundary = next;
    }

    // Bottom column endpoints use v1=(0,1). Both diagonal families use
    // v2=(1,1), so their remaining signals are summed without restriction.
    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .filter(|(state, _)| state.columns == board_mask)
        .try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| "final coefficient sum overflow".to_owned())
        })?;

    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: total_examined,
        tensor_entries_matched: total_matched,
        peak_rss_bytes: peak_rss_bytes(),
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

#[cfg(target_os = "windows")]
pub fn peak_rss_bytes() -> u64 {
    use std::ffi::c_void;
    use std::mem::size_of;

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }

    let mut counters = ProcessMemoryCounters {
        cb: size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };
    // SAFETY: `counters` is a correctly sized writable C-compatible structure,
    // and the pseudo-handle returned by GetCurrentProcess is always valid in
    // the current process.
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    if ok == 0 {
        0
    } else {
        counters.peak_working_set_size as u64
    }
}

#[cfg(not(target_os = "windows"))]
pub fn peak_rss_bytes() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::{SiteTensorB, SiteTensorC, VirtualLegs, contract_rows, known_count};

    fn brute_force_count(n: usize) -> u128 {
        fn place(row: usize, n: usize, queens: &mut Vec<usize>, count: &mut u128) {
            if row == n {
                *count += 1;
                return;
            }
            for column in 0..n {
                let legal = queens.iter().enumerate().all(|(previous_row, &other)| {
                    other != column && row.abs_diff(previous_row) != column.abs_diff(other)
                });
                if legal {
                    queens.push(column);
                    place(row + 1, n, queens, count);
                    queens.pop();
                }
            }
        }

        if n == 0 {
            return 1;
        }
        let mut count = 0;
        place(0, n, &mut Vec::with_capacity(n), &mut count);
        count
    }

    #[test]
    fn rank_nine_b_has_the_sec_vi_seventeen_entries() {
        let tensor = SiteTensorB::sec_vi();
        assert_eq!(tensor.entries().len(), 17);
        assert_eq!(
            tensor
                .entries()
                .iter()
                .filter(|entry| entry.alpha == 0)
                .count(),
            16
        );
        assert_eq!(
            tensor
                .entries()
                .iter()
                .filter(|entry| entry.alpha == 1)
                .count(),
            1
        );
        for entry in tensor.entries().iter().filter(|entry| entry.alpha == 0) {
            let legs = entry.legs;
            assert_eq!(legs.column_in, legs.column_out);
            assert_eq!(legs.row_in, legs.row_out);
            assert_eq!(legs.diag_dr_in, legs.diag_dr_out);
            assert_eq!(legs.diag_dl_in, legs.diag_dl_out);
            assert_eq!(entry.value, 1);
        }
        let occupied = tensor
            .entries()
            .iter()
            .find(|entry| entry.alpha == 1)
            .unwrap();
        assert_eq!(
            occupied.legs,
            VirtualLegs {
                column_in: 0,
                column_out: 1,
                row_in: 0,
                row_out: 1,
                diag_dr_in: 0,
                diag_dr_out: 1,
                diag_dl_in: 0,
                diag_dl_out: 1,
            }
        );
    }

    #[test]
    fn physical_contraction_produces_rank_eight_c_with_seventeen_entries() {
        let tensor = SiteTensorC::sec_vi();
        assert_eq!(tensor.entries().len(), 17);
        assert!(tensor.entries().iter().all(|entry| entry.value == 1));
    }

    #[test]
    fn local_c_truth_table_has_empty_and_occupied_branches() {
        let tensor = SiteTensorC::sec_vi();
        assert_eq!(tensor.matching_entries(0, 0, 0, 0).len(), 2);
        assert_eq!(tensor.matching_entries(1, 0, 0, 0).len(), 1);
        assert_eq!(tensor.matching_entries(0, 1, 0, 0).len(), 1);
        assert_eq!(tensor.matching_entries(1, 1, 1, 1).len(), 1);
    }

    #[test]
    fn input_index_is_mechanically_equivalent_to_scanning_c() {
        let tensor = SiteTensorC::sec_vi();
        for signature in 0_u8..16 {
            let column_in = signature & 1;
            let row_in = (signature >> 1) & 1;
            let diag_dr_in = (signature >> 2) & 1;
            let diag_dl_in = (signature >> 3) & 1;
            let indexed = tensor.matching_entries(column_in, row_in, diag_dr_in, diag_dl_in);
            let scanned: Vec<_> = tensor
                .entries()
                .iter()
                .filter(|entry| {
                    let legs = entry.legs;
                    legs.column_in == column_in
                        && legs.row_in == row_in
                        && legs.diag_dr_in == diag_dr_in
                        && legs.diag_dl_in == diag_dl_in
                })
                .copied()
                .collect();
            assert_eq!(
                indexed,
                scanned.as_slice(),
                "input signature {signature:04b}"
            );
        }
    }

    #[test]
    fn peps_contraction_matches_known_counts_through_ten() {
        for n in 0..=10 {
            let result = contract_rows(n).unwrap();
            assert_eq!(result.count, known_count(n).unwrap(), "N={n}");
        }
    }

    #[test]
    fn peps_contraction_matches_independent_brute_force_oracle() {
        for n in 0..=9 {
            assert_eq!(
                contract_rows(n).unwrap().count,
                brute_force_count(n),
                "N={n}"
            );
        }
    }

    #[test]
    fn final_column_v1_and_diagonal_v2_boundaries_give_q8() {
        let result = contract_rows(8).unwrap();
        assert_eq!(result.count, 92);
        assert_eq!(result.layers.last().unwrap().output_weight, 92);
    }

    #[test]
    fn rejects_virtual_boundaries_wider_than_u64() {
        assert!(contract_rows(64).is_err());
    }
}

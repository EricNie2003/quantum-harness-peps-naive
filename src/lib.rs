//! Exact contraction of the N-Queens PEPS from Liu--Liao--Wang, Sec. VI.
//!
//! The rank-9 tensor `B` is constructed explicitly from its 17 non-zero
//! elements. Summing its physical index produces the rank-8 counting tensor
//! `C`. The solver applies sparse entries of `C` site by site and contracts a
//! complete row before moving the boundary down by one lattice spacing.

pub mod dfs_bitmask;
pub mod frontier_audit;
pub mod weighted_dd;

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[cfg(all(feature = "e55-regular-inline", feature = "e55-noinline"))]
compile_error!("E55 regular-inline and noinline features are mutually exclusive");

use rayon::prelude::*;

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

/// The eight automorphisms of the square board.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum D4Symmetry {
    Identity,
    Rotate90,
    Rotate180,
    Rotate270,
    ReflectVertical,
    ReflectHorizontal,
    ReflectMainDiagonal,
    ReflectAntiDiagonal,
}

impl D4Symmetry {
    pub const ALL: [Self; 8] = [
        Self::Identity,
        Self::Rotate90,
        Self::Rotate180,
        Self::Rotate270,
        Self::ReflectVertical,
        Self::ReflectHorizontal,
        Self::ReflectMainDiagonal,
        Self::ReflectAntiDiagonal,
    ];

    pub fn transform_coordinate(self, n: usize, row: usize, column: usize) -> (usize, usize) {
        debug_assert!(row < n && column < n);
        match self {
            Self::Identity => (row, column),
            Self::Rotate90 => (column, n - 1 - row),
            Self::Rotate180 => (n - 1 - row, n - 1 - column),
            Self::Rotate270 => (n - 1 - column, row),
            Self::ReflectVertical => (row, n - 1 - column),
            Self::ReflectHorizontal => (n - 1 - row, column),
            Self::ReflectMainDiagonal => (column, row),
            Self::ReflectAntiDiagonal => (n - 1 - column, n - 1 - row),
        }
    }

    /// Whether this action maps the already-contracted row prefix `[0, cut)`
    /// to itself as a set.
    pub fn stabilizes_top_row_cut(self, n: usize, cut: usize) -> bool {
        debug_assert!(cut <= n);
        (0..n).all(|row| {
            (0..n).all(|column| {
                let (mapped_row, _) = self.transform_coordinate(n, row, column);
                (row < cut) == (mapped_row < cut)
            })
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstraintFamily {
    Row,
    Column,
    DiagDownRight,
    DiagDownLeft,
}

impl D4Symmetry {
    pub fn transform_constraint_family(self, family: ConstraintFamily) -> ConstraintFamily {
        use ConstraintFamily::{Column, DiagDownLeft, DiagDownRight, Row};
        match (self, family) {
            (
                Self::Identity | Self::Rotate180 | Self::ReflectVertical | Self::ReflectHorizontal,
                Row,
            ) => Row,
            (
                Self::Identity | Self::Rotate180 | Self::ReflectVertical | Self::ReflectHorizontal,
                Column,
            ) => Column,
            (
                Self::Rotate90
                | Self::Rotate270
                | Self::ReflectMainDiagonal
                | Self::ReflectAntiDiagonal,
                Row,
            ) => Column,
            (
                Self::Rotate90
                | Self::Rotate270
                | Self::ReflectMainDiagonal
                | Self::ReflectAntiDiagonal,
                Column,
            ) => Row,
            (
                Self::Rotate90 | Self::Rotate270 | Self::ReflectVertical | Self::ReflectHorizontal,
                DiagDownRight,
            ) => DiagDownLeft,
            (
                Self::Rotate90 | Self::Rotate270 | Self::ReflectVertical | Self::ReflectHorizontal,
                DiagDownLeft,
            ) => DiagDownRight,
            (
                Self::Identity
                | Self::Rotate180
                | Self::ReflectMainDiagonal
                | Self::ReflectAntiDiagonal,
                diagonal,
            ) => diagonal,
        }
    }

    /// Permute the four directed channel pairs under this board action.
    ///
    /// Each mapped constraint line is oriented so that the transformed start
    /// endpoint still carries v0 and the transformed end carries v1/v2.
    /// Thus `in` and `out` move together; this is the allowed simultaneous
    /// reversal of a line and its boundary endpoints.
    pub fn transform_virtual_legs(self, legs: VirtualLegs) -> VirtualLegs {
        use ConstraintFamily::{Column, DiagDownLeft, DiagDownRight, Row};

        fn channel(legs: VirtualLegs, family: ConstraintFamily) -> (u8, u8) {
            match family {
                ConstraintFamily::Row => (legs.row_in, legs.row_out),
                ConstraintFamily::Column => (legs.column_in, legs.column_out),
                ConstraintFamily::DiagDownRight => (legs.diag_dr_in, legs.diag_dr_out),
                ConstraintFamily::DiagDownLeft => (legs.diag_dl_in, legs.diag_dl_out),
            }
        }

        fn set_channel(
            legs: &mut VirtualLegs,
            family: ConstraintFamily,
            (incoming, outgoing): (u8, u8),
        ) {
            match family {
                ConstraintFamily::Row => {
                    legs.row_in = incoming;
                    legs.row_out = outgoing;
                }
                ConstraintFamily::Column => {
                    legs.column_in = incoming;
                    legs.column_out = outgoing;
                }
                ConstraintFamily::DiagDownRight => {
                    legs.diag_dr_in = incoming;
                    legs.diag_dr_out = outgoing;
                }
                ConstraintFamily::DiagDownLeft => {
                    legs.diag_dl_in = incoming;
                    legs.diag_dl_out = outgoing;
                }
            }
        }

        let mut transformed = VirtualLegs {
            column_in: 0,
            column_out: 0,
            row_in: 0,
            row_out: 0,
            diag_dr_in: 0,
            diag_dr_out: 0,
            diag_dl_in: 0,
            diag_dl_out: 0,
        };
        for family in [Row, Column, DiagDownRight, DiagDownLeft] {
            set_channel(
                &mut transformed,
                self.transform_constraint_family(family),
                channel(legs, family),
            );
        }
        transformed
    }
}

/// Packed hash key for the three groups of open virtual indices.
///
/// For a board of width `n`, bits `[0,n)`, `[n,2n)`, and `[2n,3n)` store
/// column, down-right diagonal, and down-left diagonal signals respectively.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PackedBoundary(u128);

impl PackedBoundary {
    fn pack(state: BoundaryState, n: usize) -> Self {
        Self(
            u128::from(state.columns)
                | (u128::from(state.diag_dr) << n)
                | (u128::from(state.diag_dl) << (2 * n)),
        )
    }

    fn unpack(self, n: usize) -> BoundaryState {
        let mask = (1_u128 << n) - 1;
        BoundaryState {
            columns: (self.0 & mask) as u64,
            diag_dr: ((self.0 >> n) & mask) as u64,
            diag_dl: ((self.0 >> (2 * n)) & mask) as u64,
        }
    }

    fn columns(self, n: usize) -> u64 {
        let mask = (1_u128 << n) - 1;
        (self.0 & mask) as u64
    }
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
    pub row_operator_candidates: u128,
    pub row_operator_matched: u128,
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
    pub row_operator_candidates: u128,
    pub row_operator_matched: u128,
    pub peak_rss_bytes: u64,
    pub layers: Vec<LayerMetric>,
}

#[derive(Default)]
struct RowCounters {
    tensor_examined: u128,
    tensor_matched: u128,
    operator_candidates: u128,
    operator_matched: u128,
}

#[derive(Clone, Debug)]
struct CompiledRowOperator {
    occupied: CEntry,
}

impl CompiledRowOperator {
    /// Partially evaluate the horizontal row MPO from the explicit sparse C.
    ///
    /// Compilation succeeds only for the exact Sec. VI structure: one
    /// identity pass-through entry for every incoming signature and one
    /// all-channel 0->1 emission entry. If C changes, this specialization
    /// fails closed instead of silently becoming a handwritten recurrence.
    fn compile(tensor: &SiteTensorC) -> Result<Self, String> {
        let mut passthrough = [false; 16];
        let mut occupied = None;

        for &entry in tensor.entries() {
            let legs = entry.legs;
            let is_passthrough = legs.column_in == legs.column_out
                && legs.row_in == legs.row_out
                && legs.diag_dr_in == legs.diag_dr_out
                && legs.diag_dl_in == legs.diag_dl_out;
            let is_occupied = legs.column_in == 0
                && legs.column_out == 1
                && legs.row_in == 0
                && legs.row_out == 1
                && legs.diag_dr_in == 0
                && legs.diag_dr_out == 1
                && legs.diag_dl_in == 0
                && legs.diag_dl_out == 1;

            if entry.value != 1 {
                return Err("compiled row operator requires unit Sec. VI coefficients".to_owned());
            }
            if is_passthrough {
                let key = input_key(
                    legs.column_in,
                    legs.row_in,
                    legs.diag_dr_in,
                    legs.diag_dl_in,
                );
                if std::mem::replace(&mut passthrough[key], true) {
                    return Err(format!("duplicate C pass-through signature {key:04b}"));
                }
            } else if is_occupied {
                if occupied.replace(entry).is_some() {
                    return Err("multiple occupied C entries".to_owned());
                }
            } else {
                return Err("C contains an entry outside the Sec. VI row automaton".to_owned());
            }
        }

        if passthrough.iter().any(|present| !present) {
            return Err("C is missing a pass-through signature".to_owned());
        }
        Ok(Self {
            occupied: occupied.ok_or_else(|| "C is missing the occupied entry".to_owned())?,
        })
    }
}

fn bit(mask: u64, index: usize) -> u8 {
    ((mask >> index) & 1) as u8
}

fn set_bit(mask: &mut u64, index: usize, value: u8) {
    if value == 1 {
        *mask |= 1_u64 << index;
    }
}

fn replace_bit(mask: u64, index: usize, value: u8) -> u64 {
    let selected = 1_u64 << index;
    (mask & !selected) | (u64::from(value) << index)
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
fn contract_one_row_sitewise(
    n: usize,
    tensor: &SiteTensorC,
    parent: BoundaryState,
    parent_weight: u128,
    counters: &mut RowCounters,
) -> Result<Vec<(BoundaryState, u128)>, String> {
    // The left row boundary is v0=(1,0): incoming row signal is exactly zero.
    let mut partials = Vec::with_capacity(n + 1);
    partials.push(PartialRow {
        columns_out: 0,
        diag_dr_out: 0,
        diag_dl_out: 0,
        row_signal: 0,
        weight: parent_weight,
    });
    let mut next_partials = Vec::with_capacity(n + 1);

    for column in 0..n {
        next_partials.clear();
        let column_in = bit(parent.columns, column);
        let diag_dr_in = bit(parent.diag_dr, column);
        let diag_dl_in = bit(parent.diag_dl, column);

        for partial in partials.drain(..) {
            let matching =
                tensor.matching_entries(column_in, partial.row_signal, diag_dr_in, diag_dl_in);
            counters.tensor_examined += matching.len() as u128;
            for entry in matching {
                counters.tensor_matched += 1;
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
        std::mem::swap(&mut partials, &mut next_partials);
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

/// Apply the exact row transfer produced by partial evaluation of C.
///
/// The formula below is valid only because `CompiledRowOperator::compile`
/// verified every empty C entry is an identity pass-through and extracted the
/// unique occupied C entry. Geometry only shifts diagonal outgoing signals to
/// their positions on the next row.
fn contract_one_row_compiled(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    counters: &mut RowCounters,
) -> Result<Vec<(BoundaryState, u128)>, String> {
    let occupied = operator.occupied;
    let board_mask = (1_u64 << n) - 1;
    let mut outputs = Vec::with_capacity(n);

    for column in 0..n {
        counters.operator_candidates += 1;
        let legs = occupied.legs;
        if bit(parent.columns, column) != legs.column_in
            || bit(parent.diag_dr, column) != legs.diag_dr_in
            || bit(parent.diag_dl, column) != legs.diag_dl_in
        {
            continue;
        }
        counters.operator_matched += 1;

        let columns_out = replace_bit(parent.columns, column, legs.column_out);
        let diag_dr_at_sites = replace_bit(parent.diag_dr, column, legs.diag_dr_out);
        let diag_dl_at_sites = replace_bit(parent.diag_dl, column, legs.diag_dl_out);
        let weight = parent_weight
            .checked_mul(occupied.value)
            .ok_or_else(|| "coefficient overflow in compiled row operator".to_owned())?;
        outputs.push((
            BoundaryState {
                columns: columns_out,
                diag_dr: (diag_dr_at_sites << 1) & board_mask,
                diag_dl: diag_dl_at_sites >> 1,
            },
            weight,
        ));
    }
    Ok(outputs)
}

fn contract_one_row_compiled_sparse(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    counters: &mut RowCounters,
) -> Result<Vec<(BoundaryState, u128)>, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 {
        return Err(
            "sparse row iterator requires occupied row_in to match the left v0 boundary".to_owned(),
        );
    }
    let board_mask = (1_u64 << n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let mut outputs = Vec::with_capacity(positions.count_ones() as usize);
    let weight = parent_weight
        .checked_mul(occupied.value)
        .ok_or_else(|| "coefficient overflow in sparse compiled row operator".to_owned())?;

    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let columns_out = replace_bit(parent.columns, column, legs.column_out);
        let diag_dr_at_sites = replace_bit(parent.diag_dr, column, legs.diag_dr_out);
        let diag_dl_at_sites = replace_bit(parent.diag_dl, column, legs.diag_dl_out);
        outputs.push((
            BoundaryState {
                columns: columns_out,
                diag_dr: (diag_dr_at_sites << 1) & board_mask,
                diag_dl: diag_dl_at_sites >> 1,
            },
            weight,
        ));
    }
    Ok(outputs)
}

#[derive(Clone, Copy)]
enum PositionMode {
    Dense,
    Sparse,
}

fn contract_one_row_with_position_mode(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    counters: &mut RowCounters,
    position_mode: PositionMode,
) -> Result<Vec<(BoundaryState, u128)>, String> {
    match position_mode {
        PositionMode::Dense => {
            contract_one_row_compiled(n, operator, parent, parent_weight, counters)
        }
        PositionMode::Sparse => {
            contract_one_row_compiled_sparse(n, operator, parent, parent_weight, counters)
        }
    }
}

#[derive(Clone, Copy)]
enum SymmetryMode {
    None,
    TopRowVerticalOrbits,
}

fn top_row_vertical_orbit_weight(n: usize, successor: BoundaryState) -> Option<u128> {
    debug_assert_eq!(successor.columns.count_ones(), 1);
    let column = successor.columns.trailing_zeros() as usize;
    let mirror = n - 1 - column;
    if column > mirror {
        None
    } else if column == mirror {
        Some(1)
    } else {
        Some(2)
    }
}

fn apply_top_row_symmetry(
    n: usize,
    row_terms: Vec<(BoundaryState, u128)>,
) -> Result<Vec<(BoundaryState, u128)>, String> {
    row_terms
        .into_iter()
        .filter_map(|(successor, weight)| {
            top_row_vertical_orbit_weight(n, successor).map(|multiplicity| {
                weight
                    .checked_mul(multiplicity)
                    .map(|weighted| (successor, weighted))
                    .ok_or_else(|| "coefficient overflow in D4 orbit weighting".to_owned())
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum RowBackend {
    Sitewise,
    Compiled,
}

/// Exactly contract the rank-8 `C` network row by row.
pub fn contract_rows(n: usize) -> Result<ContractionResult, String> {
    contract_rows_d4_orbit_sort_reduce(n)
}

/// Retained exact HashMap materialization backend used as an implementation-
/// independent layer-materialization reference for sort-reduce tests.
pub fn contract_rows_hash_materialization(n: usize) -> Result<ContractionResult, String> {
    contract_rows_with_backend(n, RowBackend::Compiled)
}

/// Exactly contract the same compiled row operator, materializing each layer
/// as a sorted vector and reducing equal packed boundary keys in place.
pub fn contract_rows_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_sort_reduce_with_modes(n, SymmetryMode::None, PositionMode::Dense)
}

pub fn contract_rows_sparse_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_sort_reduce_with_modes(n, SymmetryMode::None, PositionMode::Sparse)
}

/// Exact row contraction sliced by the vertical-reflection orbits of the
/// occupied tensor entry on the first row.
///
/// Only `{identity, vertical reflection}` preserves an interior top-down row
/// cut. The other six D4 actions are validated separately and are not used as
/// an invalid blanket factor of eight.
pub fn contract_rows_d4_orbit_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_sort_reduce_with_modes(n, SymmetryMode::TopRowVerticalOrbits, PositionMode::Dense)
}

pub fn contract_rows_d4_sparse_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_sort_reduce_with_modes(
        n,
        SymmetryMode::TopRowVerticalOrbits,
        PositionMode::Sparse,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum D4KernelVariant {
    Arena,
    ArenaBatched,
    ArenaBatchedSparse,
    ArenaBatchedSparseParallelSort,
    ArenaBatchedRadix,
}

pub fn contract_rows_d4_arena_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_d4_optimized_kernel(n, D4KernelVariant::Arena)
}

pub fn contract_rows_d4_batched_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_d4_optimized_kernel(n, D4KernelVariant::ArenaBatched)
}

pub fn contract_rows_d4_batched_sparse_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_d4_optimized_kernel(n, D4KernelVariant::ArenaBatchedSparse)
}

pub fn contract_rows_d4_deferred_sparse_sort_reduce(n: usize) -> Result<ContractionResult, String> {
    contract_rows_d4_deferred_sparse_kernel(n)
}

pub fn contract_rows_d4_batched_sparse_parallel_sort(
    n: usize,
) -> Result<ContractionResult, String> {
    contract_rows_d4_optimized_kernel(n, D4KernelVariant::ArenaBatchedSparseParallelSort)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShardMode {
    Prefix,
    Mixed,
}

pub fn contract_rows_d4_sharded_sparse_sort_reduce(
    n: usize,
    shards: usize,
    shard_mode: ShardMode,
) -> Result<ContractionResult, String> {
    contract_rows_d4_sharded_sparse_kernel(n, shards, shard_mode)
}

pub fn contract_rows_d4_compact_sharded_sort_reduce(
    n: usize,
    shards: usize,
) -> Result<ContractionResult, String> {
    contract_rows_d4_compact_sharded_kernel(n, shards)
}

#[derive(Clone, Debug)]
pub struct ParallelGenerationResult {
    pub contraction: ContractionResult,
    pub generation_elapsed: Duration,
    pub sort_elapsed: Duration,
    pub reduce_elapsed: Duration,
    pub peak_thread_local_bytes: usize,
    pub peak_worker_partials: usize,
}

pub fn contract_rows_d4_compact_parallel_generation(
    n: usize,
    shards: usize,
) -> Result<ParallelGenerationResult, String> {
    contract_rows_d4_compact_parallel_generation_kernel(n, shards)
}

#[derive(Clone, Debug)]
pub struct U64PromotionResult {
    pub contraction: ContractionResult,
    pub used_u64_fast_path: bool,
    pub promotion_reason: Option<String>,
    pub attempted_fast_path_elapsed: Duration,
    pub generation_elapsed: Duration,
    pub sort_elapsed: Duration,
    pub reduce_elapsed: Duration,
    pub peak_thread_local_bytes: usize,
}

pub fn contract_rows_d4_compact_u64_promoting(
    n: usize,
    shards: usize,
) -> Result<U64PromotionResult, String> {
    contract_rows_d4_compact_u64_promoting_with_limit(n, shards, u64::MAX)
}

#[derive(Clone, Debug)]
pub struct JointU64Result {
    pub contraction: ContractionResult,
    pub used_joint_fast_path: bool,
    pub fallback_used_u64_fast_path: Option<bool>,
    pub promotion_reason: Option<String>,
    pub coefficient_bits: u32,
    pub max_coefficient_observed: u64,
    pub attempted_joint_elapsed: Duration,
    pub generation_elapsed: Duration,
    pub sort_elapsed: Duration,
    pub reduce_elapsed: Duration,
    pub peak_thread_local_bytes: usize,
}

pub fn contract_rows_d4_joint_u64_promoting(
    n: usize,
    shards: usize,
) -> Result<JointU64Result, String> {
    let coefficient_bits = 64_u32
        .checked_sub(
            u32::try_from(3_usize.saturating_mul(n))
                .map_err(|_| "joint-u64 boundary width does not fit u32".to_owned())?,
        )
        .ok_or_else(|| "joint-u64 packing requires N <= 21".to_owned())?;
    contract_rows_d4_joint_u64_with_limits(n, shards, coefficient_bits, u64::MAX)
}

#[derive(Clone, Debug)]
pub struct ArenaReuseResult {
    pub joint: JointU64Result,
    pub total_reused_capacity_bytes: usize,
    pub total_destination_growth_bytes: usize,
    pub peak_spare_capacity_bytes: usize,
}

pub fn contract_rows_d4_joint_u64_arena_reuse(
    n: usize,
    shards: usize,
) -> Result<ArenaReuseResult, String> {
    let coefficient_bits = 64_u32
        .checked_sub(
            u32::try_from(3_usize.saturating_mul(n))
                .map_err(|_| "joint-u64 boundary width does not fit u32".to_owned())?,
        )
        .ok_or_else(|| "joint-u64 packing requires N <= 21".to_owned())?;
    let (joint, reuse) =
        contract_rows_d4_joint_u64_with_reuse(n, shards, coefficient_bits, u64::MAX, true)?;
    Ok(ArenaReuseResult {
        joint,
        total_reused_capacity_bytes: reuse.total_reused_capacity_bytes,
        total_destination_growth_bytes: reuse.total_destination_growth_bytes,
        peak_spare_capacity_bytes: reuse.peak_spare_capacity_bytes,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitFrontierOrder {
    RowMajor,
    TopLeftDiamond,
}

#[derive(Clone, Debug)]
pub struct ExplicitFrontierResult {
    pub n: usize,
    pub order: ExplicitFrontierOrder,
    pub complete: bool,
    pub count: Option<u128>,
    pub elapsed: Duration,
    pub peak_states: usize,
    pub peak_open_bonds: usize,
    pub tensor_entries_examined: u128,
    pub tensor_entries_accepted: u128,
    pub contracted_sites: usize,
    pub peak_rss_bytes: u64,
}

pub fn contract_explicit_c_frontier(
    n: usize,
    order: ExplicitFrontierOrder,
    support_limit: usize,
) -> Result<ExplicitFrontierResult, String> {
    contract_explicit_c_frontier_kernel(n, order, support_limit)
}

pub fn contract_rows_d4_batched_radix(n: usize) -> Result<ContractionResult, String> {
    contract_rows_d4_optimized_kernel(n, D4KernelVariant::ArenaBatchedRadix)
}

fn append_compiled_dense_d4(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    top_row: bool,
    counters: &mut RowCounters,
    output: &mut Vec<(PackedBoundary, u128)>,
) -> Result<usize, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    let board_mask = (1_u64 << n) - 1;
    let start_len = output.len();
    for column in 0..n {
        counters.operator_candidates += 1;
        if bit(parent.columns, column) != legs.column_in
            || bit(parent.diag_dr, column) != legs.diag_dr_in
            || bit(parent.diag_dl, column) != legs.diag_dl_in
        {
            continue;
        }
        counters.operator_matched += 1;
        let columns_out = replace_bit(parent.columns, column, legs.column_out);
        let diag_dr_at_sites = replace_bit(parent.diag_dr, column, legs.diag_dr_out);
        let diag_dl_at_sites = replace_bit(parent.diag_dl, column, legs.diag_dl_out);
        let successor = BoundaryState {
            columns: columns_out,
            diag_dr: (diag_dr_at_sites << 1) & board_mask,
            diag_dl: diag_dl_at_sites >> 1,
        };
        let mut weight = parent_weight
            .checked_mul(occupied.value)
            .ok_or_else(|| "coefficient overflow in batched row operator".to_owned())?;
        if top_row {
            let Some(multiplicity) = top_row_vertical_orbit_weight(n, successor) else {
                continue;
            };
            weight = weight
                .checked_mul(multiplicity)
                .ok_or_else(|| "coefficient overflow in batched D4 weighting".to_owned())?;
        }
        output.push((PackedBoundary::pack(successor, n), weight));
    }
    Ok(output.len() - start_len)
}

fn append_compiled_sparse_d4(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    top_row: bool,
    counters: &mut RowCounters,
    output: &mut Vec<(PackedBoundary, u128)>,
) -> Result<usize, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 {
        return Err(
            "sparse batched iterator requires occupied row_in to match the left v0 boundary"
                .to_owned(),
        );
    }
    let board_mask = (1_u64 << n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let base_weight = parent_weight
        .checked_mul(occupied.value)
        .ok_or_else(|| "coefficient overflow in sparse batched row operator".to_owned())?;
    let start_len = output.len();

    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let columns_out = replace_bit(parent.columns, column, legs.column_out);
        let diag_dr_at_sites = replace_bit(parent.diag_dr, column, legs.diag_dr_out);
        let diag_dl_at_sites = replace_bit(parent.diag_dl, column, legs.diag_dl_out);
        let successor = BoundaryState {
            columns: columns_out,
            diag_dr: (diag_dr_at_sites << 1) & board_mask,
            diag_dl: diag_dl_at_sites >> 1,
        };
        let mut weight = base_weight;
        if top_row {
            let Some(multiplicity) = top_row_vertical_orbit_weight(n, successor) else {
                continue;
            };
            weight = weight
                .checked_mul(multiplicity)
                .ok_or_else(|| "coefficient overflow in sparse batched D4 weighting".to_owned())?;
        }
        output.push((PackedBoundary::pack(successor, n), weight));
    }
    Ok(output.len() - start_len)
}

fn shard_index(key: u128, n: usize, shards: usize, mode: ShardMode) -> usize {
    let mask = shards - 1;
    match mode {
        ShardMode::Prefix => {
            let shard_bits = shards.trailing_zeros() as usize;
            let used_bits = 3 * n;
            let shift = used_bits.saturating_sub(shard_bits);
            ((key >> shift) as usize) & mask
        }
        ShardMode::Mixed => {
            let mut mixed = key as u64 ^ (key >> 64) as u64;
            mixed ^= mixed >> 30;
            mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            mixed ^= mixed >> 27;
            mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
            mixed ^= mixed >> 31;
            mixed as usize & mask
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_compiled_sparse_sharded_d4(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    top_row: bool,
    shard_mode: ShardMode,
    counters: &mut RowCounters,
    output: &mut [Vec<(PackedBoundary, u128)>],
) -> Result<usize, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 {
        return Err(
            "sharded sparse iterator requires occupied row_in to match the left v0 boundary"
                .to_owned(),
        );
    }
    let board_mask = (1_u64 << n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let base_weight = parent_weight
        .checked_mul(occupied.value)
        .ok_or_else(|| "coefficient overflow in sharded sparse row operator".to_owned())?;
    let mut appended = 0_usize;

    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let successor = BoundaryState {
            columns: replace_bit(parent.columns, column, legs.column_out),
            diag_dr: (replace_bit(parent.diag_dr, column, legs.diag_dr_out) << 1) & board_mask,
            diag_dl: replace_bit(parent.diag_dl, column, legs.diag_dl_out) >> 1,
        };
        let mut weight = base_weight;
        if top_row {
            let Some(multiplicity) = top_row_vertical_orbit_weight(n, successor) else {
                continue;
            };
            weight = weight
                .checked_mul(multiplicity)
                .ok_or_else(|| "coefficient overflow in sharded D4 weighting".to_owned())?;
        }
        let packed = PackedBoundary::pack(successor, n);
        let selected_shard = shard_index(packed.0, n, output.len(), shard_mode);
        output[selected_shard].push((packed, weight));
        appended += 1;
    }
    Ok(appended)
}

fn contract_rows_d4_sharded_sparse_kernel(
    n: usize,
    shards: usize,
    shard_mode: ShardMode,
) -> Result<ContractionResult, String> {
    if n > 42 {
        return Err("the packed u128 virtual-boundary backend supports N <= 42".to_owned());
    }
    if shards == 0 || !shards.is_power_of_two() || shards > 256 {
        return Err("shards must be a power of two in 1..=256".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let initial_packed = PackedBoundary::pack(initial, n);
    let mut boundary = (0..shards)
        .map(|_| Vec::<(PackedBoundary, u128)>::new())
        .collect::<Vec<_>>();
    boundary[shard_index(initial_packed.0, n, shards, shard_mode)].push((initial_packed, 1_u128));
    let mut candidates = (0..shards)
        .map(|_| Vec::<(PackedBoundary, u128)>::new())
        .collect::<Vec<_>>();
    let mut peak_states = 1;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states: usize = boundary.iter().map(Vec::len).sum();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        for shard in &mut candidates {
            shard.clear();
        }

        for parent_shard in &boundary {
            for &(packed_parent, parent_weight) in parent_shard {
                completed_row_terms += append_compiled_sparse_sharded_d4(
                    n,
                    &operator,
                    packed_parent.unpack(n),
                    parent_weight,
                    row == 0,
                    shard_mode,
                    &mut counters,
                    &mut candidates,
                )? as u128;
            }
        }

        candidates
            .par_iter_mut()
            .try_for_each(|shard| -> Result<(), String> {
                shard.sort_unstable_by_key(|(state, _)| state.0);
                reduce_sorted_candidates(shard, row)
            })?;
        let output_states = candidates.iter().map(Vec::len).sum();
        let output_weight = candidates
            .iter()
            .flatten()
            .try_fold(0_u128, |sum, (_, value)| {
                sum.checked_add(*value)
                    .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
            })?;
        peak_states = peak_states.max(output_states);
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        std::mem::swap(&mut boundary, &mut candidates);
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states,
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .flatten()
        .filter(|(state, _)| state.columns(n) == board_mask)
        .try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| "final coefficient sum overflow".to_owned())
        })?;
    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: 17,
        tensor_entries_matched: 17,
        row_operator_candidates: total_operator_candidates,
        row_operator_matched: total_operator_matched,
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

#[derive(Clone, Copy)]
struct CompactEntry {
    key: u64,
    weight_low: u64,
    weight_high: u64,
}

impl CompactEntry {
    fn new(key: u64, weight: u128) -> Self {
        Self {
            key,
            weight_low: weight as u64,
            weight_high: (weight >> 64) as u64,
        }
    }

    fn weight(self) -> u128 {
        u128::from(self.weight_low) | (u128::from(self.weight_high) << 64)
    }

    fn set_weight(&mut self, weight: u128) {
        self.weight_low = weight as u64;
        self.weight_high = (weight >> 64) as u64;
    }
}

fn reduce_sorted_compact(entries: &mut Vec<CompactEntry>, row: usize) -> Result<(), String> {
    let mut write = 0_usize;
    for read in 0..entries.len() {
        let selected = entries[read];
        if write > 0 && entries[write - 1].key == selected.key {
            let combined = entries[write - 1]
                .weight()
                .checked_add(selected.weight())
                .ok_or_else(|| format!("compact coefficient overflow after row {}", row + 1))?;
            entries[write - 1].set_weight(combined);
        } else {
            entries[write] = selected;
            write += 1;
        }
    }
    entries.truncate(write);
    Ok(())
}

fn append_compact_sparse_sharded_d4(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u128,
    top_row: bool,
    counters: &mut RowCounters,
    output: &mut [Vec<CompactEntry>],
) -> Result<usize, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 {
        return Err(
            "compact sparse iterator requires occupied row_in to match the left v0 boundary"
                .to_owned(),
        );
    }
    let board_mask = (1_u64 << n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let base_weight = parent_weight
        .checked_mul(occupied.value)
        .ok_or_else(|| "coefficient overflow in compact sparse row operator".to_owned())?;
    let mut appended = 0_usize;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let successor = BoundaryState {
            columns: replace_bit(parent.columns, column, legs.column_out),
            diag_dr: (replace_bit(parent.diag_dr, column, legs.diag_dr_out) << 1) & board_mask,
            diag_dl: replace_bit(parent.diag_dl, column, legs.diag_dl_out) >> 1,
        };
        let mut weight = base_weight;
        if top_row {
            let Some(multiplicity) = top_row_vertical_orbit_weight(n, successor) else {
                continue;
            };
            weight = weight
                .checked_mul(multiplicity)
                .ok_or_else(|| "coefficient overflow in compact D4 weighting".to_owned())?;
        }
        let packed = PackedBoundary::pack(successor, n).0;
        let key = u64::try_from(packed)
            .map_err(|_| "compact virtual boundary key does not fit u64".to_owned())?;
        let selected_shard = shard_index(u128::from(key), n, output.len(), ShardMode::Prefix);
        output[selected_shard].push(CompactEntry::new(key, weight));
        appended += 1;
    }
    Ok(appended)
}

#[derive(Clone, Copy)]
enum FrontierLeg {
    Boundary(u8),
    Edge(usize),
}

#[derive(Clone, Copy)]
struct FrontierSite {
    row: usize,
    column: usize,
    legs: [FrontierLeg; 8],
}

#[derive(Clone, Copy)]
struct FrontierTransition {
    required_mask: u128,
    required_value: u128,
    added_value: u128,
    local_value: u128,
}

fn explicit_frontier_sites(n: usize) -> Vec<FrontierSite> {
    let span = n.saturating_sub(1);
    let row_offset = 0;
    let column_offset = n * span;
    let diag_dr_offset = column_offset + n * span;
    let diag_dl_offset = diag_dr_offset + span * span;
    let row_edge = |row: usize, left_column: usize| row_offset + row * span + left_column;
    let column_edge = |top_row: usize, column: usize| column_offset + top_row * n + column;
    let diag_dr_edge =
        |top_row: usize, left_column: usize| diag_dr_offset + top_row * span + left_column;
    let diag_dl_edge =
        |top_row: usize, left_column: usize| diag_dl_offset + top_row * span + left_column;

    let mut sites = Vec::with_capacity(n * n);
    for row in 0..n {
        for column in 0..n {
            let column_in = if row == 0 {
                FrontierLeg::Boundary(0b01)
            } else {
                FrontierLeg::Edge(column_edge(row - 1, column))
            };
            let column_out = if row + 1 == n {
                FrontierLeg::Boundary(0b10)
            } else {
                FrontierLeg::Edge(column_edge(row, column))
            };
            let row_in = if column == 0 {
                FrontierLeg::Boundary(0b01)
            } else {
                FrontierLeg::Edge(row_edge(row, column - 1))
            };
            let row_out = if column + 1 == n {
                FrontierLeg::Boundary(0b10)
            } else {
                FrontierLeg::Edge(row_edge(row, column))
            };
            let diag_dr_in = if row == 0 || column == 0 {
                FrontierLeg::Boundary(0b01)
            } else {
                FrontierLeg::Edge(diag_dr_edge(row - 1, column - 1))
            };
            let diag_dr_out = if row + 1 == n || column + 1 == n {
                FrontierLeg::Boundary(0b11)
            } else {
                FrontierLeg::Edge(diag_dr_edge(row, column))
            };
            let diag_dl_in = if row == 0 || column + 1 == n {
                FrontierLeg::Boundary(0b01)
            } else {
                FrontierLeg::Edge(diag_dl_edge(row - 1, column))
            };
            let diag_dl_out = if row + 1 == n || column == 0 {
                FrontierLeg::Boundary(0b11)
            } else {
                FrontierLeg::Edge(diag_dl_edge(row, column - 1))
            };
            sites.push(FrontierSite {
                row,
                column,
                legs: [
                    column_in,
                    column_out,
                    row_in,
                    row_out,
                    diag_dr_in,
                    diag_dr_out,
                    diag_dl_in,
                    diag_dl_out,
                ],
            });
        }
    }
    sites
}

fn explicit_frontier_order(n: usize, order: ExplicitFrontierOrder) -> Vec<usize> {
    let mut sites = (0..n * n).collect::<Vec<_>>();
    if order == ExplicitFrontierOrder::TopLeftDiamond {
        sites.sort_unstable_by_key(|&index| {
            let row = index / n;
            let column = index % n;
            (row + column, row)
        });
    }
    sites
}

fn c_entry_leg_values(entry: CEntry) -> [u8; 8] {
    [
        entry.legs.column_in,
        entry.legs.column_out,
        entry.legs.row_in,
        entry.legs.row_out,
        entry.legs.diag_dr_in,
        entry.legs.diag_dr_out,
        entry.legs.diag_dl_in,
        entry.legs.diag_dl_out,
    ]
}

fn contract_explicit_c_frontier_kernel(
    n: usize,
    order: ExplicitFrontierOrder,
    support_limit: usize,
) -> Result<ExplicitFrontierResult, String> {
    if n > 21 {
        return Err("explicit-C frontier key supports N <= 21".to_owned());
    }
    if support_limit == 0 {
        return Err("explicit-C frontier support limit must be positive".to_owned());
    }
    if n == 0 {
        return Ok(ExplicitFrontierResult {
            n,
            order,
            complete: true,
            count: Some(1),
            elapsed: Duration::ZERO,
            peak_states: 1,
            peak_open_bonds: 0,
            tensor_entries_examined: 0,
            tensor_entries_accepted: 0,
            contracted_sites: 0,
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    if tensor.entries().len() != 17 {
        return Err("explicit-C frontier requires the Sec. VI 17-entry tensor".to_owned());
    }
    let sites = explicit_frontier_sites(n);
    let ordering = explicit_frontier_order(n, order);
    let mut boundary = HashMap::<u128, u128>::from([(0, 1)]);
    let mut open_edges = Vec::<usize>::new();
    let mut peak_states = 1_usize;
    let mut peak_open_bonds = 0_usize;
    let mut tensor_entries_examined = 0_u128;
    let mut tensor_entries_accepted = 0_u128;
    let total_start = Instant::now();

    for (step, &site_index) in ordering.iter().enumerate() {
        let site = sites[site_index];
        debug_assert_eq!(site.row * n + site.column, site_index);
        let old_positions = open_edges
            .iter()
            .enumerate()
            .map(|(position, &edge)| (edge, position))
            .collect::<HashMap<_, _>>();
        let incident_edges = site
            .legs
            .iter()
            .filter_map(|leg| match leg {
                FrontierLeg::Edge(edge) => Some(*edge),
                FrontierLeg::Boundary(_) => None,
            })
            .collect::<Vec<_>>();
        let mut next_open_edges = open_edges
            .iter()
            .copied()
            .filter(|edge| !incident_edges.contains(edge))
            .collect::<Vec<_>>();
        for &edge in &incident_edges {
            if !old_positions.contains_key(&edge) {
                next_open_edges.push(edge);
            }
        }
        next_open_edges.sort_unstable();
        if next_open_edges.len() > 128 {
            return Err(format!(
                "explicit-C frontier exceeds 128 open bonds at site ({},{})",
                site.row, site.column
            ));
        }
        let next_positions = next_open_edges
            .iter()
            .enumerate()
            .map(|(position, &edge)| (edge, position))
            .collect::<HashMap<_, _>>();
        let carry_positions = open_edges
            .iter()
            .enumerate()
            .filter_map(|(old_position, edge)| {
                next_positions
                    .get(edge)
                    .map(|&next_position| (old_position, next_position))
            })
            .collect::<Vec<_>>();

        let transitions = tensor
            .entries()
            .iter()
            .filter_map(|&entry| {
                let values = c_entry_leg_values(entry);
                let mut required_mask = 0_u128;
                let mut required_value = 0_u128;
                let mut added_value = 0_u128;
                for (&leg, value) in site.legs.iter().zip(values) {
                    match leg {
                        FrontierLeg::Boundary(allowed) => {
                            if allowed & (1 << value) == 0 {
                                return None;
                            }
                        }
                        FrontierLeg::Edge(edge) => {
                            if let Some(&position) = old_positions.get(&edge) {
                                required_mask |= 1_u128 << position;
                                required_value |= u128::from(value) << position;
                            } else {
                                let position = next_positions[&edge];
                                added_value |= u128::from(value) << position;
                            }
                        }
                    }
                }
                Some(FrontierTransition {
                    required_mask,
                    required_value,
                    added_value,
                    local_value: entry.value,
                })
            })
            .collect::<Vec<_>>();

        let mut candidates = HashMap::<u128, u128>::new();
        for (&key, &weight) in &boundary {
            tensor_entries_examined += 17;
            let mut carried = 0_u128;
            for &(old_position, next_position) in &carry_positions {
                carried |= ((key >> old_position) & 1) << next_position;
            }
            for transition in &transitions {
                if key & transition.required_mask != transition.required_value {
                    continue;
                }
                tensor_entries_accepted += 1;
                let successor = carried | transition.added_value;
                let contribution = weight.checked_mul(transition.local_value).ok_or_else(|| {
                    format!(
                        "explicit-C coefficient multiplication overflow at site ({},{})",
                        site.row, site.column
                    )
                })?;
                let accumulated = candidates.entry(successor).or_insert(0);
                *accumulated = accumulated.checked_add(contribution).ok_or_else(|| {
                    format!(
                        "explicit-C coefficient addition overflow at site ({},{})",
                        site.row, site.column
                    )
                })?;
                if candidates.len() > support_limit {
                    peak_states = peak_states.max(candidates.len());
                    peak_open_bonds = peak_open_bonds.max(next_open_edges.len());
                    return Ok(ExplicitFrontierResult {
                        n,
                        order,
                        complete: false,
                        count: None,
                        elapsed: total_start.elapsed(),
                        peak_states,
                        peak_open_bonds,
                        tensor_entries_examined,
                        tensor_entries_accepted,
                        contracted_sites: step,
                        peak_rss_bytes: peak_rss_bytes(),
                    });
                }
            }
        }
        boundary = candidates;
        open_edges = next_open_edges;
        peak_states = peak_states.max(boundary.len());
        peak_open_bonds = peak_open_bonds.max(open_edges.len());
    }

    if !open_edges.is_empty() {
        return Err("explicit-C contraction ended with uncontracted virtual bonds".to_owned());
    }
    let count = boundary.get(&0).copied().unwrap_or(0);
    Ok(ExplicitFrontierResult {
        n,
        order,
        complete: true,
        count: Some(count),
        elapsed: total_start.elapsed(),
        peak_states,
        peak_open_bonds,
        tensor_entries_examined,
        tensor_entries_accepted,
        contracted_sites: n * n,
        peak_rss_bytes: peak_rss_bytes(),
    })
}

fn contract_rows_d4_compact_sharded_kernel(
    n: usize,
    shards: usize,
) -> Result<ContractionResult, String> {
    if n > 21 {
        return Err("the compact u64 virtual-boundary backend supports N <= 21".to_owned());
    }
    if shards == 0 || !shards.is_power_of_two() || shards > 256 {
        return Err("shards must be a power of two in 1..=256".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }
    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let initial_key = u64::try_from(PackedBoundary::pack(initial, n).0)
        .map_err(|_| "initial compact key does not fit u64".to_owned())?;
    let mut boundary = (0..shards)
        .map(|_| Vec::<CompactEntry>::new())
        .collect::<Vec<_>>();
    boundary[shard_index(u128::from(initial_key), n, shards, ShardMode::Prefix)]
        .push(CompactEntry::new(initial_key, 1));
    let mut candidates = (0..shards)
        .map(|_| Vec::<CompactEntry>::new())
        .collect::<Vec<_>>();
    let mut peak_states = 1_usize;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.iter().map(Vec::len).sum();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        for shard in &mut candidates {
            shard.clear();
        }
        for parent_shard in &boundary {
            for &parent in parent_shard {
                completed_row_terms += append_compact_sparse_sharded_d4(
                    n,
                    &operator,
                    PackedBoundary(u128::from(parent.key)).unpack(n),
                    parent.weight(),
                    row == 0,
                    &mut counters,
                    &mut candidates,
                )? as u128;
            }
        }
        candidates
            .par_iter_mut()
            .try_for_each(|shard| -> Result<(), String> {
                shard.sort_unstable_by_key(|entry| entry.key);
                reduce_sorted_compact(shard, row)
            })?;
        let output_states = candidates.iter().map(Vec::len).sum();
        let output_weight = candidates.iter().flatten().try_fold(0_u128, |sum, entry| {
            sum.checked_add(entry.weight())
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(output_states);
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        std::mem::swap(&mut boundary, &mut candidates);
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states,
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }
    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .flatten()
        .filter(|entry| PackedBoundary(u128::from(entry.key)).columns(n) == board_mask)
        .try_fold(0_u128, |sum, entry| {
            sum.checked_add(entry.weight())
                .ok_or_else(|| "final compact coefficient sum overflow".to_owned())
        })?;
    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: 17,
        tensor_entries_matched: 17,
        row_operator_candidates: total_operator_candidates,
        row_operator_matched: total_operator_matched,
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

struct LocalGeneration {
    buckets: Vec<Vec<CompactEntry>>,
    counters: RowCounters,
    completed_row_terms: u128,
    error: Option<String>,
    parents_since_flush: usize,
    peak_capacity_bytes: usize,
}

impl LocalGeneration {
    fn new(shards: usize) -> Self {
        Self {
            buckets: (0..shards).map(|_| Vec::<CompactEntry>::new()).collect(),
            counters: RowCounters::default(),
            completed_row_terms: 0,
            error: None,
            parents_since_flush: 0,
            peak_capacity_bytes: 0,
        }
    }

    fn capacity_bytes(&self) -> usize {
        self.buckets
            .iter()
            .map(|bucket| bucket.capacity() * std::mem::size_of::<CompactEntry>())
            .sum()
    }

    fn flush(&mut self, shared: &[Mutex<Vec<CompactEntry>>], force: bool) -> Result<(), String> {
        self.peak_capacity_bytes = self.peak_capacity_bytes.max(self.capacity_bytes());
        for (local, destination) in self.buckets.iter_mut().zip(shared) {
            if !local.is_empty() && (force || local.len() >= 1_024) {
                let mut destination = destination
                    .lock()
                    .map_err(|_| "parallel candidate bucket lock was poisoned".to_owned())?;
                destination.extend_from_slice(local);
                local.clear();
            }
        }
        self.parents_since_flush = 0;
        Ok(())
    }
}

fn contract_rows_d4_compact_parallel_generation_kernel(
    n: usize,
    shards: usize,
) -> Result<ParallelGenerationResult, String> {
    if n > 21 {
        return Err("the compact u64 virtual-boundary backend supports N <= 21".to_owned());
    }
    if shards == 0 || !shards.is_power_of_two() || shards > 256 {
        return Err("shards must be a power of two in 1..=256".to_owned());
    }
    if n == 0 {
        return Ok(ParallelGenerationResult {
            contraction: ContractionResult {
                n,
                count: 1,
                elapsed: Duration::ZERO,
                peak_states: 1,
                tensor_entries_examined: 0,
                tensor_entries_matched: 0,
                row_operator_candidates: 0,
                row_operator_matched: 0,
                peak_rss_bytes: peak_rss_bytes(),
                layers: Vec::new(),
            },
            generation_elapsed: Duration::ZERO,
            sort_elapsed: Duration::ZERO,
            reduce_elapsed: Duration::ZERO,
            peak_thread_local_bytes: 0,
            peak_worker_partials: 0,
        });
    }

    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let initial_key = u64::try_from(PackedBoundary::pack(initial, n).0)
        .map_err(|_| "initial compact key does not fit u64".to_owned())?;
    let mut boundary = (0..shards)
        .map(|_| Vec::<CompactEntry>::new())
        .collect::<Vec<_>>();
    boundary[shard_index(u128::from(initial_key), n, shards, ShardMode::Prefix)]
        .push(CompactEntry::new(initial_key, 1));
    let mut peak_states = 1_usize;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(n);
    let mut generation_elapsed = Duration::ZERO;
    let mut sort_elapsed = Duration::ZERO;
    let mut reduce_elapsed = Duration::ZERO;
    let mut peak_thread_local_bytes = 0_usize;
    let mut peak_worker_partials = 0_usize;
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states: usize = boundary.iter().map(Vec::len).sum();

        let generation_start = Instant::now();
        let worker_count = rayon::current_num_threads().max(1);
        let target_states = input_states.div_ceil(worker_count).max(1);
        let mut source_ranges = Vec::<(usize, usize)>::with_capacity(worker_count);
        let mut range_start = 0_usize;
        let mut range_states = 0_usize;
        for (index, source_shard) in boundary.iter().enumerate() {
            range_states += source_shard.len();
            if range_states >= target_states
                && source_ranges.len() + 1 < worker_count
                && index + 1 < shards
            {
                source_ranges.push((range_start, index + 1));
                range_start = index + 1;
                range_states = 0;
            }
        }
        source_ranges.push((range_start, shards));
        let shared = (0..shards)
            .map(|_| Mutex::new(Vec::<CompactEntry>::new()))
            .collect::<Vec<_>>();
        let partials = source_ranges
            .par_iter()
            .map(|&(start, end)| {
                let mut local = LocalGeneration::new(shards);
                for parent_shard in &boundary[start..end] {
                    if local.error.is_some() {
                        break;
                    }
                    for &parent in parent_shard {
                        match append_compact_sparse_sharded_d4(
                            n,
                            &operator,
                            PackedBoundary(u128::from(parent.key)).unpack(n),
                            parent.weight(),
                            row == 0,
                            &mut local.counters,
                            &mut local.buckets,
                        ) {
                            Ok(count) => local.completed_row_terms += count as u128,
                            Err(error) => {
                                local.error = Some(error);
                                break;
                            }
                        }
                        local.parents_since_flush += 1;
                        if local.parents_since_flush >= 256
                            && let Err(error) = local.flush(&shared, false)
                        {
                            local.error = Some(error);
                            break;
                        }
                    }
                }
                if local.error.is_none()
                    && let Err(error) = local.flush(&shared, true)
                {
                    local.error = Some(error);
                }
                local
            })
            .collect::<Vec<_>>();
        peak_worker_partials = peak_worker_partials.max(partials.len());
        peak_thread_local_bytes = peak_thread_local_bytes.max(
            partials
                .iter()
                .map(|partial| partial.peak_capacity_bytes)
                .sum(),
        );
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        for partial in partials {
            if let Some(error) = partial.error {
                return Err(error);
            }
            counters.tensor_examined += partial.counters.tensor_examined;
            counters.tensor_matched += partial.counters.tensor_matched;
            counters.operator_candidates += partial.counters.operator_candidates;
            counters.operator_matched += partial.counters.operator_matched;
            completed_row_terms += partial.completed_row_terms;
        }
        drop(boundary);
        let mut candidates = shared
            .into_iter()
            .map(|bucket| {
                bucket
                    .into_inner()
                    .map_err(|_| "parallel candidate bucket lock was poisoned".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        generation_elapsed += generation_start.elapsed();

        let sort_start = Instant::now();
        candidates
            .par_iter_mut()
            .for_each(|shard| shard.sort_unstable_by_key(|entry| entry.key));
        sort_elapsed += sort_start.elapsed();

        let reduce_start = Instant::now();
        candidates
            .par_iter_mut()
            .try_for_each(|shard| reduce_sorted_compact(shard, row))?;
        reduce_elapsed += reduce_start.elapsed();

        let output_states = candidates.iter().map(Vec::len).sum();
        let output_weight = candidates.iter().flatten().try_fold(0_u128, |sum, entry| {
            sum.checked_add(entry.weight())
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(output_states);
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        boundary = candidates;
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states,
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .flatten()
        .filter(|entry| PackedBoundary(u128::from(entry.key)).columns(n) == board_mask)
        .try_fold(0_u128, |sum, entry| {
            sum.checked_add(entry.weight())
                .ok_or_else(|| "final parallel-generation coefficient overflow".to_owned())
        })?;
    Ok(ParallelGenerationResult {
        contraction: ContractionResult {
            n,
            count,
            elapsed: total_start.elapsed(),
            peak_states,
            tensor_entries_examined: 17,
            tensor_entries_matched: 17,
            row_operator_candidates: total_operator_candidates,
            row_operator_matched: total_operator_matched,
            peak_rss_bytes: peak_rss_bytes(),
            layers,
        },
        generation_elapsed,
        sort_elapsed,
        reduce_elapsed,
        peak_thread_local_bytes,
        peak_worker_partials,
    })
}

const U64_PROMOTION_PREFIX: &str = "u64 coefficient promotion required";

fn u64_promotion(row: usize, operation: &str) -> String {
    format!(
        "{U64_PROMOTION_PREFIX} after row {} during {operation}",
        row + 1
    )
}

#[derive(Clone, Copy)]
struct CompactEntry64 {
    key: u64,
    weight: u64,
}

#[derive(Clone, Copy)]
struct Compact64Config {
    n: usize,
    row: usize,
    top_row: bool,
    coefficient_limit: u64,
}

fn reduce_sorted_compact64(
    entries: &mut Vec<CompactEntry64>,
    row: usize,
    coefficient_limit: u64,
) -> Result<(), String> {
    let mut write = 0_usize;
    for read in 0..entries.len() {
        let selected = entries[read];
        if write > 0 && entries[write - 1].key == selected.key {
            let combined = entries[write - 1]
                .weight
                .checked_add(selected.weight)
                .filter(|&value| value <= coefficient_limit)
                .ok_or_else(|| u64_promotion(row, "sorted reduction"))?;
            entries[write - 1].weight = combined;
        } else {
            entries[write] = selected;
            write += 1;
        }
    }
    entries.truncate(write);
    Ok(())
}

fn append_compact64_sparse_sharded_d4(
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u64,
    config: Compact64Config,
    counters: &mut RowCounters,
    output: &mut [Vec<CompactEntry64>],
) -> Result<usize, String> {
    let Compact64Config {
        n,
        row,
        top_row,
        coefficient_limit,
    } = config;
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 {
        return Err(
            "compact64 sparse iterator requires occupied row_in to match the left v0 boundary"
                .to_owned(),
        );
    }
    let occupied_value = u64::try_from(occupied.value)
        .map_err(|_| u64_promotion(row, "local C-entry multiplication"))?;
    let board_mask = (1_u64 << n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let base_weight = parent_weight
        .checked_mul(occupied_value)
        .filter(|&value| value <= coefficient_limit)
        .ok_or_else(|| u64_promotion(row, "local C-entry multiplication"))?;
    let mut appended = 0_usize;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let successor = BoundaryState {
            columns: replace_bit(parent.columns, column, legs.column_out),
            diag_dr: (replace_bit(parent.diag_dr, column, legs.diag_dr_out) << 1) & board_mask,
            diag_dl: replace_bit(parent.diag_dl, column, legs.diag_dl_out) >> 1,
        };
        let mut weight = base_weight;
        if top_row {
            let Some(multiplicity) = top_row_vertical_orbit_weight(n, successor) else {
                continue;
            };
            let multiplicity = u64::try_from(multiplicity)
                .map_err(|_| u64_promotion(row, "D4 orbit weighting"))?;
            weight = weight
                .checked_mul(multiplicity)
                .filter(|&value| value <= coefficient_limit)
                .ok_or_else(|| u64_promotion(row, "D4 orbit weighting"))?;
        }
        let packed = PackedBoundary::pack(successor, n).0;
        let key = u64::try_from(packed)
            .map_err(|_| "compact64 virtual boundary key does not fit u64".to_owned())?;
        let selected_shard = shard_index(u128::from(key), n, output.len(), ShardMode::Prefix);
        output[selected_shard].push(CompactEntry64 { key, weight });
        appended += 1;
    }
    Ok(appended)
}

struct LocalGeneration64 {
    buckets: Vec<Vec<CompactEntry64>>,
    counters: RowCounters,
    completed_row_terms: u128,
    error: Option<String>,
    parents_since_flush: usize,
    peak_capacity_bytes: usize,
}

impl LocalGeneration64 {
    fn new(shards: usize) -> Self {
        Self {
            buckets: (0..shards).map(|_| Vec::<CompactEntry64>::new()).collect(),
            counters: RowCounters::default(),
            completed_row_terms: 0,
            error: None,
            parents_since_flush: 0,
            peak_capacity_bytes: 0,
        }
    }

    fn capacity_bytes(&self) -> usize {
        self.buckets
            .iter()
            .map(|bucket| bucket.capacity() * std::mem::size_of::<CompactEntry64>())
            .sum()
    }

    fn flush(&mut self, shared: &[Mutex<Vec<CompactEntry64>>], force: bool) -> Result<(), String> {
        self.peak_capacity_bytes = self.peak_capacity_bytes.max(self.capacity_bytes());
        for (local, destination) in self.buckets.iter_mut().zip(shared) {
            if !local.is_empty() && (force || local.len() >= 1_024) {
                let mut destination = destination.lock().map_err(|_| {
                    "parallel compact64 candidate bucket lock was poisoned".to_owned()
                })?;
                destination.extend_from_slice(local);
                local.clear();
            }
        }
        self.parents_since_flush = 0;
        Ok(())
    }
}

fn contract_rows_d4_compact64_parallel_generation_kernel(
    n: usize,
    shards: usize,
    coefficient_limit: u64,
) -> Result<ParallelGenerationResult, String> {
    if n > 21 {
        return Err("the compact64 virtual-boundary backend supports N <= 21".to_owned());
    }
    if shards == 0 || !shards.is_power_of_two() || shards > 256 {
        return Err("shards must be a power of two in 1..=256".to_owned());
    }
    if coefficient_limit == 0 {
        return Err("compact64 coefficient limit must be positive".to_owned());
    }
    if n == 0 {
        return contract_rows_d4_compact_parallel_generation_kernel(n, shards);
    }

    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let initial_key = u64::try_from(PackedBoundary::pack(initial, n).0)
        .map_err(|_| "initial compact64 key does not fit u64".to_owned())?;
    let mut boundary = (0..shards)
        .map(|_| Vec::<CompactEntry64>::new())
        .collect::<Vec<_>>();
    boundary[shard_index(u128::from(initial_key), n, shards, ShardMode::Prefix)].push(
        CompactEntry64 {
            key: initial_key,
            weight: 1,
        },
    );
    let mut peak_states = 1_usize;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(n);
    let mut generation_elapsed = Duration::ZERO;
    let mut sort_elapsed = Duration::ZERO;
    let mut reduce_elapsed = Duration::ZERO;
    let mut peak_thread_local_bytes = 0_usize;
    let mut peak_worker_partials = 0_usize;
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states: usize = boundary.iter().map(Vec::len).sum();
        let generation_start = Instant::now();
        let worker_count = rayon::current_num_threads().max(1);
        let target_states = input_states.div_ceil(worker_count).max(1);
        let mut source_ranges = Vec::<(usize, usize)>::with_capacity(worker_count);
        let mut range_start = 0_usize;
        let mut range_states = 0_usize;
        for (index, source_shard) in boundary.iter().enumerate() {
            range_states += source_shard.len();
            if range_states >= target_states
                && source_ranges.len() + 1 < worker_count
                && index + 1 < shards
            {
                source_ranges.push((range_start, index + 1));
                range_start = index + 1;
                range_states = 0;
            }
        }
        source_ranges.push((range_start, shards));
        let shared = (0..shards)
            .map(|_| Mutex::new(Vec::<CompactEntry64>::new()))
            .collect::<Vec<_>>();
        let partials = source_ranges
            .par_iter()
            .map(|&(start, end)| {
                let mut local = LocalGeneration64::new(shards);
                for parent_shard in &boundary[start..end] {
                    if local.error.is_some() {
                        break;
                    }
                    for &parent in parent_shard {
                        match append_compact64_sparse_sharded_d4(
                            &operator,
                            PackedBoundary(u128::from(parent.key)).unpack(n),
                            parent.weight,
                            Compact64Config {
                                n,
                                row,
                                top_row: row == 0,
                                coefficient_limit,
                            },
                            &mut local.counters,
                            &mut local.buckets,
                        ) {
                            Ok(count) => local.completed_row_terms += count as u128,
                            Err(error) => {
                                local.error = Some(error);
                                break;
                            }
                        }
                        local.parents_since_flush += 1;
                        if local.parents_since_flush >= 256
                            && let Err(error) = local.flush(&shared, false)
                        {
                            local.error = Some(error);
                            break;
                        }
                    }
                }
                if local.error.is_none()
                    && let Err(error) = local.flush(&shared, true)
                {
                    local.error = Some(error);
                }
                local
            })
            .collect::<Vec<_>>();
        peak_worker_partials = peak_worker_partials.max(partials.len());
        peak_thread_local_bytes = peak_thread_local_bytes.max(
            partials
                .iter()
                .map(|partial| partial.peak_capacity_bytes)
                .sum(),
        );
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        for partial in partials {
            if let Some(error) = partial.error {
                return Err(error);
            }
            counters.tensor_examined += partial.counters.tensor_examined;
            counters.tensor_matched += partial.counters.tensor_matched;
            counters.operator_candidates += partial.counters.operator_candidates;
            counters.operator_matched += partial.counters.operator_matched;
            completed_row_terms += partial.completed_row_terms;
        }
        drop(boundary);
        let mut candidates = shared
            .into_iter()
            .map(|bucket| {
                bucket
                    .into_inner()
                    .map_err(|_| "parallel compact64 candidate bucket lock was poisoned".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        generation_elapsed += generation_start.elapsed();

        let sort_start = Instant::now();
        candidates
            .par_iter_mut()
            .for_each(|shard| shard.sort_unstable_by_key(|entry| entry.key));
        sort_elapsed += sort_start.elapsed();

        let reduce_start = Instant::now();
        candidates
            .par_iter_mut()
            .try_for_each(|shard| reduce_sorted_compact64(shard, row, coefficient_limit))?;
        reduce_elapsed += reduce_start.elapsed();

        let output_states = candidates.iter().map(Vec::len).sum();
        let output_weight = candidates.iter().flatten().try_fold(0_u128, |sum, entry| {
            sum.checked_add(u128::from(entry.weight))
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(output_states);
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        boundary = candidates;
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states,
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .flatten()
        .filter(|entry| PackedBoundary(u128::from(entry.key)).columns(n) == board_mask)
        .try_fold(0_u128, |sum, entry| {
            sum.checked_add(u128::from(entry.weight))
                .ok_or_else(|| "final compact64 coefficient sum overflow".to_owned())
        })?;
    Ok(ParallelGenerationResult {
        contraction: ContractionResult {
            n,
            count,
            elapsed: total_start.elapsed(),
            peak_states,
            tensor_entries_examined: 17,
            tensor_entries_matched: 17,
            row_operator_candidates: total_operator_candidates,
            row_operator_matched: total_operator_matched,
            peak_rss_bytes: peak_rss_bytes(),
            layers,
        },
        generation_elapsed,
        sort_elapsed,
        reduce_elapsed,
        peak_thread_local_bytes,
        peak_worker_partials,
    })
}

fn contract_rows_d4_compact_u64_promoting_with_limit(
    n: usize,
    shards: usize,
    coefficient_limit: u64,
) -> Result<U64PromotionResult, String> {
    let total_start = Instant::now();
    let fast_start = Instant::now();
    match contract_rows_d4_compact64_parallel_generation_kernel(n, shards, coefficient_limit) {
        Ok(result) => Ok(U64PromotionResult {
            contraction: result.contraction,
            used_u64_fast_path: true,
            promotion_reason: None,
            attempted_fast_path_elapsed: fast_start.elapsed(),
            generation_elapsed: result.generation_elapsed,
            sort_elapsed: result.sort_elapsed,
            reduce_elapsed: result.reduce_elapsed,
            peak_thread_local_bytes: result.peak_thread_local_bytes,
        }),
        Err(error) if error.starts_with(U64_PROMOTION_PREFIX) => {
            let attempted_fast_path_elapsed = fast_start.elapsed();
            let fallback = contract_rows_d4_compact_parallel_generation_kernel(n, shards)?;
            let mut contraction = fallback.contraction;
            contraction.elapsed = total_start.elapsed();
            contraction.peak_rss_bytes = contraction.peak_rss_bytes.max(peak_rss_bytes());
            Ok(U64PromotionResult {
                contraction,
                used_u64_fast_path: false,
                promotion_reason: Some(error),
                attempted_fast_path_elapsed,
                generation_elapsed: fallback.generation_elapsed,
                sort_elapsed: fallback.sort_elapsed,
                reduce_elapsed: fallback.reduce_elapsed,
                peak_thread_local_bytes: fallback.peak_thread_local_bytes,
            })
        }
        Err(error) => Err(error),
    }
}

const JOINT_PROMOTION_PREFIX: &str = "joint-u64 coefficient promotion required";

fn joint_promotion(row: usize, operation: &str) -> String {
    format!(
        "{JOINT_PROMOTION_PREFIX} after row {} during {operation}",
        row + 1
    )
}

#[derive(Clone, Copy)]
struct JointEntry(u64);

impl JointEntry {
    fn new(key: u64, weight: u64, coefficient_bits: u32) -> Self {
        Self((key << coefficient_bits) | weight)
    }

    fn key(self, coefficient_bits: u32) -> u64 {
        self.0 >> coefficient_bits
    }

    fn weight(self, coefficient_mask: u64) -> u64 {
        self.0 & coefficient_mask
    }
}

fn coefficient_mask(bits: u32) -> u64 {
    if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

#[derive(Clone, Copy)]
struct JointConfig {
    n: usize,
    row: usize,
    top_row: bool,
    coefficient_bits: u32,
    coefficient_mask: u64,
}

fn reduce_sorted_joint(
    entries: &mut Vec<JointEntry>,
    config: JointConfig,
    max_coefficient: &mut u64,
) -> Result<(), String> {
    let mut write = 0_usize;
    for read in 0..entries.len() {
        let selected = entries[read];
        let selected_key = selected.key(config.coefficient_bits);
        let selected_weight = selected.weight(config.coefficient_mask);
        if write > 0 && entries[write - 1].key(config.coefficient_bits) == selected_key {
            let combined = entries[write - 1]
                .weight(config.coefficient_mask)
                .checked_add(selected_weight)
                .filter(|&weight| weight <= config.coefficient_mask)
                .ok_or_else(|| joint_promotion(config.row, "sorted reduction"))?;
            entries[write - 1] = JointEntry::new(selected_key, combined, config.coefficient_bits);
            *max_coefficient = (*max_coefficient).max(combined);
        } else {
            entries[write] = selected;
            write += 1;
            *max_coefficient = (*max_coefficient).max(selected_weight);
        }
    }
    entries.truncate(write);
    Ok(())
}

fn append_joint_sparse_sharded_d4(
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_weight: u64,
    config: JointConfig,
    counters: &mut RowCounters,
    output: &mut [Vec<JointEntry>],
    max_coefficient: &mut u64,
) -> Result<usize, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 {
        return Err(
            "joint-u64 sparse iterator requires occupied row_in to match left v0".to_owned(),
        );
    }
    let occupied_value = u64::try_from(occupied.value)
        .map_err(|_| joint_promotion(config.row, "local C-entry multiplication"))?;
    let board_mask = (1_u64 << config.n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let base_weight = parent_weight
        .checked_mul(occupied_value)
        .filter(|&weight| weight <= config.coefficient_mask)
        .ok_or_else(|| joint_promotion(config.row, "local C-entry multiplication"))?;
    let mut appended = 0_usize;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let successor = BoundaryState {
            columns: replace_bit(parent.columns, column, legs.column_out),
            diag_dr: (replace_bit(parent.diag_dr, column, legs.diag_dr_out) << 1) & board_mask,
            diag_dl: replace_bit(parent.diag_dl, column, legs.diag_dl_out) >> 1,
        };
        let mut weight = base_weight;
        if config.top_row {
            let Some(multiplicity) = top_row_vertical_orbit_weight(config.n, successor) else {
                continue;
            };
            let multiplicity = u64::try_from(multiplicity)
                .map_err(|_| joint_promotion(config.row, "D4 orbit weighting"))?;
            weight = weight
                .checked_mul(multiplicity)
                .filter(|&value| value <= config.coefficient_mask)
                .ok_or_else(|| joint_promotion(config.row, "D4 orbit weighting"))?;
        }
        *max_coefficient = (*max_coefficient).max(weight);
        let key = u64::try_from(PackedBoundary::pack(successor, config.n).0)
            .map_err(|_| "joint-u64 virtual boundary key does not fit u64".to_owned())?;
        let selected_shard =
            shard_index(u128::from(key), config.n, output.len(), ShardMode::Prefix);
        output[selected_shard].push(JointEntry::new(key, weight, config.coefficient_bits));
        appended += 1;
    }
    Ok(appended)
}

struct LocalJointGeneration {
    buckets: Vec<Vec<JointEntry>>,
    counters: RowCounters,
    completed_row_terms: u128,
    error: Option<String>,
    parents_since_flush: usize,
    peak_capacity_bytes: usize,
    max_coefficient: u64,
}

impl LocalJointGeneration {
    fn new(shards: usize) -> Self {
        Self {
            buckets: (0..shards).map(|_| Vec::<JointEntry>::new()).collect(),
            counters: RowCounters::default(),
            completed_row_terms: 0,
            error: None,
            parents_since_flush: 0,
            peak_capacity_bytes: 0,
            max_coefficient: 1,
        }
    }

    fn capacity_bytes(&self) -> usize {
        self.buckets
            .iter()
            .map(|bucket| bucket.capacity() * std::mem::size_of::<JointEntry>())
            .sum()
    }

    fn flush(&mut self, shared: &[Mutex<Vec<JointEntry>>], force: bool) -> Result<(), String> {
        self.peak_capacity_bytes = self.peak_capacity_bytes.max(self.capacity_bytes());
        for (local, destination) in self.buckets.iter_mut().zip(shared) {
            if !local.is_empty() && (force || local.len() >= 1_024) {
                let mut destination = destination
                    .lock()
                    .map_err(|_| "parallel joint-u64 bucket lock was poisoned".to_owned())?;
                destination.extend_from_slice(local);
                local.clear();
            }
        }
        self.parents_since_flush = 0;
        Ok(())
    }
}

struct JointKernelResult {
    parallel: ParallelGenerationResult,
    max_coefficient_observed: u64,
    reuse: ArenaReuseMetrics,
    boundary: Vec<Vec<JointEntry>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ArenaReuseMetrics {
    total_reused_capacity_bytes: usize,
    total_destination_growth_bytes: usize,
    peak_spare_capacity_bytes: usize,
}

fn contract_rows_d4_joint_u64_kernel(
    n: usize,
    shards: usize,
    coefficient_bits: u32,
    reuse_arenas: bool,
    stop_after_rows: usize,
) -> Result<JointKernelResult, String> {
    if n > 21 {
        return Err("joint-u64 virtual-boundary backend supports N <= 21".to_owned());
    }
    if shards == 0 || !shards.is_power_of_two() || shards > 256 {
        return Err("shards must be a power of two in 1..=256".to_owned());
    }
    if stop_after_rows > n {
        return Err("joint-u64 prefix cut cannot exceed N".to_owned());
    }
    let key_bits = u32::try_from(3_usize.saturating_mul(n))
        .map_err(|_| "joint-u64 key width does not fit u32".to_owned())?;
    if coefficient_bits == 0 || coefficient_bits > 64 || key_bits + coefficient_bits > 64 {
        return Err("joint-u64 coefficient bits do not fit beside the 3N-bit key".to_owned());
    }
    if n == 0 {
        return Ok(JointKernelResult {
            parallel: contract_rows_d4_compact_parallel_generation_kernel(n, shards)?,
            max_coefficient_observed: 1,
            reuse: ArenaReuseMetrics::default(),
            boundary: Vec::new(),
        });
    }

    let mask = coefficient_mask(coefficient_bits);
    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let initial_key = u64::try_from(PackedBoundary::pack(initial, n).0)
        .map_err(|_| "initial joint-u64 key does not fit u64".to_owned())?;
    let mut boundary = (0..shards)
        .map(|_| Vec::<JointEntry>::new())
        .collect::<Vec<_>>();
    boundary[shard_index(u128::from(initial_key), n, shards, ShardMode::Prefix)]
        .push(JointEntry::new(initial_key, 1, coefficient_bits));
    let mut peak_states = 1_usize;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(stop_after_rows);
    let mut generation_elapsed = Duration::ZERO;
    let mut sort_elapsed = Duration::ZERO;
    let mut reduce_elapsed = Duration::ZERO;
    let mut peak_thread_local_bytes = 0_usize;
    let mut peak_worker_partials = 0_usize;
    let mut max_coefficient_observed = 1_u64;
    let mut reuse = ArenaReuseMetrics::default();
    let mut spare = (0..shards)
        .map(|_| Vec::<JointEntry>::new())
        .collect::<Vec<_>>();
    let total_start = Instant::now();

    for row in 0..stop_after_rows {
        let config = JointConfig {
            n,
            row,
            top_row: row == 0,
            coefficient_bits,
            coefficient_mask: mask,
        };
        let layer_start = Instant::now();
        let input_states: usize = boundary.iter().map(Vec::len).sum();
        let generation_start = Instant::now();
        let worker_count = rayon::current_num_threads().max(1);
        let target_states = input_states.div_ceil(worker_count).max(1);
        let mut source_ranges = Vec::<(usize, usize)>::with_capacity(worker_count);
        let mut range_start = 0_usize;
        let mut range_states = 0_usize;
        for (index, source_shard) in boundary.iter().enumerate() {
            range_states += source_shard.len();
            if range_states >= target_states
                && source_ranges.len() + 1 < worker_count
                && index + 1 < shards
            {
                source_ranges.push((range_start, index + 1));
                range_start = index + 1;
                range_states = 0;
            }
        }
        source_ranges.push((range_start, shards));
        let destination = if reuse_arenas {
            std::mem::take(&mut spare)
        } else {
            (0..shards)
                .map(|_| Vec::<JointEntry>::new())
                .collect::<Vec<_>>()
        };
        let initial_destination_capacity_bytes = destination
            .iter()
            .map(|bucket| bucket.capacity() * std::mem::size_of::<JointEntry>())
            .sum::<usize>();
        reuse.total_reused_capacity_bytes = reuse
            .total_reused_capacity_bytes
            .saturating_add(initial_destination_capacity_bytes);
        reuse.peak_spare_capacity_bytes = reuse
            .peak_spare_capacity_bytes
            .max(initial_destination_capacity_bytes);
        let shared = destination.into_iter().map(Mutex::new).collect::<Vec<_>>();
        let partials = source_ranges
            .par_iter()
            .map(|&(start, end)| {
                let mut local = LocalJointGeneration::new(shards);
                for parent_shard in &boundary[start..end] {
                    if local.error.is_some() {
                        break;
                    }
                    for &parent in parent_shard {
                        let key = parent.key(coefficient_bits);
                        let weight = parent.weight(mask);
                        match append_joint_sparse_sharded_d4(
                            &operator,
                            PackedBoundary(u128::from(key)).unpack(n),
                            weight,
                            config,
                            &mut local.counters,
                            &mut local.buckets,
                            &mut local.max_coefficient,
                        ) {
                            Ok(count) => local.completed_row_terms += count as u128,
                            Err(error) => {
                                local.error = Some(error);
                                break;
                            }
                        }
                        local.parents_since_flush += 1;
                        if local.parents_since_flush >= 256
                            && let Err(error) = local.flush(&shared, false)
                        {
                            local.error = Some(error);
                            break;
                        }
                    }
                }
                if local.error.is_none()
                    && let Err(error) = local.flush(&shared, true)
                {
                    local.error = Some(error);
                }
                local
            })
            .collect::<Vec<_>>();
        peak_worker_partials = peak_worker_partials.max(partials.len());
        peak_thread_local_bytes = peak_thread_local_bytes.max(
            partials
                .iter()
                .map(|partial| partial.peak_capacity_bytes)
                .sum(),
        );
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        for partial in partials {
            if let Some(error) = partial.error {
                return Err(error);
            }
            counters.tensor_examined += partial.counters.tensor_examined;
            counters.tensor_matched += partial.counters.tensor_matched;
            counters.operator_candidates += partial.counters.operator_candidates;
            counters.operator_matched += partial.counters.operator_matched;
            completed_row_terms += partial.completed_row_terms;
            max_coefficient_observed = max_coefficient_observed.max(partial.max_coefficient);
        }
        let mut previous_boundary = std::mem::take(&mut boundary);
        let mut candidates = shared
            .into_iter()
            .map(|bucket| {
                bucket
                    .into_inner()
                    .map_err(|_| "parallel joint-u64 bucket lock was poisoned".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let destination_capacity_bytes = candidates
            .iter()
            .map(|bucket| bucket.capacity() * std::mem::size_of::<JointEntry>())
            .sum::<usize>();
        reuse.total_destination_growth_bytes = reuse.total_destination_growth_bytes.saturating_add(
            destination_capacity_bytes.saturating_sub(initial_destination_capacity_bytes),
        );
        if reuse_arenas {
            previous_boundary.iter_mut().for_each(Vec::clear);
            spare = previous_boundary;
        }
        generation_elapsed += generation_start.elapsed();

        let sort_start = Instant::now();
        candidates
            .par_iter_mut()
            .for_each(|shard| shard.sort_unstable_by_key(|entry| entry.0));
        sort_elapsed += sort_start.elapsed();

        let reduce_start = Instant::now();
        let row_max = Mutex::new(1_u64);
        candidates.par_iter_mut().try_for_each(|shard| {
            let mut shard_max = 1_u64;
            reduce_sorted_joint(shard, config, &mut shard_max)?;
            let mut maximum = row_max
                .lock()
                .map_err(|_| "joint-u64 maximum lock was poisoned".to_owned())?;
            *maximum = (*maximum).max(shard_max);
            Ok::<(), String>(())
        })?;
        max_coefficient_observed = max_coefficient_observed.max(
            row_max
                .into_inner()
                .map_err(|_| "joint-u64 maximum lock was poisoned".to_owned())?,
        );
        reduce_elapsed += reduce_start.elapsed();

        let output_states = candidates.iter().map(Vec::len).sum();
        let output_weight = candidates.iter().flatten().try_fold(0_u128, |sum, entry| {
            sum.checked_add(u128::from(entry.weight(mask)))
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(output_states);
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        boundary = candidates;
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states,
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .flatten()
        .filter(|entry| {
            let key = entry.key(coefficient_bits);
            PackedBoundary(u128::from(key)).columns(n) == board_mask
        })
        .try_fold(0_u128, |sum, entry| {
            sum.checked_add(u128::from(entry.weight(mask)))
                .ok_or_else(|| "final joint-u64 coefficient sum overflow".to_owned())
        })?;
    Ok(JointKernelResult {
        parallel: ParallelGenerationResult {
            contraction: ContractionResult {
                n,
                count,
                elapsed: total_start.elapsed(),
                peak_states,
                tensor_entries_examined: 17,
                tensor_entries_matched: 17,
                row_operator_candidates: total_operator_candidates,
                row_operator_matched: total_operator_matched,
                peak_rss_bytes: peak_rss_bytes(),
                layers,
            },
            generation_elapsed,
            sort_elapsed,
            reduce_elapsed,
            peak_thread_local_bytes,
            peak_worker_partials,
        },
        max_coefficient_observed,
        reuse,
        boundary,
    })
}

fn contract_rows_d4_joint_u64_with_limits(
    n: usize,
    shards: usize,
    coefficient_bits: u32,
    compact64_limit: u64,
) -> Result<JointU64Result, String> {
    contract_rows_d4_joint_u64_with_reuse(n, shards, coefficient_bits, compact64_limit, false)
        .map(|(joint, _)| joint)
}

fn contract_rows_d4_joint_u64_with_reuse(
    n: usize,
    shards: usize,
    coefficient_bits: u32,
    compact64_limit: u64,
    reuse_arenas: bool,
) -> Result<(JointU64Result, ArenaReuseMetrics), String> {
    let total_start = Instant::now();
    let joint_start = Instant::now();
    match contract_rows_d4_joint_u64_kernel(n, shards, coefficient_bits, reuse_arenas, n) {
        Ok(result) => {
            let reuse = result.reuse;
            Ok((
                JointU64Result {
                    contraction: result.parallel.contraction,
                    used_joint_fast_path: true,
                    fallback_used_u64_fast_path: None,
                    promotion_reason: None,
                    coefficient_bits,
                    max_coefficient_observed: result.max_coefficient_observed,
                    attempted_joint_elapsed: joint_start.elapsed(),
                    generation_elapsed: result.parallel.generation_elapsed,
                    sort_elapsed: result.parallel.sort_elapsed,
                    reduce_elapsed: result.parallel.reduce_elapsed,
                    peak_thread_local_bytes: result.parallel.peak_thread_local_bytes,
                },
                reuse,
            ))
        }
        Err(error) if error.starts_with(JOINT_PROMOTION_PREFIX) => {
            let attempted_joint_elapsed = joint_start.elapsed();
            let fallback =
                contract_rows_d4_compact_u64_promoting_with_limit(n, shards, compact64_limit)?;
            let fallback_used_u64_fast_path = fallback.used_u64_fast_path;
            let fallback_reason = fallback.promotion_reason.clone();
            let mut contraction = fallback.contraction;
            contraction.elapsed = total_start.elapsed();
            contraction.peak_rss_bytes = contraction.peak_rss_bytes.max(peak_rss_bytes());
            Ok((
                JointU64Result {
                    contraction,
                    used_joint_fast_path: false,
                    fallback_used_u64_fast_path: Some(fallback_used_u64_fast_path),
                    promotion_reason: Some(match fallback_reason {
                        Some(reason) => format!("{error}; compact64 fallback: {reason}"),
                        None => error,
                    }),
                    coefficient_bits,
                    max_coefficient_observed: 0,
                    attempted_joint_elapsed,
                    generation_elapsed: fallback.generation_elapsed,
                    sort_elapsed: fallback.sort_elapsed,
                    reduce_elapsed: fallback.reduce_elapsed,
                    peak_thread_local_bytes: fallback.peak_thread_local_bytes,
                },
                ArenaReuseMetrics::default(),
            ))
        }
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug)]
pub struct RecursiveTailResult {
    pub contraction: ContractionResult,
    pub cut: usize,
    pub prefix_elapsed: Duration,
    pub tail_elapsed: Duration,
    pub prefix_support: usize,
    pub tail_tasks: usize,
    pub recursive_nodes: u128,
    pub recursive_accepted_entries: u128,
    pub coefficient_bits: u32,
    pub max_prefix_coefficient: u64,
    pub generation_elapsed: Duration,
    pub sort_elapsed: Duration,
    pub reduce_elapsed: Duration,
    pub peak_thread_local_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
struct RecursiveTailRelation {
    legs: VirtualLegs,
    value: u128,
}

impl RecursiveTailRelation {
    fn compile(operator: &CompiledRowOperator) -> Result<Self, String> {
        let entry = operator.occupied;
        let legs = entry.legs;
        let binary = [
            legs.column_in,
            legs.column_out,
            legs.row_in,
            legs.row_out,
            legs.diag_dr_in,
            legs.diag_dr_out,
            legs.diag_dl_in,
            legs.diag_dl_out,
        ]
        .into_iter()
        .all(|signal| signal <= 1);
        if !binary || legs.row_in != 0 || legs.row_out != 1 {
            return Err(
                "recursive tail requires the compiled binary v0-to-v1 row entry".to_owned(),
            );
        }
        Ok(Self {
            legs,
            value: entry.value,
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RecursiveTailMetrics {
    nodes: u128,
    accepted_entries: u128,
}

#[inline]
fn recursive_matching_bits(mask: u64, required: u8, board_mask: u64) -> u64 {
    if required == 0 {
        (!mask) & board_mask
    } else {
        mask & board_mask
    }
}

#[inline]
fn recursive_tail_positions(
    parent: BoundaryState,
    relation: RecursiveTailRelation,
    board_mask: u64,
) -> u64 {
    recursive_matching_bits(parent.columns, relation.legs.column_in, board_mask)
        & recursive_matching_bits(parent.diag_dr, relation.legs.diag_dr_in, board_mask)
        & recursive_matching_bits(parent.diag_dl, relation.legs.diag_dl_in, board_mask)
}

#[inline]
fn recursive_tail_successor(
    parent: BoundaryState,
    selected: u64,
    relation: RecursiveTailRelation,
    board_mask: u64,
) -> BoundaryState {
    let column = selected.trailing_zeros() as usize;
    BoundaryState {
        columns: replace_bit(parent.columns, column, relation.legs.column_out),
        diag_dr: (replace_bit(parent.diag_dr, column, relation.legs.diag_dr_out) << 1) & board_mask,
        diag_dl: replace_bit(parent.diag_dl, column, relation.legs.diag_dl_out) >> 1,
    }
}

fn contract_recursive_tail(
    n: usize,
    row: usize,
    parent: BoundaryState,
    relation: RecursiveTailRelation,
    board_mask: u64,
    metrics: &mut RecursiveTailMetrics,
) -> Result<u128, String> {
    // For N<=21, the complete recursion tree is bounded by
    // sum_k P(N,k) < e*N!, which is far below u128::MAX.
    metrics.nodes += 1;
    if row == n {
        // Column lines terminate in v1=(0,1): every column signal must be 1.
        // Both diagonal families terminate in v2=(1,1), so either outgoing
        // signal is accepted and no diagonal filter is applied here.
        return Ok(u128::from(parent.columns == board_mask));
    }

    let mut positions = recursive_tail_positions(parent, relation, board_mask);
    let mut count = 0_u128;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        positions &= positions - 1;
        metrics.accepted_entries += 1;
        let successor = recursive_tail_successor(parent, selected, relation, board_mask);
        let child = contract_recursive_tail(n, row + 1, successor, relation, board_mask, metrics)?;
        let weighted = child
            .checked_mul(relation.value)
            .ok_or_else(|| "coefficient overflow in recursive local C multiplication".to_owned())?;
        count = count
            .checked_add(weighted)
            .ok_or_else(|| "coefficient overflow in recursive tail reduction".to_owned())?;
    }
    Ok(count)
}

pub fn contract_rows_d4_recursive_tail(
    n: usize,
    shards: usize,
    cut: usize,
) -> Result<RecursiveTailResult, String> {
    if n > 21 {
        return Err("recursive-tail virtual-boundary backend supports N <= 21".to_owned());
    }
    if cut > n {
        return Err("recursive-tail cut cannot exceed N".to_owned());
    }
    if shards == 0 || !shards.is_power_of_two() || shards > 256 {
        return Err("shards must be a power of two in 1..=256".to_owned());
    }
    if n == 0 {
        return Ok(RecursiveTailResult {
            contraction: ContractionResult {
                n,
                count: 1,
                elapsed: Duration::ZERO,
                peak_states: 1,
                tensor_entries_examined: 17,
                tensor_entries_matched: 17,
                row_operator_candidates: 0,
                row_operator_matched: 0,
                peak_rss_bytes: peak_rss_bytes(),
                layers: Vec::new(),
            },
            cut,
            prefix_elapsed: Duration::ZERO,
            tail_elapsed: Duration::ZERO,
            prefix_support: 1,
            tail_tasks: 1,
            recursive_nodes: 1,
            recursive_accepted_entries: 0,
            coefficient_bits: 64,
            max_prefix_coefficient: 1,
            generation_elapsed: Duration::ZERO,
            sort_elapsed: Duration::ZERO,
            reduce_elapsed: Duration::ZERO,
            peak_thread_local_bytes: 0,
        });
    }

    let total_start = Instant::now();
    let coefficient_bits = 64_u32
        .checked_sub(
            u32::try_from(3_usize.saturating_mul(n))
                .map_err(|_| "recursive-tail boundary width does not fit u32".to_owned())?,
        )
        .ok_or_else(|| "recursive-tail packing requires N <= 21".to_owned())?;
    let coefficient_mask = coefficient_mask(coefficient_bits);
    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let relation = RecursiveTailRelation::compile(&operator)?;
    let board_mask = (1_u64 << n) - 1;

    let (mut prefix, boundary, max_prefix_coefficient) =
        if cut == 0 {
            let initial = BoundaryState {
                columns: 0,
                diag_dr: 0,
                diag_dl: 0,
            };
            let key = u64::try_from(PackedBoundary::pack(initial, n).0)
                .map_err(|_| "initial recursive-tail key does not fit u64".to_owned())?;
            let mut boundary = (0..shards)
                .map(|_| Vec::<JointEntry>::new())
                .collect::<Vec<_>>();
            boundary[shard_index(u128::from(key), n, shards, ShardMode::Prefix)]
                .push(JointEntry::new(key, 1, coefficient_bits));
            (
                ParallelGenerationResult {
                    contraction: ContractionResult {
                        n,
                        count: 0,
                        elapsed: Duration::ZERO,
                        peak_states: 1,
                        tensor_entries_examined: 17,
                        tensor_entries_matched: 17,
                        row_operator_candidates: 0,
                        row_operator_matched: 0,
                        peak_rss_bytes: peak_rss_bytes(),
                        layers: Vec::new(),
                    },
                    generation_elapsed: Duration::ZERO,
                    sort_elapsed: Duration::ZERO,
                    reduce_elapsed: Duration::ZERO,
                    peak_thread_local_bytes: 0,
                    peak_worker_partials: 0,
                },
                boundary,
                1,
            )
        } else {
            let prefix = contract_rows_d4_joint_u64_kernel(n, shards, coefficient_bits, true, cut)?;
            (
                prefix.parallel,
                prefix.boundary,
                prefix.max_coefficient_observed,
            )
        };
    let prefix_elapsed = total_start.elapsed();
    let prefix_support = boundary.iter().map(Vec::len).sum::<usize>();

    let tail_start = Instant::now();
    let partials = boundary
        .par_iter()
        .flat_map_iter(|shard| shard.iter().copied())
        .map(|entry| {
            let key = entry.key(coefficient_bits);
            let weight = entry.weight(coefficient_mask);
            let mut metrics = RecursiveTailMetrics::default();
            let completions = contract_recursive_tail(
                n,
                cut,
                PackedBoundary(u128::from(key)).unpack(n),
                relation,
                board_mask,
                &mut metrics,
            )?;
            let weighted = completions.checked_mul(u128::from(weight)).ok_or_else(|| {
                "coefficient overflow joining prefix and recursive tail".to_owned()
            })?;
            Ok::<_, String>((weighted, metrics))
        })
        .collect::<Vec<_>>();
    let mut count = 0_u128;
    let mut recursive_nodes = 0_u128;
    let mut recursive_accepted_entries = 0_u128;
    for partial in partials {
        let (weighted, metrics) = partial?;
        count = count
            .checked_add(weighted)
            .ok_or_else(|| "coefficient overflow reducing recursive-tail tasks".to_owned())?;
        recursive_nodes += metrics.nodes;
        recursive_accepted_entries += metrics.accepted_entries;
    }
    let tail_elapsed = tail_start.elapsed();

    prefix.contraction.count = count;
    prefix.contraction.elapsed = total_start.elapsed();
    prefix.contraction.peak_rss_bytes = prefix.contraction.peak_rss_bytes.max(peak_rss_bytes());
    prefix.contraction.row_operator_candidates += recursive_accepted_entries;
    prefix.contraction.row_operator_matched += recursive_accepted_entries;
    Ok(RecursiveTailResult {
        contraction: prefix.contraction,
        cut,
        prefix_elapsed,
        tail_elapsed,
        prefix_support,
        tail_tasks: prefix_support,
        recursive_nodes,
        recursive_accepted_entries,
        coefficient_bits,
        max_prefix_coefficient,
        generation_elapsed: prefix.generation_elapsed,
        sort_elapsed: prefix.sort_elapsed,
        reduce_elapsed: prefix.reduce_elapsed,
        peak_thread_local_bytes: prefix.peak_thread_local_bytes,
    })
}

#[derive(Clone, Copy, Debug)]
struct CertifiedSecViTailPlan;

impl CertifiedSecViTailPlan {
    fn compile(relation: RecursiveTailRelation) -> Result<Self, String> {
        let expected = VirtualLegs {
            column_in: 0,
            column_out: 1,
            row_in: 0,
            row_out: 1,
            diag_dr_in: 0,
            diag_dr_out: 1,
            diag_dl_in: 0,
            diag_dl_out: 1,
        };
        if relation.legs != expected || relation.value != 1 {
            return Err(
                "certified tail fast path requires the explicit Sec. VI occupied C entry"
                    .to_owned(),
            );
        }
        Ok(Self)
    }
}

#[inline]
fn contract_certified_tail_u64(
    remaining_rows: usize,
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    board_mask: u64,
    coefficient_limit: u64,
) -> Option<u64> {
    if remaining_rows == 0 {
        // Contract the column endpoints with v1. Diagonal endpoints use v2
        // and therefore require no filter.
        return Some(u64::from(columns == board_mask));
    }
    let mut positions = !(columns | diag_dr | diag_dl) & board_mask;
    if remaining_rows == 1 {
        // Exactly one unused column remains. Any available occupied C entry
        // completes column v1, while both diagonal outputs are accepted by v2.
        return Some(u64::from(positions != 0));
    }
    let mut count = 0_u64;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        positions &= positions - 1;
        let child = contract_certified_tail_u64(
            remaining_rows - 1,
            columns | selected,
            ((diag_dr | selected) << 1) & board_mask,
            (diag_dl | selected) >> 1,
            board_mask,
            coefficient_limit,
        )?;
        count = count
            .checked_add(child)
            .filter(|&value| value <= coefficient_limit)?;
    }
    Some(count)
}

#[inline(always)]
fn certified_tail_successor(
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    selected: u64,
    board_mask: u64,
) -> (u64, u64, u64) {
    (
        columns | selected,
        ((diag_dr | selected) << 1) & board_mask,
        (diag_dl | selected) >> 1,
    )
}

#[cfg_attr(feature = "e55-noinline", inline(never))]
#[cfg_attr(not(feature = "e55-noinline"), inline)]
fn contract_certified_last_four_u64(
    remaining_rows: usize,
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    board_mask: u64,
    coefficient_limit: u64,
) -> Option<u64> {
    debug_assert!((2..=4).contains(&remaining_rows));
    let mut count = 0_u64;
    let mut first = !(columns | diag_dr | diag_dl) & board_mask;
    while first != 0 {
        let q1 = first & first.wrapping_neg();
        first &= first - 1;
        let (columns1, diag_dr1, diag_dl1) =
            certified_tail_successor(columns, diag_dr, diag_dl, q1, board_mask);
        let mut second = !(columns1 | diag_dr1 | diag_dl1) & board_mask;
        if remaining_rows == 2 {
            count = count
                .checked_add(u64::from(second != 0))
                .filter(|&value| value <= coefficient_limit)?;
            continue;
        }
        while second != 0 {
            let q2 = second & second.wrapping_neg();
            second &= second - 1;
            let (columns2, diag_dr2, diag_dl2) =
                certified_tail_successor(columns1, diag_dr1, diag_dl1, q2, board_mask);
            let mut third = !(columns2 | diag_dr2 | diag_dl2) & board_mask;
            if remaining_rows == 3 {
                count = count
                    .checked_add(u64::from(third != 0))
                    .filter(|&value| value <= coefficient_limit)?;
                continue;
            }
            while third != 0 {
                let q3 = third & third.wrapping_neg();
                third &= third - 1;
                let (columns3, diag_dr3, diag_dl3) =
                    certified_tail_successor(columns2, diag_dr2, diag_dl2, q3, board_mask);
                let fourth = !(columns3 | diag_dr3 | diag_dl3) & board_mask;
                count = count
                    .checked_add(u64::from(fourth != 0))
                    .filter(|&value| value <= coefficient_limit)?;
            }
        }
    }
    Some(count)
}

#[cfg_attr(feature = "e55-noinline", inline(never))]
#[cfg_attr(feature = "e55-regular-inline", inline)]
#[cfg_attr(
    not(any(feature = "e55-noinline", feature = "e55-regular-inline")),
    inline(always)
)]
fn contract_certified_last_five_u64(
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    board_mask: u64,
    coefficient_limit: u64,
) -> Option<u64> {
    let mut count = 0_u64;
    let mut first = !(columns | diag_dr | diag_dl) & board_mask;
    while first != 0 {
        let selected = first & first.wrapping_neg();
        first &= first - 1;
        let (next_columns, next_diag_dr, next_diag_dl) =
            certified_tail_successor(columns, diag_dr, diag_dl, selected, board_mask);
        let child = contract_certified_last_four_u64(
            4,
            next_columns,
            next_diag_dr,
            next_diag_dl,
            board_mask,
            coefficient_limit,
        )?;
        count = count
            .checked_add(child)
            .filter(|&value| value <= coefficient_limit)?;
    }
    Some(count)
}

#[cfg_attr(feature = "e55-noinline", inline(never))]
#[cfg_attr(feature = "e55-regular-inline", inline)]
#[cfg_attr(
    not(any(feature = "e55-noinline", feature = "e55-regular-inline")),
    inline(always)
)]
fn contract_certified_last_six_u64(
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    board_mask: u64,
    coefficient_limit: u64,
) -> Option<u64> {
    let mut count = 0_u64;
    let mut first = !(columns | diag_dr | diag_dl) & board_mask;
    while first != 0 {
        let selected = first & first.wrapping_neg();
        first &= first - 1;
        let (next_columns, next_diag_dr, next_diag_dl) =
            certified_tail_successor(columns, diag_dr, diag_dl, selected, board_mask);
        let child = contract_certified_last_five_u64(
            next_columns,
            next_diag_dr,
            next_diag_dl,
            board_mask,
            coefficient_limit,
        )?;
        count = count
            .checked_add(child)
            .filter(|&value| value <= coefficient_limit)?;
    }
    Some(count)
}

#[inline]
fn contract_certified_tail_last_k_u64<const LAST_K: usize>(
    remaining_rows: usize,
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    board_mask: u64,
    coefficient_limit: u64,
) -> Option<u64> {
    if remaining_rows == 0 {
        return Some(u64::from(columns == board_mask));
    }
    let positions = !(columns | diag_dr | diag_dl) & board_mask;
    if remaining_rows == 1 {
        return Some(u64::from(positions != 0));
    }
    if remaining_rows <= LAST_K {
        return match remaining_rows {
            2..=4 => contract_certified_last_four_u64(
                remaining_rows,
                columns,
                diag_dr,
                diag_dl,
                board_mask,
                coefficient_limit,
            ),
            5 => contract_certified_last_five_u64(
                columns,
                diag_dr,
                diag_dl,
                board_mask,
                coefficient_limit,
            ),
            6 => contract_certified_last_six_u64(
                columns,
                diag_dr,
                diag_dl,
                board_mask,
                coefficient_limit,
            ),
            _ => None,
        };
    }
    let mut positions = positions;
    let mut count = 0_u64;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        positions &= positions - 1;
        let (next_columns, next_diag_dr, next_diag_dl) =
            certified_tail_successor(columns, diag_dr, diag_dl, selected, board_mask);
        let child = contract_certified_tail_last_k_u64::<LAST_K>(
            remaining_rows - 1,
            next_columns,
            next_diag_dr,
            next_diag_dl,
            board_mask,
            coefficient_limit,
        )?;
        count = count
            .checked_add(child)
            .filter(|&value| value <= coefficient_limit)?;
    }
    Some(count)
}

#[allow(clippy::too_many_arguments)]
fn contract_certified_tail_tasks_u64(
    tasks: &[JointEntry],
    n: usize,
    cut: usize,
    coefficient_bits: u32,
    coefficient_mask: u64,
    coefficient_limit: u64,
    board_mask: u64,
    microkernel_rows: usize,
) -> Option<u64> {
    let contract_task: fn(usize, u64, u64, u64, u64, u64) -> Option<u64> = match microkernel_rows {
        0 | 1 => contract_certified_tail_u64,
        2 => contract_certified_tail_last_k_u64::<2>,
        3 => contract_certified_tail_last_k_u64::<3>,
        4 => contract_certified_tail_last_k_u64::<4>,
        _ => return None,
    };
    let worker_count = rayon::current_num_threads().max(1).min(tasks.len().max(1));
    if worker_count == 1 {
        let mut total = 0_u64;
        for &entry in tasks {
            let state = PackedBoundary(u128::from(entry.key(coefficient_bits))).unpack(n);
            let completions = contract_task(
                n - cut,
                state.columns,
                state.diag_dr,
                state.diag_dl,
                board_mask,
                coefficient_limit,
            )?;
            let weighted = completions
                .checked_mul(entry.weight(coefficient_mask))
                .filter(|&value| value <= coefficient_limit)?;
            total = total
                .checked_add(weighted)
                .filter(|&value| value <= coefficient_limit)?;
        }
        return Some(total);
    }

    let next_task = AtomicUsize::new(0);
    let partials = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            handles.push(scope.spawn(|| {
                let mut subtotal = 0_u64;
                const TASK_CHUNK: usize = 16;
                loop {
                    let start = next_task.fetch_add(TASK_CHUNK, Ordering::Relaxed);
                    if start >= tasks.len() {
                        break;
                    }
                    let end = (start + TASK_CHUNK).min(tasks.len());
                    for &entry in &tasks[start..end] {
                        let state =
                            PackedBoundary(u128::from(entry.key(coefficient_bits))).unpack(n);
                        let completions = contract_task(
                            n - cut,
                            state.columns,
                            state.diag_dr,
                            state.diag_dl,
                            board_mask,
                            coefficient_limit,
                        )?;
                        let weighted = completions
                            .checked_mul(entry.weight(coefficient_mask))
                            .filter(|&value| value <= coefficient_limit)?;
                        subtotal = subtotal
                            .checked_add(weighted)
                            .filter(|&value| value <= coefficient_limit)?;
                    }
                }
                Some(subtotal)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("certified tail worker panicked"))
            .collect::<Vec<_>>()
    });
    let mut total = 0_u64;
    for partial in partials {
        total = total
            .checked_add(partial?)
            .filter(|&value| value <= coefficient_limit)?;
    }
    Some(total)
}

#[derive(Clone, Debug)]
pub struct CertifiedFastTailResult {
    pub contraction: ContractionResult,
    pub cut: usize,
    pub used_u64_fast_path: bool,
    pub promotion_reason: Option<String>,
    pub prefix_elapsed: Duration,
    pub tail_elapsed: Duration,
    pub profile_replay_elapsed: Duration,
    pub prefix_support: usize,
    pub tail_tasks: usize,
    pub recursive_nodes: u128,
    pub recursive_accepted_entries: u128,
    pub coefficient_bits: u32,
    pub max_prefix_coefficient: u64,
}

pub fn contract_rows_certified_fast_tail(
    n: usize,
    shards: usize,
    cut: usize,
    profile_replay: bool,
) -> Result<CertifiedFastTailResult, String> {
    contract_rows_certified_fast_tail_with_limit(n, shards, cut, profile_replay, u64::MAX)
}

#[derive(Clone, Debug)]
pub struct AdaptiveCutProbe {
    pub cut: usize,
    pub prefix_support: usize,
    pub prefix_elapsed: Duration,
    pub prefix_accepted_entries: u128,
}

#[derive(Clone, Debug)]
pub struct AdaptiveFastTailResult {
    pub fast: CertifiedFastTailResult,
    pub selected_cut: usize,
    pub target_tail_tasks: usize,
    pub selection_elapsed: Duration,
    pub probes: Vec<AdaptiveCutProbe>,
}

pub fn contract_rows_adaptive_fast_tail(
    n: usize,
    shards: usize,
    profile_replay: bool,
) -> Result<AdaptiveFastTailResult, String> {
    contract_rows_adaptive_fast_tail_impl(n, shards, profile_replay, 0, u64::MAX)
}

pub fn contract_rows_adaptive_last_k_tail(
    n: usize,
    shards: usize,
    profile_replay: bool,
) -> Result<AdaptiveFastTailResult, String> {
    contract_rows_adaptive_last_k_tail_with_rows(n, shards, profile_replay, 4)
}

pub fn contract_rows_adaptive_last_k_tail_with_rows(
    n: usize,
    shards: usize,
    profile_replay: bool,
    microkernel_rows: usize,
) -> Result<AdaptiveFastTailResult, String> {
    if !(2..=4).contains(&microkernel_rows) {
        return Err("last-k microkernel rows must be in 2..=4".to_owned());
    }
    contract_rows_adaptive_fast_tail_impl(n, shards, profile_replay, microkernel_rows, u64::MAX)
}

const CRT_PRIME_CANDIDATES: [u64; 4] = [4_294_967_291, 4_294_967_279, 4_294_967_231, 4_294_967_197];

#[derive(Clone, Debug)]
pub struct WideCrtPlan {
    pub n: usize,
    pub factorial_bound: u128,
    pub primes: Vec<u64>,
    pub modulus_product: u128,
}

#[derive(Clone, Copy, Debug)]
struct WideCrtTask {
    state: BoundaryState,
    orbit_weight: u64,
}

#[derive(Clone, Debug)]
pub struct WideCrtPrefixPlan {
    pub plan: WideCrtPlan,
    pub split_depth: usize,
    pub target_tail_tasks: usize,
    pub tail_tasks: usize,
    pub prefix_nodes: u128,
    pub prefix_accepted_entries: u128,
    pub prefix_kept_entries: u128,
    pub seed_elapsed: Duration,
    pub peak_rss_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct WideCrtResult {
    pub contraction: ContractionResult,
    pub plan: WideCrtPlan,
    pub split_depth: usize,
    pub target_tail_tasks: usize,
    pub tail_tasks: usize,
    pub prefix_nodes: u128,
    pub prefix_accepted_entries: u128,
    pub prefix_kept_entries: u128,
    pub recursive_nodes: u128,
    pub recursive_accepted_entries: u128,
    pub seed_elapsed: Duration,
    pub tail_elapsed: Duration,
    pub profile_replay_elapsed: Duration,
    pub residues: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct WideScalarResult {
    pub contraction: ContractionResult,
    pub plan: WideCrtPlan,
    pub split_depth: usize,
    pub target_tail_tasks: usize,
    pub tail_tasks: usize,
    pub prefix_nodes: u128,
    pub prefix_accepted_entries: u128,
    pub prefix_kept_entries: u128,
    pub recursive_nodes: u128,
    pub recursive_accepted_entries: u128,
    pub seed_elapsed: Duration,
    pub tail_elapsed: Duration,
    pub profile_replay_elapsed: Duration,
    pub used_scalar_u64: bool,
    pub promotion_reason: Option<String>,
    pub residues: Vec<u64>,
}

pub const fn e55_hot_code_shape() -> &'static str {
    #[cfg(feature = "e55-noinline")]
    {
        "noinline"
    }
    #[cfg(all(not(feature = "e55-noinline"), feature = "e55-regular-inline"))]
    {
        "regular-inline"
    }
    #[cfg(not(any(feature = "e55-noinline", feature = "e55-regular-inline")))]
    {
        "fully-inline"
    }
}

fn is_prime_u32_range(value: u64) -> bool {
    if value < 2 {
        return false;
    }
    if value.is_multiple_of(2) {
        return value == 2;
    }
    let mut divisor = 3_u64;
    while divisor * divisor <= value {
        if value.is_multiple_of(divisor) {
            return false;
        }
        divisor += 2;
    }
    true
}

fn factorial_u128(n: usize) -> Result<u128, String> {
    (2..=n).try_fold(1_u128, |product, factor| {
        product
            .checked_mul(factor as u128)
            .ok_or_else(|| "N! exactness bound does not fit u128".to_owned())
    })
}

pub fn wide_crt_plan(n: usize) -> Result<WideCrtPlan, String> {
    if n > 34 {
        return Err("wide CRT backend supports N <= 34 because 35! exceeds u128".to_owned());
    }
    let factorial_bound = factorial_u128(n)?;
    let mut primes = Vec::new();
    let mut modulus_product = 1_u128;
    for prime in CRT_PRIME_CANDIDATES {
        if !is_prime_u32_range(prime) {
            return Err(format!(
                "CRT modulus {prime} failed deterministic primality check"
            ));
        }
        primes.push(prime);
        modulus_product = modulus_product
            .checked_mul(u128::from(prime))
            .ok_or_else(|| "CRT modulus product overflow".to_owned())?;
        if modulus_product > factorial_bound {
            return Ok(WideCrtPlan {
                n,
                factorial_bound,
                primes,
                modulus_product,
            });
        }
    }
    Err("available certified CRT product does not exceed N! bound".to_owned())
}

fn build_wide_crt_tasks(
    n: usize,
    relation: RecursiveTailRelation,
    board_mask: u64,
    target_tail_tasks: usize,
) -> Result<(Vec<WideCrtTask>, usize, u128, u128, u128), String> {
    let mut tasks = vec![WideCrtTask {
        state: BoundaryState {
            columns: 0,
            diag_dr: 0,
            diag_dl: 0,
        },
        orbit_weight: 1,
    }];
    let mut split_depth = 0_usize;
    let mut prefix_nodes = 0_u128;
    let mut prefix_accepted_entries = 0_u128;
    let mut prefix_kept_entries = 0_u128;
    while split_depth < n && tasks.len() < target_tail_tasks {
        let top_row = split_depth == 0;
        let mut next = Vec::with_capacity(tasks.len().saturating_mul(n - split_depth));
        for task in tasks {
            prefix_nodes = prefix_nodes
                .checked_add(1)
                .ok_or_else(|| "wide CRT prefix node counter overflow".to_owned())?;
            let mut positions = recursive_tail_positions(task.state, relation, board_mask);
            while positions != 0 {
                let selected = positions & positions.wrapping_neg();
                positions &= positions - 1;
                prefix_accepted_entries = prefix_accepted_entries
                    .checked_add(1)
                    .ok_or_else(|| "wide CRT prefix accepted counter overflow".to_owned())?;
                let successor =
                    recursive_tail_successor(task.state, selected, relation, board_mask);
                let orbit_weight = if top_row {
                    let Some(weight) = top_row_vertical_orbit_weight(n, successor) else {
                        continue;
                    };
                    u64::try_from(weight)
                        .map_err(|_| "wide CRT orbit weight does not fit u64".to_owned())?
                } else {
                    1
                };
                let local_value = u64::try_from(relation.value)
                    .map_err(|_| "wide CRT local C value does not fit u64".to_owned())?;
                next.push(WideCrtTask {
                    state: successor,
                    orbit_weight: task
                        .orbit_weight
                        .checked_mul(orbit_weight)
                        .and_then(|value| value.checked_mul(local_value))
                        .ok_or_else(|| "wide CRT prefix weight overflow".to_owned())?,
                });
                prefix_kept_entries = prefix_kept_entries
                    .checked_add(1)
                    .ok_or_else(|| "wide CRT prefix kept counter overflow".to_owned())?;
            }
        }
        tasks = next;
        split_depth += 1;
    }
    Ok((
        tasks,
        split_depth,
        prefix_nodes,
        prefix_accepted_entries,
        prefix_kept_entries,
    ))
}

#[inline]
fn add_residue(left: u64, right: u64, prime: u64) -> u64 {
    let sum = left + right;
    if sum >= prime { sum - prime } else { sum }
}

fn contract_wide_crt_tail<const LANES: usize>(
    remaining_rows: usize,
    columns: u64,
    diag_dr: u64,
    diag_dl: u64,
    board_mask: u64,
    primes: &[u64; LANES],
) -> [u64; LANES] {
    if remaining_rows <= 4 {
        let exact = contract_certified_tail_last_k_u64::<4>(
            remaining_rows,
            columns,
            diag_dr,
            diag_dl,
            board_mask,
            u64::MAX,
        )
        .expect("four-row exact microkernel cannot overflow u64");
        return std::array::from_fn(|lane| exact % primes[lane]);
    }
    let mut total = [0_u64; LANES];
    let mut positions = !(columns | diag_dr | diag_dl) & board_mask;
    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        positions &= positions - 1;
        let (next_columns, next_diag_dr, next_diag_dl) =
            certified_tail_successor(columns, diag_dr, diag_dl, selected, board_mask);
        let child = contract_wide_crt_tail(
            remaining_rows - 1,
            next_columns,
            next_diag_dr,
            next_diag_dl,
            board_mask,
            primes,
        );
        for lane in 0..LANES {
            total[lane] = add_residue(total[lane], child[lane], primes[lane]);
        }
    }
    total
}

fn contract_wide_crt_tasks<const LANES: usize>(
    tasks: &[WideCrtTask],
    n: usize,
    split_depth: usize,
    board_mask: u64,
    primes: &[u64; LANES],
) -> [u64; LANES] {
    let worker_count = rayon::current_num_threads().max(1).min(tasks.len().max(1));
    let next_task = AtomicUsize::new(0);
    let partials = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            handles.push(scope.spawn(|| {
                let mut subtotal = [0_u64; LANES];
                const TASK_CHUNK: usize = 16;
                loop {
                    let start = next_task.fetch_add(TASK_CHUNK, Ordering::Relaxed);
                    if start >= tasks.len() {
                        break;
                    }
                    let end = (start + TASK_CHUNK).min(tasks.len());
                    for task in &tasks[start..end] {
                        let residues = contract_wide_crt_tail(
                            n - split_depth,
                            task.state.columns,
                            task.state.diag_dr,
                            task.state.diag_dl,
                            board_mask,
                            primes,
                        );
                        for lane in 0..LANES {
                            let weighted = ((u128::from(residues[lane])
                                * u128::from(task.orbit_weight))
                                % u128::from(primes[lane]))
                                as u64;
                            subtotal[lane] = add_residue(subtotal[lane], weighted, primes[lane]);
                        }
                    }
                }
                subtotal
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("wide CRT worker panicked"))
            .collect::<Vec<_>>()
    });
    let mut total = [0_u64; LANES];
    for partial in partials {
        for lane in 0..LANES {
            total[lane] = add_residue(total[lane], partial[lane], primes[lane]);
        }
    }
    total
}

fn contract_wide_scalar_tasks(
    tasks: &[WideCrtTask],
    n: usize,
    split_depth: usize,
    board_mask: u64,
    coefficient_limit: u64,
    microkernel_rows: usize,
) -> Option<u64> {
    let contract_task: fn(usize, u64, u64, u64, u64, u64) -> Option<u64> = match microkernel_rows {
        4 => contract_certified_tail_last_k_u64::<4>,
        5 => contract_certified_tail_last_k_u64::<5>,
        6 => contract_certified_tail_last_k_u64::<6>,
        _ => return None,
    };
    let worker_count = rayon::current_num_threads().max(1).min(tasks.len().max(1));
    let next_task = AtomicUsize::new(0);
    let partials = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            handles.push(scope.spawn(|| {
                let mut subtotal = 0_u64;
                const TASK_CHUNK: usize = 16;
                loop {
                    let start = next_task.fetch_add(TASK_CHUNK, Ordering::Relaxed);
                    if start >= tasks.len() {
                        break;
                    }
                    let end = (start + TASK_CHUNK).min(tasks.len());
                    for task in &tasks[start..end] {
                        let completions = contract_task(
                            n - split_depth,
                            task.state.columns,
                            task.state.diag_dr,
                            task.state.diag_dl,
                            board_mask,
                            coefficient_limit,
                        )?;
                        let weighted = completions
                            .checked_mul(task.orbit_weight)
                            .filter(|&value| value <= coefficient_limit)?;
                        subtotal = subtotal
                            .checked_add(weighted)
                            .filter(|&value| value <= coefficient_limit)?;
                    }
                }
                Some(subtotal)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("wide scalar worker panicked"))
            .collect::<Vec<_>>()
    });
    partials.into_iter().try_fold(0_u64, |total, partial| {
        total
            .checked_add(partial?)
            .filter(|&value| value <= coefficient_limit)
    })
}

fn mod_pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = ((u128::from(result) * u128::from(base)) % u128::from(modulus)) as u64;
        }
        base = ((u128::from(base) * u128::from(base)) % u128::from(modulus)) as u64;
        exponent >>= 1;
    }
    result
}

fn reconstruct_crt(residues: &[u64], primes: &[u64]) -> Result<u128, String> {
    if residues.len() != primes.len() || residues.is_empty() {
        return Err("CRT residue/modulus length mismatch".to_owned());
    }
    let mut value = 0_u128;
    let mut modulus_product = 1_u128;
    for (&residue, &prime) in residues.iter().zip(primes) {
        let prime_u128 = u128::from(prime);
        let value_mod = (value % prime_u128) as u64;
        let delta = if residue >= value_mod {
            residue - value_mod
        } else {
            residue + prime - value_mod
        };
        let product_mod = (modulus_product % prime_u128) as u64;
        let inverse = mod_pow(product_mod, prime - 2, prime);
        let step = ((u128::from(delta) * u128::from(inverse)) % prime_u128) as u64;
        value = value
            .checked_add(
                modulus_product
                    .checked_mul(u128::from(step))
                    .ok_or_else(|| "CRT reconstruction product overflow".to_owned())?,
            )
            .ok_or_else(|| "CRT reconstruction sum overflow".to_owned())?;
        modulus_product = modulus_product
            .checked_mul(prime_u128)
            .ok_or_else(|| "CRT reconstruction modulus overflow".to_owned())?;
    }
    Ok(value)
}

fn contract_wide_crt_residues(
    tasks: &[WideCrtTask],
    n: usize,
    split_depth: usize,
    board_mask: u64,
    primes: &[u64],
) -> Result<Vec<u64>, String> {
    let mut padded = [0_u64; 4];
    padded[..primes.len()].copy_from_slice(primes);
    Ok(match primes.len() {
        1 => contract_wide_crt_tasks(tasks, n, split_depth, board_mask, &[padded[0]]).to_vec(),
        2 => contract_wide_crt_tasks(tasks, n, split_depth, board_mask, &[padded[0], padded[1]])
            .to_vec(),
        3 => contract_wide_crt_tasks(
            tasks,
            n,
            split_depth,
            board_mask,
            &[padded[0], padded[1], padded[2]],
        )
        .to_vec(),
        4 => contract_wide_crt_tasks(tasks, n, split_depth, board_mask, &padded).to_vec(),
        _ => return Err("wide CRT lane count must be in 1..=4".to_owned()),
    })
}

fn prepare_wide_crt_prefix(
    n: usize,
    target_tasks_per_thread: usize,
) -> Result<
    (
        WideCrtPlan,
        RecursiveTailRelation,
        Vec<WideCrtTask>,
        WideCrtPrefixPlan,
    ),
    String,
> {
    if target_tasks_per_thread == 0 {
        return Err("wide CRT target tasks per thread must be positive".to_owned());
    }
    let plan = wide_crt_plan(n)?;
    if n == 0 {
        let prefix = WideCrtPrefixPlan {
            plan: plan.clone(),
            split_depth: 0,
            target_tail_tasks: 1,
            tail_tasks: 1,
            prefix_nodes: 0,
            prefix_accepted_entries: 0,
            prefix_kept_entries: 0,
            seed_elapsed: Duration::ZERO,
            peak_rss_bytes: peak_rss_bytes(),
        };
        return Ok((
            plan,
            RecursiveTailRelation::compile(&CompiledRowOperator::compile(&SiteTensorC::sec_vi())?)?,
            vec![WideCrtTask {
                state: BoundaryState {
                    columns: 0,
                    diag_dr: 0,
                    diag_dl: 0,
                },
                orbit_weight: 1,
            }],
            prefix,
        ));
    }
    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let relation = RecursiveTailRelation::compile(&operator)?;
    let _certified = CertifiedSecViTailPlan::compile(relation)?;
    let board_mask = (1_u64 << n) - 1;
    let target_tail_tasks = rayon::current_num_threads()
        .max(1)
        .saturating_mul(target_tasks_per_thread);
    let seed_start = Instant::now();
    let (tasks, split_depth, prefix_nodes, prefix_accepted_entries, prefix_kept_entries) =
        build_wide_crt_tasks(n, relation, board_mask, target_tail_tasks)?;
    let prefix = WideCrtPrefixPlan {
        plan: plan.clone(),
        split_depth,
        target_tail_tasks,
        tail_tasks: tasks.len(),
        prefix_nodes,
        prefix_accepted_entries,
        prefix_kept_entries,
        seed_elapsed: seed_start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
    };
    Ok((plan, relation, tasks, prefix))
}

pub fn probe_wide_crt_prefix(n: usize) -> Result<WideCrtPrefixPlan, String> {
    let (_, _, _, prefix) = prepare_wide_crt_prefix(n, 512)?;
    Ok(prefix)
}

pub fn contract_rows_wide_crt(n: usize, profile_replay: bool) -> Result<WideCrtResult, String> {
    contract_rows_wide_crt_with_target(n, profile_replay, 512)
}

pub fn contract_rows_wide_crt_with_target(
    n: usize,
    profile_replay: bool,
    target_tasks_per_thread: usize,
) -> Result<WideCrtResult, String> {
    let total_start = Instant::now();
    let (plan, relation, tasks, prefix) = prepare_wide_crt_prefix(n, target_tasks_per_thread)?;
    let board_mask = if n == 0 { 0 } else { (1_u64 << n) - 1 };
    let tail_start = Instant::now();
    let residues =
        contract_wide_crt_residues(&tasks, n, prefix.split_depth, board_mask, &plan.primes)?;
    let tail_elapsed = tail_start.elapsed();
    let elapsed = total_start.elapsed();
    let count = reconstruct_crt(&residues, &plan.primes)?;
    if count > plan.factorial_bound {
        return Err("CRT reconstruction exceeds certified N! count bound".to_owned());
    }

    let profile_start = Instant::now();
    let mut recursive_nodes = 0_u128;
    let mut recursive_accepted_entries = 0_u128;
    let mut replay_count = 0_u128;
    if profile_replay {
        let partials = tasks
            .par_iter()
            .map(|task| {
                let mut metrics = RecursiveTailMetrics::default();
                let completions = contract_recursive_tail(
                    n,
                    prefix.split_depth,
                    task.state,
                    relation,
                    board_mask,
                    &mut metrics,
                )?;
                let weighted = completions
                    .checked_mul(u128::from(task.orbit_weight))
                    .ok_or_else(|| "wide CRT profile task weight overflow".to_owned())?;
                Ok::<_, String>((weighted, metrics))
            })
            .collect::<Vec<_>>();
        for partial in partials {
            let (weighted, metrics) = partial?;
            replay_count = replay_count
                .checked_add(weighted)
                .ok_or_else(|| "wide CRT profile reduction overflow".to_owned())?;
            recursive_nodes = recursive_nodes
                .checked_add(metrics.nodes)
                .ok_or_else(|| "wide CRT recursive node counter overflow".to_owned())?;
            recursive_accepted_entries = recursive_accepted_entries
                .checked_add(metrics.accepted_entries)
                .ok_or_else(|| "wide CRT accepted-entry counter overflow".to_owned())?;
        }
        if replay_count != count {
            return Err("wide CRT reconstruction disagrees with generic C replay".to_owned());
        }
    }
    let profile_replay_elapsed = profile_start.elapsed();
    let total_accepted = prefix
        .prefix_accepted_entries
        .checked_add(recursive_accepted_entries)
        .ok_or_else(|| "wide CRT total accepted-entry counter overflow".to_owned())?;
    Ok(WideCrtResult {
        contraction: ContractionResult {
            n,
            count,
            elapsed,
            peak_states: tasks.len().max(1),
            tensor_entries_examined: 17,
            tensor_entries_matched: 17,
            row_operator_candidates: total_accepted,
            row_operator_matched: total_accepted,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        },
        plan,
        split_depth: prefix.split_depth,
        target_tail_tasks: prefix.target_tail_tasks,
        tail_tasks: tasks.len(),
        prefix_nodes: prefix.prefix_nodes,
        prefix_accepted_entries: prefix.prefix_accepted_entries,
        prefix_kept_entries: prefix.prefix_kept_entries,
        recursive_nodes,
        recursive_accepted_entries,
        seed_elapsed: prefix.seed_elapsed,
        tail_elapsed,
        profile_replay_elapsed,
        residues,
    })
}

pub fn contract_rows_wide_scalar(
    n: usize,
    profile_replay: bool,
) -> Result<WideScalarResult, String> {
    contract_rows_wide_scalar_with_target_and_limit(n, profile_replay, 512, u64::MAX)
}

pub fn contract_rows_wide_scalar_with_target(
    n: usize,
    profile_replay: bool,
    target_tasks_per_thread: usize,
) -> Result<WideScalarResult, String> {
    contract_rows_wide_scalar_with_target_and_limit(
        n,
        profile_replay,
        target_tasks_per_thread,
        u64::MAX,
    )
}

pub fn contract_rows_wide_scalar_with_target_and_limit(
    n: usize,
    profile_replay: bool,
    target_tasks_per_thread: usize,
    coefficient_limit: u64,
) -> Result<WideScalarResult, String> {
    contract_rows_wide_scalar_last_k_impl(
        n,
        profile_replay,
        target_tasks_per_thread,
        coefficient_limit,
        4,
    )
}

pub fn contract_rows_wide_scalar_last_k_with_target(
    n: usize,
    profile_replay: bool,
    target_tasks_per_thread: usize,
    microkernel_rows: usize,
) -> Result<WideScalarResult, String> {
    if !(4..=6).contains(&microkernel_rows) {
        return Err("wide scalar last-k rows must be in 4..=6".to_owned());
    }
    contract_rows_wide_scalar_last_k_impl(
        n,
        profile_replay,
        target_tasks_per_thread,
        u64::MAX,
        microkernel_rows,
    )
}

fn contract_rows_wide_scalar_last_k_impl(
    n: usize,
    profile_replay: bool,
    target_tasks_per_thread: usize,
    coefficient_limit: u64,
    microkernel_rows: usize,
) -> Result<WideScalarResult, String> {
    let total_start = Instant::now();
    let (plan, relation, tasks, prefix) = prepare_wide_crt_prefix(n, target_tasks_per_thread)?;
    let board_mask = if n == 0 { 0 } else { (1_u64 << n) - 1 };
    let scalar_bound_certified = plan.factorial_bound <= u128::from(u64::MAX);
    let tail_start = Instant::now();
    let scalar_count = scalar_bound_certified.then(|| {
        contract_wide_scalar_tasks(
            &tasks,
            n,
            prefix.split_depth,
            board_mask,
            coefficient_limit,
            microkernel_rows,
        )
    });
    let (count, used_scalar_u64, promotion_reason, residues) = match scalar_count {
        Some(Some(count)) => {
            let residues = plan
                .primes
                .iter()
                .map(|&prime| count % prime)
                .collect::<Vec<_>>();
            (u128::from(count), true, None, residues)
        }
        Some(None) => {
            let residues = contract_wide_crt_residues(
                &tasks,
                n,
                prefix.split_depth,
                board_mask,
                &plan.primes,
            )?;
            let count = reconstruct_crt(&residues, &plan.primes)?;
            (
                count,
                false,
                Some(
                    "checked scalar-u64 limit exceeded; replayed identical C sectors with CRT"
                        .to_owned(),
                ),
                residues,
            )
        }
        None => {
            let residues = contract_wide_crt_residues(
                &tasks,
                n,
                prefix.split_depth,
                board_mask,
                &plan.primes,
            )?;
            let count = reconstruct_crt(&residues, &plan.primes)?;
            (
                count,
                false,
                Some("N! exactness bound exceeds u64; used certified CRT backend".to_owned()),
                residues,
            )
        }
    };
    let tail_elapsed = tail_start.elapsed();
    let elapsed = total_start.elapsed();
    if count > plan.factorial_bound {
        return Err("wide scalar/CRT reconstruction exceeds certified N! count bound".to_owned());
    }

    let profile_start = Instant::now();
    let mut recursive_nodes = 0_u128;
    let mut recursive_accepted_entries = 0_u128;
    let mut replay_count = 0_u128;
    if profile_replay {
        let partials = tasks
            .par_iter()
            .map(|task| {
                let mut metrics = RecursiveTailMetrics::default();
                let completions = contract_recursive_tail(
                    n,
                    prefix.split_depth,
                    task.state,
                    relation,
                    board_mask,
                    &mut metrics,
                )?;
                let weighted = completions
                    .checked_mul(u128::from(task.orbit_weight))
                    .ok_or_else(|| "wide scalar profile task weight overflow".to_owned())?;
                Ok::<_, String>((weighted, metrics))
            })
            .collect::<Vec<_>>();
        for partial in partials {
            let (weighted, metrics) = partial?;
            replay_count = replay_count
                .checked_add(weighted)
                .ok_or_else(|| "wide scalar profile reduction overflow".to_owned())?;
            recursive_nodes = recursive_nodes
                .checked_add(metrics.nodes)
                .ok_or_else(|| "wide scalar recursive node counter overflow".to_owned())?;
            recursive_accepted_entries = recursive_accepted_entries
                .checked_add(metrics.accepted_entries)
                .ok_or_else(|| "wide scalar accepted-entry counter overflow".to_owned())?;
        }
        if replay_count != count {
            return Err("wide scalar/CRT result disagrees with generic C replay".to_owned());
        }
    }
    let profile_replay_elapsed = profile_start.elapsed();
    let total_accepted = prefix
        .prefix_accepted_entries
        .checked_add(recursive_accepted_entries)
        .ok_or_else(|| "wide scalar total accepted-entry counter overflow".to_owned())?;
    Ok(WideScalarResult {
        contraction: ContractionResult {
            n,
            count,
            elapsed,
            peak_states: tasks.len().max(1),
            tensor_entries_examined: 17,
            tensor_entries_matched: 17,
            row_operator_candidates: total_accepted,
            row_operator_matched: total_accepted,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        },
        plan,
        split_depth: prefix.split_depth,
        target_tail_tasks: prefix.target_tail_tasks,
        tail_tasks: tasks.len(),
        prefix_nodes: prefix.prefix_nodes,
        prefix_accepted_entries: prefix.prefix_accepted_entries,
        prefix_kept_entries: prefix.prefix_kept_entries,
        recursive_nodes,
        recursive_accepted_entries,
        seed_elapsed: prefix.seed_elapsed,
        tail_elapsed,
        profile_replay_elapsed,
        used_scalar_u64,
        promotion_reason,
        residues,
    })
}

fn contract_rows_adaptive_fast_tail_impl(
    n: usize,
    shards: usize,
    profile_replay: bool,
    microkernel_rows: usize,
    coefficient_limit: u64,
) -> Result<AdaptiveFastTailResult, String> {
    if n == 0 {
        return Ok(AdaptiveFastTailResult {
            fast: contract_rows_certified_fast_tail(0, shards, 0, profile_replay)?,
            selected_cut: 0,
            target_tail_tasks: 1,
            selection_elapsed: Duration::ZERO,
            probes: Vec::new(),
        });
    }
    if n > 21 {
        return Err("adaptive fast-tail backend supports N <= 21".to_owned());
    }
    let selection_start = Instant::now();
    let target_tail_tasks = rayon::current_num_threads().max(1).saturating_mul(512);
    let coefficient_bits = 64_u32
        .checked_sub(
            u32::try_from(3_usize.saturating_mul(n))
                .map_err(|_| "adaptive fast-tail boundary width does not fit u32".to_owned())?,
        )
        .ok_or_else(|| "adaptive fast-tail packing requires N <= 21".to_owned())?;
    // E38 showed that early-tail recursive work is nearly cut-invariant.
    // Start at the shallowest plausible merge and deepen only until there
    // are enough actual sectors for dynamic scheduling.
    let mut selected_cut = n.saturating_sub(11).max(1);
    let mut probes = Vec::new();
    let selected_prefix;
    loop {
        let probe_start = Instant::now();
        let prefix =
            contract_rows_d4_joint_u64_kernel(n, shards, coefficient_bits, true, selected_cut)?;
        let prefix_support = prefix.boundary.iter().map(Vec::len).sum::<usize>();
        probes.push(AdaptiveCutProbe {
            cut: selected_cut,
            prefix_support,
            prefix_elapsed: probe_start.elapsed(),
            prefix_accepted_entries: prefix.parallel.contraction.row_operator_matched,
        });
        if prefix_support >= target_tail_tasks || selected_cut == n {
            selected_prefix = prefix;
            break;
        }
        selected_cut += 1;
    }
    let selection_elapsed = selection_start.elapsed();
    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let relation = RecursiveTailRelation::compile(&operator)?;
    let _plan = CertifiedSecViTailPlan::compile(relation)?;
    let selected_prefix_elapsed = probes
        .last()
        .map_or(Duration::ZERO, |probe| probe.prefix_elapsed);
    let fast = finish_adaptive_selected_prefix(
        n,
        selected_cut,
        profile_replay,
        coefficient_bits,
        relation,
        selected_prefix,
        selection_start,
        selected_prefix_elapsed,
        microkernel_rows,
        coefficient_limit,
    )?;
    Ok(AdaptiveFastTailResult {
        fast,
        selected_cut,
        target_tail_tasks,
        selection_elapsed,
        probes,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_adaptive_selected_prefix(
    n: usize,
    cut: usize,
    profile_replay: bool,
    coefficient_bits: u32,
    relation: RecursiveTailRelation,
    prefix: JointKernelResult,
    total_start: Instant,
    prefix_elapsed: Duration,
    microkernel_rows: usize,
    coefficient_limit: u64,
) -> Result<CertifiedFastTailResult, String> {
    let coefficient_mask = coefficient_mask(coefficient_bits);
    let board_mask = (1_u64 << n) - 1;
    let tail_tasks = prefix
        .boundary
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let prefix_support = tail_tasks.len();
    let tail_start = Instant::now();
    let fast_count = contract_certified_tail_tasks_u64(
        &tail_tasks,
        n,
        cut,
        coefficient_bits,
        coefficient_mask,
        coefficient_limit,
        board_mask,
        microkernel_rows,
    );
    let tail_elapsed = tail_start.elapsed();
    let fast_elapsed = total_start.elapsed();
    let should_replay = profile_replay || fast_count.is_none();
    let profile_start = Instant::now();
    let mut recursive_nodes = 0_u128;
    let mut recursive_accepted_entries = 0_u128;
    let mut replay_count = 0_u128;
    if should_replay {
        let partials = tail_tasks
            .par_iter()
            .map(|&entry| {
                let state = PackedBoundary(u128::from(entry.key(coefficient_bits))).unpack(n);
                let mut metrics = RecursiveTailMetrics::default();
                let completions =
                    contract_recursive_tail(n, cut, state, relation, board_mask, &mut metrics)?;
                let weighted = completions
                    .checked_mul(u128::from(entry.weight(coefficient_mask)))
                    .ok_or_else(|| {
                        "coefficient overflow joining profiled adaptive tail".to_owned()
                    })?;
                Ok::<_, String>((weighted, metrics))
            })
            .collect::<Vec<_>>();
        for partial in partials {
            let (weighted, metrics) = partial?;
            replay_count = replay_count
                .checked_add(weighted)
                .ok_or_else(|| "coefficient overflow reducing profiled adaptive tail".to_owned())?;
            recursive_nodes += metrics.nodes;
            recursive_accepted_entries += metrics.accepted_entries;
        }
    }
    let profile_replay_elapsed = profile_start.elapsed();
    let count = match fast_count {
        Some(count) => {
            if profile_replay && replay_count != u128::from(count) {
                return Err("adaptive u64 tail disagrees with u128 profile replay".to_owned());
            }
            u128::from(count)
        }
        None => replay_count,
    };
    let used_u64_fast_path = fast_count.is_some();
    let mut contraction = prefix.parallel.contraction;
    contraction.count = count;
    contraction.elapsed = if used_u64_fast_path {
        fast_elapsed
    } else {
        total_start.elapsed()
    };
    contraction.peak_rss_bytes = contraction.peak_rss_bytes.max(peak_rss_bytes());
    contraction.row_operator_candidates += recursive_accepted_entries;
    contraction.row_operator_matched += recursive_accepted_entries;
    Ok(CertifiedFastTailResult {
        contraction,
        cut,
        used_u64_fast_path,
        promotion_reason: (!used_u64_fast_path)
            .then(|| "checked u64 adaptive tail overflow; exact u128 replay used".to_owned()),
        prefix_elapsed,
        tail_elapsed,
        profile_replay_elapsed,
        prefix_support,
        tail_tasks: prefix_support,
        recursive_nodes,
        recursive_accepted_entries,
        coefficient_bits,
        max_prefix_coefficient: prefix.max_coefficient_observed,
    })
}

fn contract_rows_certified_fast_tail_with_limit(
    n: usize,
    shards: usize,
    cut: usize,
    profile_replay: bool,
    coefficient_limit: u64,
) -> Result<CertifiedFastTailResult, String> {
    if n == 0 {
        return Ok(CertifiedFastTailResult {
            contraction: ContractionResult {
                n,
                count: 1,
                elapsed: Duration::ZERO,
                peak_states: 1,
                tensor_entries_examined: 17,
                tensor_entries_matched: 17,
                row_operator_candidates: 0,
                row_operator_matched: 0,
                peak_rss_bytes: peak_rss_bytes(),
                layers: Vec::new(),
            },
            cut,
            used_u64_fast_path: true,
            promotion_reason: None,
            prefix_elapsed: Duration::ZERO,
            tail_elapsed: Duration::ZERO,
            profile_replay_elapsed: Duration::ZERO,
            prefix_support: 1,
            tail_tasks: 1,
            recursive_nodes: 1,
            recursive_accepted_entries: 0,
            coefficient_bits: 64,
            max_prefix_coefficient: 1,
        });
    }
    if n > 21 {
        return Err("certified fast-tail backend supports N <= 21".to_owned());
    }
    if cut == 0 || cut > n {
        return Err("certified fast-tail cut must be in 1..=N".to_owned());
    }
    let total_start = Instant::now();
    let coefficient_bits = 64_u32
        .checked_sub(
            u32::try_from(3_usize.saturating_mul(n))
                .map_err(|_| "certified fast-tail boundary width does not fit u32".to_owned())?,
        )
        .ok_or_else(|| "certified fast-tail packing requires N <= 21".to_owned())?;
    let coefficient_mask = coefficient_mask(coefficient_bits);
    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let relation = RecursiveTailRelation::compile(&operator)?;
    let _plan = CertifiedSecViTailPlan::compile(relation)?;
    let board_mask = (1_u64 << n) - 1;
    let prefix = contract_rows_d4_joint_u64_kernel(n, shards, coefficient_bits, true, cut)?;
    let prefix_elapsed = total_start.elapsed();
    let tail_tasks = prefix
        .boundary
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let prefix_support = tail_tasks.len();
    let tail_start = Instant::now();
    let fast_count = contract_certified_tail_tasks_u64(
        &tail_tasks,
        n,
        cut,
        coefficient_bits,
        coefficient_mask,
        coefficient_limit,
        board_mask,
        0,
    );
    let tail_elapsed = tail_start.elapsed();
    let fast_elapsed = total_start.elapsed();

    let should_replay = profile_replay || fast_count.is_none();
    let profile_start = Instant::now();
    let mut recursive_nodes = 0_u128;
    let mut recursive_accepted_entries = 0_u128;
    let mut replay_count = 0_u128;
    if should_replay {
        let partials = tail_tasks
            .par_iter()
            .map(|&entry| {
                let state = PackedBoundary(u128::from(entry.key(coefficient_bits))).unpack(n);
                let mut metrics = RecursiveTailMetrics::default();
                let completions =
                    contract_recursive_tail(n, cut, state, relation, board_mask, &mut metrics)?;
                let weighted = completions
                    .checked_mul(u128::from(entry.weight(coefficient_mask)))
                    .ok_or_else(|| {
                        "coefficient overflow joining profiled certified tail".to_owned()
                    })?;
                Ok::<_, String>((weighted, metrics))
            })
            .collect::<Vec<_>>();
        for partial in partials {
            let (weighted, metrics) = partial?;
            replay_count = replay_count.checked_add(weighted).ok_or_else(|| {
                "coefficient overflow reducing profiled certified tail".to_owned()
            })?;
            recursive_nodes += metrics.nodes;
            recursive_accepted_entries += metrics.accepted_entries;
        }
    }
    let profile_replay_elapsed = profile_start.elapsed();
    let count = match fast_count {
        Some(count) => {
            if profile_replay && replay_count != u128::from(count) {
                return Err("u64 certified tail disagrees with u128 profile replay".to_owned());
            }
            u128::from(count)
        }
        None => replay_count,
    };
    let used_u64_fast_path = fast_count.is_some();
    let promotion_reason = (!used_u64_fast_path)
        .then(|| "checked u64 tail overflow; exact u128 replay used".to_owned());
    let mut contraction = prefix.parallel.contraction;
    contraction.count = count;
    contraction.elapsed = if used_u64_fast_path {
        fast_elapsed
    } else {
        total_start.elapsed()
    };
    contraction.peak_rss_bytes = contraction.peak_rss_bytes.max(peak_rss_bytes());
    contraction.row_operator_candidates += recursive_accepted_entries;
    contraction.row_operator_matched += recursive_accepted_entries;
    Ok(CertifiedFastTailResult {
        contraction,
        cut,
        used_u64_fast_path,
        promotion_reason,
        prefix_elapsed,
        tail_elapsed,
        profile_replay_elapsed,
        prefix_support,
        tail_tasks: prefix_support,
        recursive_nodes,
        recursive_accepted_entries,
        coefficient_bits,
        max_prefix_coefficient: prefix.max_coefficient_observed,
    })
}

#[derive(Clone, Copy)]
struct DeferredCandidate {
    state: u64,
    parent_index: u32,
    multiplicity: u8,
}

fn append_deferred_sparse_d4(
    n: usize,
    operator: &CompiledRowOperator,
    parent: BoundaryState,
    parent_index: u32,
    top_row: bool,
    counters: &mut RowCounters,
    output: &mut Vec<DeferredCandidate>,
) -> Result<usize, String> {
    let occupied = operator.occupied;
    let legs = occupied.legs;
    if legs.row_in != 0 || occupied.value != 1 {
        return Err(
            "deferred sparse kernel requires the unit occupied entry derived from Sec. VI C"
                .to_owned(),
        );
    }
    let board_mask = (1_u64 << n) - 1;
    let matching_bits = |mask: u64, required: u8| -> Result<u64, String> {
        match required {
            0 => Ok((!mask) & board_mask),
            1 => Ok(mask & board_mask),
            _ => Err("compiled C entry contains a non-binary incoming signal".to_owned()),
        }
    };
    let mut positions = matching_bits(parent.columns, legs.column_in)?
        & matching_bits(parent.diag_dr, legs.diag_dr_in)?
        & matching_bits(parent.diag_dl, legs.diag_dl_in)?;
    let start_len = output.len();

    while positions != 0 {
        let selected = positions & positions.wrapping_neg();
        let column = selected.trailing_zeros() as usize;
        positions &= positions - 1;
        counters.operator_candidates += 1;
        counters.operator_matched += 1;
        let successor = BoundaryState {
            columns: replace_bit(parent.columns, column, legs.column_out),
            diag_dr: (replace_bit(parent.diag_dr, column, legs.diag_dr_out) << 1) & board_mask,
            diag_dl: replace_bit(parent.diag_dl, column, legs.diag_dl_out) >> 1,
        };
        let multiplicity = if top_row {
            let Some(value) = top_row_vertical_orbit_weight(n, successor) else {
                continue;
            };
            u8::try_from(value).map_err(|_| "D4 multiplicity does not fit u8".to_owned())?
        } else {
            1
        };
        let packed = PackedBoundary::pack(successor, n).0;
        let state = u64::try_from(packed)
            .map_err(|_| "deferred sparse boundary key does not fit u64".to_owned())?;
        output.push(DeferredCandidate {
            state,
            parent_index,
            multiplicity,
        });
    }
    Ok(output.len() - start_len)
}

fn contract_rows_d4_deferred_sparse_kernel(n: usize) -> Result<ContractionResult, String> {
    if n > 21 {
        return Err("the compact deferred-candidate backend supports N <= 21".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let mut boundary = vec![(PackedBoundary::pack(initial, n), 1_u128)];
    let mut next_boundary = Vec::<(PackedBoundary, u128)>::new();
    let mut candidates = Vec::<DeferredCandidate>::new();
    let mut peak_states = 1;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.len();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        candidates.clear();
        next_boundary.clear();

        for (parent_index, &(packed_parent, _)) in boundary.iter().enumerate() {
            let parent_index = u32::try_from(parent_index)
                .map_err(|_| "deferred sparse parent index exceeds u32".to_owned())?;
            completed_row_terms += append_deferred_sparse_d4(
                n,
                &operator,
                packed_parent.unpack(n),
                parent_index,
                row == 0,
                &mut counters,
                &mut candidates,
            )? as u128;
        }

        candidates.sort_unstable_by_key(|candidate| candidate.state);
        let mut read = 0_usize;
        while read < candidates.len() {
            let state = candidates[read].state;
            let mut weight = 0_u128;
            while read < candidates.len() && candidates[read].state == state {
                let candidate = candidates[read];
                let parent_weight = boundary[candidate.parent_index as usize].1;
                let contribution = parent_weight
                    .checked_mul(u128::from(candidate.multiplicity))
                    .ok_or_else(|| {
                        format!("deferred coefficient overflow after row {}", row + 1)
                    })?;
                weight = weight.checked_add(contribution).ok_or_else(|| {
                    format!("deferred coefficient sum overflow after row {}", row + 1)
                })?;
                read += 1;
            }
            next_boundary.push((PackedBoundary(u128::from(state)), weight));
        }
        std::mem::swap(&mut boundary, &mut next_boundary);

        let output_weight = boundary.iter().try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(boundary.len());
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states: boundary.len(),
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .filter(|(state, _)| state.columns(n) == board_mask)
        .try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| "final coefficient sum overflow".to_owned())
        })?;
    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: 17,
        tensor_entries_matched: 17,
        row_operator_candidates: total_operator_candidates,
        row_operator_matched: total_operator_matched,
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

fn radix_sort_msd(values: &mut [(PackedBoundary, u128)], shift: i32) {
    if values.len() <= 64 || shift < 0 {
        values.sort_unstable_by_key(|(state, _)| state.0);
        return;
    }
    let mut counts = [0_usize; 256];
    for &(state, _) in values.iter() {
        counts[((state.0 >> shift) & 0xff) as usize] += 1;
    }
    let mut starts = [0_usize; 256];
    for bucket in 1..256 {
        starts[bucket] = starts[bucket - 1] + counts[bucket - 1];
    }
    let mut next = starts;
    for bucket in 0..256 {
        let end = starts[bucket] + counts[bucket];
        while next[bucket] < end {
            let selected = ((values[next[bucket]].0.0 >> shift) & 0xff) as usize;
            if selected == bucket {
                next[bucket] += 1;
            } else {
                let target = next[selected];
                values.swap(next[bucket], target);
                next[selected] += 1;
            }
        }
    }
    if shift >= 8 {
        for bucket in 0..256 {
            let start = starts[bucket];
            let end = start + counts[bucket];
            if end - start > 1 {
                radix_sort_msd(&mut values[start..end], shift - 8);
            }
        }
    }
}

fn sort_packed_radix(values: &mut [(PackedBoundary, u128)], n: usize) {
    let key_bits = 3 * n;
    let highest_shift = ((key_bits.saturating_sub(1)) / 8 * 8) as i32;
    radix_sort_msd(values, highest_shift);
}

fn reduce_sorted_candidates(
    candidates: &mut Vec<(PackedBoundary, u128)>,
    row: usize,
) -> Result<(), String> {
    let mut write = 0_usize;
    for read in 0..candidates.len() {
        let (state, weight) = candidates[read];
        if write > 0 && candidates[write - 1].0 == state {
            candidates[write - 1].1 = candidates[write - 1]
                .1
                .checked_add(weight)
                .ok_or_else(|| format!("coefficient overflow after row {}", row + 1))?;
        } else {
            candidates[write] = (state, weight);
            write += 1;
        }
    }
    candidates.truncate(write);
    Ok(())
}

fn contract_rows_d4_optimized_kernel(
    n: usize,
    variant: D4KernelVariant,
) -> Result<ContractionResult, String> {
    if n > 42 {
        return Err("the packed u128 virtual-boundary backend supports N <= 42".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let mut boundary = vec![(PackedBoundary::pack(initial, n), 1_u128)];
    let mut candidates = Vec::<(PackedBoundary, u128)>::new();
    let mut peak_states = 1;
    let mut total_operator_candidates = 0_u128;
    let mut total_operator_matched = 0_u128;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.len();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0_u128;
        candidates.clear();

        for (packed_parent, parent_weight) in boundary.drain(..) {
            let parent = packed_parent.unpack(n);
            if variant == D4KernelVariant::Arena {
                let mut row_terms =
                    contract_one_row_compiled(n, &operator, parent, parent_weight, &mut counters)?;
                if row == 0 {
                    row_terms = apply_top_row_symmetry(n, row_terms)?;
                }
                completed_row_terms += row_terms.len() as u128;
                candidates.extend(
                    row_terms
                        .into_iter()
                        .map(|(state, weight)| (PackedBoundary::pack(state, n), weight)),
                );
            } else {
                let appended = if matches!(
                    variant,
                    D4KernelVariant::ArenaBatchedSparse
                        | D4KernelVariant::ArenaBatchedSparseParallelSort
                ) {
                    append_compiled_sparse_d4(
                        n,
                        &operator,
                        parent,
                        parent_weight,
                        row == 0,
                        &mut counters,
                        &mut candidates,
                    )?
                } else {
                    append_compiled_dense_d4(
                        n,
                        &operator,
                        parent,
                        parent_weight,
                        row == 0,
                        &mut counters,
                        &mut candidates,
                    )?
                };
                completed_row_terms += appended as u128;
            }
        }

        if variant == D4KernelVariant::ArenaBatchedRadix {
            sort_packed_radix(&mut candidates, n);
        } else if variant == D4KernelVariant::ArenaBatchedSparseParallelSort {
            candidates.par_sort_unstable_by_key(|(state, _)| state.0);
        } else {
            candidates.sort_unstable_by_key(|(state, _)| state.0);
        }
        reduce_sorted_candidates(&mut candidates, row)?;
        std::mem::swap(&mut boundary, &mut candidates);

        let output_weight = boundary.iter().try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(boundary.len());
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states: boundary.len(),
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: peak_rss_bytes(),
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .filter(|(state, _)| state.columns(n) == board_mask)
        .try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| "final coefficient sum overflow".to_owned())
        })?;
    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: 17,
        tensor_entries_matched: 17,
        row_operator_candidates: total_operator_candidates,
        row_operator_matched: total_operator_matched,
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

fn contract_rows_sort_reduce_with_modes(
    n: usize,
    symmetry_mode: SymmetryMode,
    position_mode: PositionMode,
) -> Result<ContractionResult, String> {
    if n > 42 {
        return Err("the packed u128 virtual-boundary backend supports N <= 42".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    debug_assert_eq!(tensor.entries().len(), 17);
    let operator = CompiledRowOperator::compile(&tensor)?;
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let mut boundary = vec![(PackedBoundary::pack(initial, n), 1_u128)];
    let mut peak_states = 1;
    let mut total_operator_candidates = 0;
    let mut total_operator_matched = 0;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.len();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0;
        let mut candidates = Vec::<(PackedBoundary, u128)>::new();

        for (packed_parent, parent_weight) in std::mem::take(&mut boundary) {
            let parent = packed_parent.unpack(n);
            let mut row_terms = contract_one_row_with_position_mode(
                n,
                &operator,
                parent,
                parent_weight,
                &mut counters,
                position_mode,
            )?;
            if row == 0 && matches!(symmetry_mode, SymmetryMode::TopRowVerticalOrbits) {
                row_terms = apply_top_row_symmetry(n, row_terms)?;
            }
            completed_row_terms += row_terms.len() as u128;
            candidates.extend(
                row_terms
                    .into_iter()
                    .map(|(successor, weight)| (PackedBoundary::pack(successor, n), weight)),
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
                    .ok_or_else(|| format!("coefficient overflow after row {}", row + 1))?;
            } else {
                candidates[write] = (state, weight);
                write += 1;
            }
        }
        candidates.truncate(write);
        boundary = candidates;

        let output_weight = boundary.iter().try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
        })?;
        peak_states = peak_states.max(boundary.len());
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        let layer_peak_rss = peak_rss_bytes();
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
            completed_row_terms,
            output_states: boundary.len(),
            output_weight,
            elapsed: layer_start.elapsed(),
            peak_rss_bytes: layer_peak_rss,
        });
    }

    // Identical v1/v2 final contraction to the hash materialization backend.
    let board_mask = (1_u64 << n) - 1;
    let count = boundary
        .iter()
        .filter(|(state, _)| state.columns(n) == board_mask)
        .try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| "final coefficient sum overflow".to_owned())
        })?;

    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: 17,
        tensor_entries_matched: 17,
        row_operator_candidates: total_operator_candidates,
        row_operator_matched: total_operator_matched,
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

/// Contract the exact compiled row operator with sliced parallel expansion and
/// parallel sorting. The final reduce remains serial and uses checked integer
/// addition in sorted key order.
pub fn contract_rows_parallel_sort_reduce(
    n: usize,
    threads: usize,
) -> Result<ContractionResult, String> {
    contract_rows_parallel_sort_reduce_with_modes(
        n,
        threads,
        SymmetryMode::None,
        PositionMode::Dense,
    )
}

pub fn contract_rows_sparse_parallel_sort_reduce(
    n: usize,
    threads: usize,
) -> Result<ContractionResult, String> {
    contract_rows_parallel_sort_reduce_with_modes(
        n,
        threads,
        SymmetryMode::None,
        PositionMode::Sparse,
    )
}

/// Parallel sort-reduce after exact first-row vertical-reflection orbit slicing.
pub fn contract_rows_d4_orbit_parallel_sort_reduce(
    n: usize,
    threads: usize,
) -> Result<ContractionResult, String> {
    contract_rows_parallel_sort_reduce_with_modes(
        n,
        threads,
        SymmetryMode::TopRowVerticalOrbits,
        PositionMode::Dense,
    )
}

pub fn contract_rows_d4_sparse_parallel_sort_reduce(
    n: usize,
    threads: usize,
) -> Result<ContractionResult, String> {
    contract_rows_parallel_sort_reduce_with_modes(
        n,
        threads,
        SymmetryMode::TopRowVerticalOrbits,
        PositionMode::Sparse,
    )
}

fn contract_rows_parallel_sort_reduce_with_modes(
    n: usize,
    threads: usize,
    symmetry_mode: SymmetryMode,
    position_mode: PositionMode,
) -> Result<ContractionResult, String> {
    if threads == 0 {
        return Err("parallel sort-reduce requires at least one thread".to_owned());
    }
    if n > 42 {
        return Err("the packed u128 virtual-boundary backend supports N <= 42".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|error| format!("failed to build Rayon pool: {error}"))?;
    pool.install(|| {
        let tensor = SiteTensorC::sec_vi();
        debug_assert_eq!(tensor.entries().len(), 17);
        let operator = CompiledRowOperator::compile(&tensor)?;
        let initial = BoundaryState {
            columns: 0,
            diag_dr: 0,
            diag_dl: 0,
        };
        let mut boundary = vec![(PackedBoundary::pack(initial, n), 1_u128)];
        let mut peak_states = 1;
        let mut total_operator_candidates = 0;
        let mut total_operator_matched = 0;
        let mut layers = Vec::with_capacity(n);
        let total_start = Instant::now();

        for row in 0..n {
            let layer_start = Instant::now();
            let input_states = boundary.len();
            let target_slices = threads.saturating_mul(4).max(1);
            let chunk_size = input_states.div_ceil(target_slices).max(1);
            let chunks = boundary
                .par_chunks(chunk_size)
                .map(|parents| {
                    let mut local = Vec::<(PackedBoundary, u128)>::new();
                    let mut counters = RowCounters::default();
                    for &(packed_parent, parent_weight) in parents {
                        let parent = packed_parent.unpack(n);
                        let mut row_terms = contract_one_row_with_position_mode(
                            n,
                            &operator,
                            parent,
                            parent_weight,
                            &mut counters,
                            position_mode,
                        )?;
                        if row == 0 && matches!(symmetry_mode, SymmetryMode::TopRowVerticalOrbits) {
                            row_terms = apply_top_row_symmetry(n, row_terms)?;
                        }
                        local.extend(row_terms.into_iter().map(|(successor, weight)| {
                            (PackedBoundary::pack(successor, n), weight)
                        }));
                    }
                    Ok::<_, String>((local, counters))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let completed_row_terms = chunks
                .iter()
                .map(|(terms, _)| terms.len() as u128)
                .sum::<u128>();
            let candidate_capacity = usize::try_from(completed_row_terms)
                .map_err(|_| format!("row {} candidate count exceeds usize", row + 1))?;
            let mut candidates = Vec::with_capacity(candidate_capacity);
            let mut counters = RowCounters::default();
            for (mut terms, local) in chunks {
                candidates.append(&mut terms);
                counters.tensor_examined += local.tensor_examined;
                counters.tensor_matched += local.tensor_matched;
                counters.operator_candidates += local.operator_candidates;
                counters.operator_matched += local.operator_matched;
            }
            drop(std::mem::take(&mut boundary));

            candidates.par_sort_unstable_by_key(|(state, _)| state.0);
            let mut write = 0_usize;
            for read in 0..candidates.len() {
                let (state, weight) = candidates[read];
                if write > 0 && candidates[write - 1].0 == state {
                    candidates[write - 1].1 = candidates[write - 1]
                        .1
                        .checked_add(weight)
                        .ok_or_else(|| format!("coefficient overflow after row {}", row + 1))?;
                } else {
                    candidates[write] = (state, weight);
                    write += 1;
                }
            }
            candidates.truncate(write);
            boundary = candidates;

            let output_weight = boundary.iter().try_fold(0_u128, |sum, (_, value)| {
                sum.checked_add(*value)
                    .ok_or_else(|| format!("coefficient sum overflow after row {}", row + 1))
            })?;
            peak_states = peak_states.max(boundary.len());
            total_operator_candidates += counters.operator_candidates;
            total_operator_matched += counters.operator_matched;
            let layer_peak_rss = peak_rss_bytes();
            layers.push(LayerMetric {
                row,
                input_states,
                tensor_entries_examined: counters.tensor_examined,
                tensor_entries_matched: counters.tensor_matched,
                row_operator_candidates: counters.operator_candidates,
                row_operator_matched: counters.operator_matched,
                completed_row_terms,
                output_states: boundary.len(),
                output_weight,
                elapsed: layer_start.elapsed(),
                peak_rss_bytes: layer_peak_rss,
            });
        }

        let board_mask = (1_u64 << n) - 1;
        let count = boundary
            .iter()
            .filter(|(state, _)| state.columns(n) == board_mask)
            .try_fold(0_u128, |sum, (_, value)| {
                sum.checked_add(*value)
                    .ok_or_else(|| "final coefficient sum overflow".to_owned())
            })?;

        Ok(ContractionResult {
            n,
            count,
            elapsed: total_start.elapsed(),
            peak_states,
            tensor_entries_examined: 17,
            tensor_entries_matched: 17,
            row_operator_candidates: total_operator_candidates,
            row_operator_matched: total_operator_matched,
            peak_rss_bytes: peak_rss_bytes(),
            layers,
        })
    })
}

/// Reference backend retained for tensor-level verification.
pub fn contract_rows_sitewise(n: usize) -> Result<ContractionResult, String> {
    contract_rows_with_backend(n, RowBackend::Sitewise)
}

fn contract_rows_with_backend(
    n: usize,
    row_backend: RowBackend,
) -> Result<ContractionResult, String> {
    if n > 42 {
        return Err("the packed u128 virtual-boundary backend supports N <= 42".to_owned());
    }
    if n == 0 {
        return Ok(ContractionResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_states: 1,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            peak_rss_bytes: peak_rss_bytes(),
            layers: Vec::new(),
        });
    }

    let tensor = SiteTensorC::sec_vi();
    debug_assert_eq!(tensor.entries().len(), 17);
    let compiled_operator = match row_backend {
        RowBackend::Compiled => Some(CompiledRowOperator::compile(&tensor)?),
        RowBackend::Sitewise => None,
    };
    let initial = BoundaryState {
        columns: 0,
        diag_dr: 0,
        diag_dl: 0,
    };
    let mut boundary = HashMap::from([(PackedBoundary::pack(initial, n), 1_u128)]);
    let mut peak_states = 1;
    let mut total_tensor_examined = if compiled_operator.is_some() { 17 } else { 0 };
    let mut total_tensor_matched = if compiled_operator.is_some() { 17 } else { 0 };
    let mut total_operator_candidates = 0;
    let mut total_operator_matched = 0;
    let mut layers = Vec::with_capacity(n);
    let total_start = Instant::now();

    for row in 0..n {
        let layer_start = Instant::now();
        let input_states = boundary.len();
        let mut counters = RowCounters::default();
        let mut completed_row_terms = 0;
        let mut next = HashMap::<PackedBoundary, u128>::new();

        for (packed_parent, parent_weight) in boundary.drain() {
            let parent = packed_parent.unpack(n);
            let row_terms = match &compiled_operator {
                Some(operator) => {
                    contract_one_row_compiled(n, operator, parent, parent_weight, &mut counters)?
                }
                None => {
                    contract_one_row_sitewise(n, &tensor, parent, parent_weight, &mut counters)?
                }
            };
            for (successor, weight) in row_terms {
                completed_row_terms += 1;
                let coefficient = next.entry(PackedBoundary::pack(successor, n)).or_insert(0);
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
        total_tensor_examined += counters.tensor_examined;
        total_tensor_matched += counters.tensor_matched;
        total_operator_candidates += counters.operator_candidates;
        total_operator_matched += counters.operator_matched;
        let layer_peak_rss = peak_rss_bytes();
        layers.push(LayerMetric {
            row,
            input_states,
            tensor_entries_examined: counters.tensor_examined,
            tensor_entries_matched: counters.tensor_matched,
            row_operator_candidates: counters.operator_candidates,
            row_operator_matched: counters.operator_matched,
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
        .filter(|(state, _)| state.columns(n) == board_mask)
        .try_fold(0_u128, |sum, (_, value)| {
            sum.checked_add(*value)
                .ok_or_else(|| "final coefficient sum overflow".to_owned())
        })?;

    Ok(ContractionResult {
        n,
        count,
        elapsed: total_start.elapsed(),
        peak_states,
        tensor_entries_examined: total_tensor_examined,
        tensor_entries_matched: total_tensor_matched,
        row_operator_candidates: total_operator_candidates,
        row_operator_matched: total_operator_matched,
        peak_rss_bytes: peak_rss_bytes(),
        layers,
    })
}

pub fn known_count(n: usize) -> Option<u128> {
    const COUNTS: [u128; 28] = [
        1,
        1,
        0,
        0,
        2,
        10,
        4,
        40,
        92,
        352,
        724,
        2_680,
        14_200,
        73_712,
        365_596,
        2_279_184,
        14_772_512,
        95_815_104,
        666_090_624,
        4_968_057_848,
        39_029_188_884,
        314_666_222_712,
        2_691_008_701_644,
        24_233_937_684_440,
        227_514_171_973_736,
        2_207_893_435_808_352,
        22_317_699_616_364_044,
        234_907_967_154_122_528,
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
mod e24_kernel_tests {
    use super::{
        ExplicitFrontierOrder, PackedBoundary, ShardMode, contract_explicit_c_frontier,
        contract_rows_d4_arena_sort_reduce, contract_rows_d4_batched_radix,
        contract_rows_d4_batched_sort_reduce, contract_rows_d4_batched_sparse_parallel_sort,
        contract_rows_d4_batched_sparse_sort_reduce, contract_rows_d4_compact_parallel_generation,
        contract_rows_d4_compact_sharded_sort_reduce, contract_rows_d4_compact_u64_promoting,
        contract_rows_d4_compact_u64_promoting_with_limit,
        contract_rows_d4_deferred_sparse_sort_reduce, contract_rows_d4_joint_u64_arena_reuse,
        contract_rows_d4_joint_u64_promoting, contract_rows_d4_joint_u64_with_limits,
        contract_rows_d4_orbit_sort_reduce, contract_rows_d4_sharded_sparse_sort_reduce,
        contract_rows_d4_sparse_sort_reduce, known_count, sort_packed_radix,
    };

    #[test]
    fn in_place_radix_matches_standard_u128_key_sort() {
        let mut radix = (0..10_000_u128)
            .map(|index| {
                let key = index
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    .rotate_left((index % 127) as u32)
                    & ((1_u128 << 45) - 1);
                (PackedBoundary(key), index)
            })
            .rev()
            .collect::<Vec<_>>();
        let mut expected = radix.clone();
        expected.sort_unstable_by_key(|(state, _)| state.0);
        sort_packed_radix(&mut radix, 15);
        assert_eq!(
            radix.iter().map(|entry| entry.0.0).collect::<Vec<_>>(),
            expected.iter().map(|entry| entry.0.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn e24_variants_preserve_counts_support_and_operator_work() {
        for n in 0..=10 {
            let baseline = contract_rows_d4_orbit_sort_reduce(n).unwrap();
            for candidate in [
                contract_rows_d4_arena_sort_reduce(n).unwrap(),
                contract_rows_d4_batched_sort_reduce(n).unwrap(),
                contract_rows_d4_batched_radix(n).unwrap(),
            ] {
                assert_eq!(candidate.count, baseline.count, "N={n}");
                assert_eq!(candidate.peak_states, baseline.peak_states, "N={n}");
                assert_eq!(
                    candidate.row_operator_candidates, baseline.row_operator_candidates,
                    "N={n}"
                );
                assert_eq!(
                    candidate.row_operator_matched, baseline.row_operator_matched,
                    "N={n}"
                );
            }
        }
    }

    #[test]
    fn sparse_batched_variant_matches_existing_sparse_contraction() {
        for n in 0..=10 {
            let baseline = contract_rows_d4_sparse_sort_reduce(n).unwrap();
            let candidate = contract_rows_d4_batched_sparse_sort_reduce(n).unwrap();
            assert_eq!(candidate.count, baseline.count, "N={n}");
            assert_eq!(candidate.peak_states, baseline.peak_states, "N={n}");
            assert_eq!(
                candidate.row_operator_candidates, baseline.row_operator_candidates,
                "N={n}"
            );
            assert_eq!(
                candidate.row_operator_matched, baseline.row_operator_matched,
                "N={n}"
            );
        }
    }

    #[test]
    fn deferred_sparse_variant_matches_existing_sparse_contraction() {
        assert_eq!(std::mem::size_of::<super::DeferredCandidate>(), 16);
        for n in 0..=10 {
            let baseline = contract_rows_d4_sparse_sort_reduce(n).unwrap();
            let candidate = contract_rows_d4_deferred_sparse_sort_reduce(n).unwrap();
            assert_eq!(candidate.count, baseline.count, "N={n}");
            assert_eq!(candidate.peak_states, baseline.peak_states, "N={n}");
            assert_eq!(
                candidate.row_operator_candidates, baseline.row_operator_candidates,
                "N={n}"
            );
            assert_eq!(
                candidate.row_operator_matched, baseline.row_operator_matched,
                "N={n}"
            );
        }
    }

    #[test]
    fn parallel_sort_variant_matches_serial_sparse_contraction() {
        for n in 0..=10 {
            let baseline = contract_rows_d4_sparse_sort_reduce(n).unwrap();
            let candidate = contract_rows_d4_batched_sparse_parallel_sort(n).unwrap();
            assert_eq!(candidate.count, baseline.count, "N={n}");
            assert_eq!(candidate.peak_states, baseline.peak_states, "N={n}");
            assert_eq!(
                candidate.row_operator_candidates, baseline.row_operator_candidates,
                "N={n}"
            );
            assert_eq!(
                candidate.row_operator_matched, baseline.row_operator_matched,
                "N={n}"
            );
        }
    }

    #[test]
    fn sharded_variants_match_serial_sparse_contraction() {
        for n in 0..=10 {
            let baseline = contract_rows_d4_sparse_sort_reduce(n).unwrap();
            for mode in [ShardMode::Prefix, ShardMode::Mixed] {
                for shards in [1, 8] {
                    let candidate =
                        contract_rows_d4_sharded_sparse_sort_reduce(n, shards, mode).unwrap();
                    assert_eq!(candidate.count, baseline.count, "N={n}");
                    assert_eq!(candidate.peak_states, baseline.peak_states, "N={n}");
                    assert_eq!(
                        candidate.row_operator_candidates, baseline.row_operator_candidates,
                        "N={n}"
                    );
                    assert_eq!(
                        candidate.row_operator_matched, baseline.row_operator_matched,
                        "N={n}"
                    );
                }
            }
        }
    }

    #[test]
    fn compact_sharded_layout_matches_e26() {
        assert_eq!(std::mem::size_of::<super::CompactEntry>(), 24);
        for n in 0..=10 {
            let baseline =
                contract_rows_d4_sharded_sparse_sort_reduce(n, 8, ShardMode::Prefix).unwrap();
            let candidate = contract_rows_d4_compact_sharded_sort_reduce(n, 8).unwrap();
            assert_eq!(candidate.count, baseline.count, "N={n}");
            assert_eq!(candidate.peak_states, baseline.peak_states, "N={n}");
            assert_eq!(
                candidate.row_operator_candidates, baseline.row_operator_candidates,
                "N={n}"
            );
            assert_eq!(
                candidate.row_operator_matched, baseline.row_operator_matched,
                "N={n}"
            );
        }
    }

    #[test]
    fn parallel_generation_matches_compact_serial_generation() {
        for n in 0..=10 {
            let baseline = contract_rows_d4_compact_sharded_sort_reduce(n, 8).unwrap();
            let candidate = contract_rows_d4_compact_parallel_generation(n, 8).unwrap();
            assert_eq!(candidate.contraction.count, baseline.count, "N={n}");
            assert_eq!(
                candidate.contraction.peak_states, baseline.peak_states,
                "N={n}"
            );
            assert_eq!(
                candidate.contraction.row_operator_candidates, baseline.row_operator_candidates,
                "N={n}"
            );
            assert_eq!(
                candidate.contraction.row_operator_matched, baseline.row_operator_matched,
                "N={n}"
            );
        }
    }

    #[test]
    fn compact64_fast_path_and_forced_promotion_are_exact() {
        assert_eq!(std::mem::size_of::<super::CompactEntry64>(), 16);
        for n in 0..=10 {
            let baseline = contract_rows_d4_compact_parallel_generation(n, 8).unwrap();
            let candidate = contract_rows_d4_compact_u64_promoting(n, 8).unwrap();
            assert!(candidate.used_u64_fast_path, "N={n}");
            assert_eq!(
                candidate.contraction.count, baseline.contraction.count,
                "N={n}"
            );
            assert_eq!(
                candidate.contraction.peak_states, baseline.contraction.peak_states,
                "N={n}"
            );
            assert_eq!(
                candidate.contraction.row_operator_matched,
                baseline.contraction.row_operator_matched,
                "N={n}"
            );
        }

        let promoted = contract_rows_d4_compact_u64_promoting_with_limit(8, 8, 1).unwrap();
        assert!(!promoted.used_u64_fast_path);
        assert!(promoted.promotion_reason.is_some());
        assert_eq!(promoted.contraction.count, 92);
        assert_eq!(
            promoted.contraction.peak_states,
            contract_rows_d4_compact_parallel_generation(8, 8)
                .unwrap()
                .contraction
                .peak_states
        );
    }

    #[test]
    fn joint_u64_fast_path_and_two_level_promotion_are_exact() {
        assert_eq!(std::mem::size_of::<super::JointEntry>(), 8);
        for n in 0..=10 {
            let baseline = contract_rows_d4_compact_u64_promoting(n, 8).unwrap();
            let candidate = contract_rows_d4_joint_u64_promoting(n, 8).unwrap();
            assert!(candidate.used_joint_fast_path, "N={n}");
            assert_eq!(
                candidate.contraction.count, baseline.contraction.count,
                "N={n}"
            );
            assert_eq!(
                candidate.contraction.peak_states, baseline.contraction.peak_states,
                "N={n}"
            );
            assert_eq!(
                candidate.contraction.row_operator_matched,
                baseline.contraction.row_operator_matched,
                "N={n}"
            );
        }

        let compact64 = contract_rows_d4_joint_u64_with_limits(8, 8, 1, u64::MAX).unwrap();
        assert!(!compact64.used_joint_fast_path);
        assert_eq!(compact64.fallback_used_u64_fast_path, Some(true));
        assert_eq!(compact64.contraction.count, 92);

        let u128_fallback = contract_rows_d4_joint_u64_with_limits(8, 8, 1, 1).unwrap();
        assert!(!u128_fallback.used_joint_fast_path);
        assert_eq!(u128_fallback.fallback_used_u64_fast_path, Some(false));
        assert_eq!(u128_fallback.contraction.count, 92);
    }

    #[test]
    fn reused_destination_arenas_preserve_joint_contraction() {
        for n in 0..=10 {
            let baseline = contract_rows_d4_joint_u64_promoting(n, 8).unwrap();
            let candidate = contract_rows_d4_joint_u64_arena_reuse(n, 8).unwrap();
            assert!(candidate.joint.used_joint_fast_path, "N={n}");
            assert_eq!(
                candidate.joint.contraction.count, baseline.contraction.count,
                "N={n}"
            );
            assert_eq!(
                candidate.joint.contraction.peak_states, baseline.contraction.peak_states,
                "N={n}"
            );
            assert_eq!(
                candidate.joint.contraction.row_operator_matched,
                baseline.contraction.row_operator_matched,
                "N={n}"
            );
        }
        let candidate = contract_rows_d4_joint_u64_arena_reuse(10, 8).unwrap();
        assert!(candidate.total_reused_capacity_bytes > 0);
        assert!(candidate.peak_spare_capacity_bytes > 0);
    }

    #[test]
    fn generic_explicit_c_frontiers_match_known_counts() {
        for n in 0..=6 {
            for order in [
                ExplicitFrontierOrder::RowMajor,
                ExplicitFrontierOrder::TopLeftDiamond,
            ] {
                let result = contract_explicit_c_frontier(n, order, 5_000_000).unwrap();
                assert!(result.complete, "N={n}, order={order:?}");
                assert_eq!(result.count, known_count(n), "N={n}, order={order:?}");
                assert_eq!(result.contracted_sites, n * n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoundaryState, CompiledRowOperator, ConstraintFamily, D4Symmetry, PackedBoundary,
        RecursiveTailRelation, RowCounters, SiteTensorB, SiteTensorC, VirtualLegs,
        contract_one_row_compiled, contract_one_row_sitewise, contract_rows,
        contract_rows_adaptive_fast_tail, contract_rows_adaptive_fast_tail_impl,
        contract_rows_adaptive_last_k_tail_with_rows, contract_rows_certified_fast_tail,
        contract_rows_certified_fast_tail_with_limit, contract_rows_d4_orbit_parallel_sort_reduce,
        contract_rows_d4_orbit_sort_reduce, contract_rows_d4_recursive_tail,
        contract_rows_d4_sparse_parallel_sort_reduce, contract_rows_d4_sparse_sort_reduce,
        contract_rows_hash_materialization, contract_rows_parallel_sort_reduce,
        contract_rows_sitewise, contract_rows_sort_reduce,
        contract_rows_sparse_parallel_sort_reduce, contract_rows_sparse_sort_reduce,
        contract_rows_wide_crt, contract_rows_wide_scalar,
        contract_rows_wide_scalar_last_k_with_target,
        contract_rows_wide_scalar_with_target_and_limit, known_count, probe_wide_crt_prefix,
        reconstruct_crt, recursive_tail_positions, recursive_tail_successor,
        top_row_vertical_orbit_weight, wide_crt_plan,
    };
    use std::collections::{HashMap, HashSet};

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

    fn line_amplitude_from_b(occupations: &[u8], end_vector: [u128; 2]) -> u128 {
        let tensor = SiteTensorB::sec_vi();
        // v0=(1,0) fixes the incoming signal at the start of the line.
        let mut boundary = [1_u128, 0_u128];
        for &alpha in occupations {
            let mut transfer = [[None; 2]; 2];
            for entry in tensor.entries().iter().filter(|entry| entry.alpha == alpha) {
                let incoming = usize::from(entry.legs.column_in);
                let outgoing = usize::from(entry.legs.column_out);
                if let Some(previous) = transfer[incoming][outgoing] {
                    assert_eq!(previous, entry.value);
                } else {
                    transfer[incoming][outgoing] = Some(entry.value);
                }
            }
            let mut next = [0_u128; 2];
            for incoming in 0..2 {
                for (outgoing, local_value) in transfer[incoming].iter().enumerate() {
                    if let Some(local_value) = local_value {
                        next[outgoing] += boundary[incoming] * local_value;
                    }
                }
            }
            boundary = next;
        }
        boundary[0] * end_vector[0] + boundary[1] * end_vector[1]
    }

    #[test]
    fn v0_v1_v2_line_boundaries_enforce_exactly_and_at_most_one() {
        let v1 = [0_u128, 1_u128];
        let v2 = [1_u128, 1_u128];
        for bits in 0_u8..16 {
            let occupations = (0..4).map(|site| (bits >> site) & 1).collect::<Vec<_>>();
            let queens = bits.count_ones();
            assert_eq!(
                line_amplitude_from_b(&occupations, v1),
                u128::from(queens == 1),
                "v0...v1 must accept exactly one occupied site: {occupations:?}"
            );
            assert_eq!(
                line_amplitude_from_b(&occupations, v2),
                u128::from(queens <= 1),
                "v0...v2 must accept at most one occupied site: {occupations:?}"
            );
        }
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

    fn normalized_terms(terms: Vec<(BoundaryState, u128)>) -> HashMap<BoundaryState, u128> {
        let mut normalized = HashMap::new();
        for (state, weight) in terms {
            *normalized.entry(state).or_insert(0) += weight;
        }
        normalized
    }

    #[test]
    fn compiled_operator_matches_sitewise_for_every_reachable_parent_through_n8() {
        let tensor = SiteTensorC::sec_vi();
        let operator = CompiledRowOperator::compile(&tensor).unwrap();

        for n in 1..=8 {
            let mut boundary = HashMap::from([(
                BoundaryState {
                    columns: 0,
                    diag_dr: 0,
                    diag_dl: 0,
                },
                1_u128,
            )]);
            for row in 0..n {
                let mut next = HashMap::new();
                for (&parent, &parent_weight) in &boundary {
                    let reference = contract_one_row_sitewise(
                        n,
                        &tensor,
                        parent,
                        parent_weight,
                        &mut RowCounters::default(),
                    )
                    .unwrap();
                    let compiled = contract_one_row_compiled(
                        n,
                        &operator,
                        parent,
                        parent_weight,
                        &mut RowCounters::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        normalized_terms(reference.clone()),
                        normalized_terms(compiled),
                        "N={n}, row={}, parent={parent:?}",
                        row + 1
                    );
                    for (state, weight) in reference {
                        *next.entry(state).or_insert(0) += weight;
                    }
                }
                boundary = next;
            }
        }
    }

    #[test]
    fn recursive_tail_successors_replay_compiled_c_on_every_reachable_parent() {
        let tensor = SiteTensorC::sec_vi();
        let operator = CompiledRowOperator::compile(&tensor).unwrap();
        let relation = RecursiveTailRelation::compile(&operator).unwrap();

        for n in 1..=8 {
            let board_mask = (1_u64 << n) - 1;
            let mut boundary = HashMap::from([(
                BoundaryState {
                    columns: 0,
                    diag_dr: 0,
                    diag_dl: 0,
                },
                1_u128,
            )]);
            for row in 0..n {
                let mut next = HashMap::new();
                for (&parent, &parent_weight) in &boundary {
                    let compiled = contract_one_row_compiled(
                        n,
                        &operator,
                        parent,
                        1,
                        &mut RowCounters::default(),
                    )
                    .unwrap();
                    let mut positions = recursive_tail_positions(parent, relation, board_mask);
                    let mut recursive = Vec::new();
                    while positions != 0 {
                        let selected = positions & positions.wrapping_neg();
                        positions &= positions - 1;
                        recursive.push((
                            recursive_tail_successor(parent, selected, relation, board_mask),
                            relation.value,
                        ));
                    }
                    assert_eq!(
                        normalized_terms(recursive),
                        normalized_terms(compiled),
                        "N={n}, row={}, parent={parent:?}",
                        row + 1
                    );
                    for (state, weight) in contract_one_row_sitewise(
                        n,
                        &tensor,
                        parent,
                        parent_weight,
                        &mut RowCounters::default(),
                    )
                    .unwrap()
                    {
                        *next.entry(state).or_insert(0) += weight;
                    }
                }
                boundary = next;
            }
        }
    }

    #[test]
    fn every_recursive_tail_cut_matches_known_counts() {
        for n in 0..=10 {
            for cut in 0..=n {
                let result = contract_rows_d4_recursive_tail(n, 8, cut).unwrap();
                assert_eq!(
                    Some(result.contraction.count),
                    known_count(n),
                    "N={n}, cut={cut}"
                );
            }
        }
    }

    #[test]
    fn certified_u64_tail_and_forced_u128_replay_are_exact() {
        assert_eq!(
            contract_rows_certified_fast_tail(0, 8, 0, true)
                .unwrap()
                .contraction
                .count,
            1
        );
        for n in 1..=10 {
            for cut in 1..=n {
                let result = contract_rows_certified_fast_tail(n, 8, cut, true).unwrap();
                assert!(result.used_u64_fast_path, "N={n}, cut={cut}");
                assert_eq!(
                    Some(result.contraction.count),
                    known_count(n),
                    "N={n}, cut={cut}"
                );
                if result.prefix_support > 0 {
                    assert!(result.recursive_nodes > 0);
                }
            }
        }
        let promoted = contract_rows_certified_fast_tail_with_limit(8, 8, 1, false, 1).unwrap();
        assert!(!promoted.used_u64_fast_path);
        assert_eq!(promoted.contraction.count, 92);
        assert!(promoted.promotion_reason.is_some());
    }

    #[test]
    fn adaptive_actual_support_cut_selection_is_exact() {
        for n in 0..=10 {
            let result = contract_rows_adaptive_fast_tail(n, 8, true).unwrap();
            assert_eq!(Some(result.fast.contraction.count), known_count(n), "N={n}");
            assert!(result.selected_cut <= n);
            if n > 0 {
                assert!(!result.probes.is_empty());
                assert_eq!(result.probes.last().unwrap().cut, result.selected_cut);
            }
        }
    }

    #[test]
    fn certified_last_k_microkernel_and_forced_replay_are_exact() {
        for n in 0..=10 {
            for microkernel_rows in 2..=4 {
                let result =
                    contract_rows_adaptive_last_k_tail_with_rows(n, 8, true, microkernel_rows)
                        .unwrap();
                assert_eq!(
                    Some(result.fast.contraction.count),
                    known_count(n),
                    "N={n}, last-k={microkernel_rows}"
                );
                assert!(result.fast.used_u64_fast_path);
            }
        }
        let promoted = contract_rows_adaptive_fast_tail_impl(8, 8, false, 4, 1).unwrap();
        assert!(!promoted.fast.used_u64_fast_path);
        assert_eq!(promoted.fast.contraction.count, 92);
        assert!(promoted.fast.promotion_reason.is_some());
    }

    #[test]
    fn wide_crt_bounds_primes_and_reconstruction_are_certified() {
        let plan22 = wide_crt_plan(22).unwrap();
        assert_eq!(plan22.primes.len(), 3);
        assert!(plan22.modulus_product > plan22.factorial_bound);
        let plan28 = wide_crt_plan(28).unwrap();
        assert_eq!(plan28.primes.len(), 4);
        assert!(plan28.modulus_product > plan28.factorial_bound);

        let value = 234_907_967_154_122_528_u128;
        let residues = plan28
            .primes
            .iter()
            .map(|&prime| (value % u128::from(prime)) as u64)
            .collect::<Vec<_>>();
        assert_eq!(reconstruct_crt(&residues, &plan28.primes).unwrap(), value);
    }

    #[test]
    fn wide_crt_contraction_and_n22_prefix_are_explicit_c_exact() {
        for n in 0..=12 {
            let result = contract_rows_wide_crt(n, true).unwrap();
            assert_eq!(Some(result.contraction.count), known_count(n), "N={n}");
            assert!(result.plan.modulus_product > result.plan.factorial_bound);
        }
        for n in [22, 28] {
            let prefix = probe_wide_crt_prefix(n).unwrap();
            assert!(prefix.tail_tasks >= prefix.target_tail_tasks);
            assert!(prefix.split_depth <= n);
            assert_eq!(prefix.plan.n, n);
        }
    }

    #[test]
    fn wide_scalar_fast_path_and_forced_crt_replay_are_explicit_c_exact() {
        for n in 0..=12 {
            let result = contract_rows_wide_scalar(n, true).unwrap();
            assert!(result.used_scalar_u64, "N={n}");
            assert_eq!(Some(result.contraction.count), known_count(n), "N={n}");
            assert!(result.plan.factorial_bound <= u128::from(u64::MAX));
        }
        let promoted = contract_rows_wide_scalar_with_target_and_limit(8, true, 512, 1).unwrap();
        assert!(!promoted.used_scalar_u64);
        assert!(promoted.promotion_reason.is_some());
        assert_eq!(promoted.contraction.count, 92);
        assert_eq!(promoted.residues.len(), promoted.plan.primes.len());
    }

    #[test]
    fn wide_scalar_last_five_and_six_match_generic_c_replay() {
        for n in 0..=10 {
            for microkernel_rows in 4..=6 {
                let result =
                    contract_rows_wide_scalar_last_k_with_target(n, true, 512, microkernel_rows)
                        .unwrap();
                assert!(result.used_scalar_u64, "N={n}, last-k={microkernel_rows}");
                assert_eq!(
                    Some(result.contraction.count),
                    known_count(n),
                    "N={n}, last-k={microkernel_rows}"
                );
            }
        }
        assert!(contract_rows_wide_scalar_last_k_with_target(8, false, 512, 7).is_err());
    }

    #[test]
    fn complete_compiled_and_sitewise_contractions_agree_through_n10() {
        for n in 0..=10 {
            assert_eq!(
                contract_rows(n).unwrap().count,
                contract_rows_sitewise(n).unwrap().count,
                "N={n}"
            );
        }
    }

    #[test]
    fn sort_reduce_matches_hash_materialization_through_n10() {
        for n in 0..=10 {
            let hash = contract_rows_hash_materialization(n).unwrap();
            let sorted = contract_rows_sort_reduce(n).unwrap();
            assert_eq!(sorted.count, hash.count, "count mismatch at N={n}");
            assert_eq!(
                sorted.peak_states, hash.peak_states,
                "support mismatch at N={n}"
            );
            assert_eq!(
                sorted.row_operator_candidates, hash.row_operator_candidates,
                "candidate mismatch at N={n}"
            );
            assert_eq!(
                sorted.row_operator_matched, hash.row_operator_matched,
                "matched mismatch at N={n}"
            );
            let hash_layers = hash
                .layers
                .iter()
                .map(|layer| {
                    (
                        layer.input_states,
                        layer.completed_row_terms,
                        layer.output_states,
                        layer.output_weight,
                    )
                })
                .collect::<Vec<_>>();
            let sorted_layers = sorted
                .layers
                .iter()
                .map(|layer| {
                    (
                        layer.input_states,
                        layer.completed_row_terms,
                        layer.output_states,
                        layer.output_weight,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(sorted_layers, hash_layers, "layer mismatch at N={n}");
        }
    }

    #[test]
    fn parallel_slices_match_serial_sort_reduce_through_n10() {
        for threads in [1, 2, 4] {
            for n in 0..=10 {
                let serial = contract_rows_sort_reduce(n).unwrap();
                let parallel = contract_rows_parallel_sort_reduce(n, threads).unwrap();
                assert_eq!(
                    parallel.count, serial.count,
                    "count mismatch at N={n}, threads={threads}"
                );
                assert_eq!(
                    parallel.peak_states, serial.peak_states,
                    "support mismatch at N={n}, threads={threads}"
                );
                assert_eq!(
                    parallel.row_operator_candidates, serial.row_operator_candidates,
                    "candidate mismatch at N={n}, threads={threads}"
                );
                assert_eq!(
                    parallel.row_operator_matched, serial.row_operator_matched,
                    "matched mismatch at N={n}, threads={threads}"
                );
                let serial_layers = serial
                    .layers
                    .iter()
                    .map(|layer| {
                        (
                            layer.input_states,
                            layer.completed_row_terms,
                            layer.output_states,
                            layer.output_weight,
                        )
                    })
                    .collect::<Vec<_>>();
                let parallel_layers = parallel
                    .layers
                    .iter()
                    .map(|layer| {
                        (
                            layer.input_states,
                            layer.completed_row_terms,
                            layer.output_states,
                            layer.output_weight,
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    parallel_layers, serial_layers,
                    "layer mismatch at N={n}, threads={threads}"
                );
            }
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
    fn rejects_virtual_boundaries_wider_than_u128_layout() {
        assert!(contract_rows(43).is_err());
    }

    #[test]
    fn packed_boundary_round_trips_without_losing_virtual_indices() {
        for n in 1..=42 {
            let mask = (1_u64 << n) - 1;
            let states = [
                BoundaryState {
                    columns: 0,
                    diag_dr: 0,
                    diag_dl: 0,
                },
                BoundaryState {
                    columns: mask,
                    diag_dr: mask,
                    diag_dl: mask,
                },
                BoundaryState {
                    columns: 0x2aaa_aaaa_aaaa & mask,
                    diag_dr: 0x1555_5555_5555 & mask,
                    diag_dl: 0x3333_3333_3333 & mask,
                },
            ];
            for state in states {
                assert_eq!(PackedBoundary::pack(state, n).unpack(n), state, "N={n}");
            }
        }
    }

    #[test]
    fn all_d4_actions_are_distinct_bijections_and_preserve_solutions() {
        let n = 8;
        let sample_solution = [0, 4, 7, 5, 2, 6, 1, 3];
        let mut action_maps = HashSet::new();
        for symmetry in D4Symmetry::ALL {
            let action_map = (0..n)
                .flat_map(|row| {
                    (0..n).map(move |column| symmetry.transform_coordinate(n, row, column))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                action_map.iter().copied().collect::<HashSet<_>>().len(),
                n * n
            );
            action_maps.insert(action_map);

            let transformed = sample_solution
                .iter()
                .enumerate()
                .map(|(row, &column)| symmetry.transform_coordinate(n, row, column))
                .collect::<HashSet<_>>();
            assert_eq!(transformed.len(), n);
            for &(row_a, column_a) in &transformed {
                for &(row_b, column_b) in &transformed {
                    if (row_a, column_a) != (row_b, column_b) {
                        assert_ne!(row_a, row_b);
                        assert_ne!(column_a, column_b);
                        assert_ne!(row_a.abs_diff(row_b), column_a.abs_diff(column_b));
                    }
                }
            }
        }
        assert_eq!(action_maps.len(), 8);
    }

    #[test]
    fn d4_constraint_family_permutations_match_square_geometry() {
        use ConstraintFamily::{Column, DiagDownLeft, DiagDownRight, Row};
        assert_eq!(
            D4Symmetry::Rotate90.transform_constraint_family(Row),
            Column
        );
        assert_eq!(
            D4Symmetry::Rotate90.transform_constraint_family(Column),
            Row
        );
        assert_eq!(
            D4Symmetry::Rotate90.transform_constraint_family(DiagDownRight),
            DiagDownLeft
        );
        assert_eq!(
            D4Symmetry::ReflectVertical.transform_constraint_family(DiagDownLeft),
            DiagDownRight
        );
        assert_eq!(
            D4Symmetry::ReflectMainDiagonal.transform_constraint_family(DiagDownRight),
            DiagDownRight
        );
    }

    #[test]
    fn every_d4_action_preserves_the_explicit_local_b_and_c_tensors() {
        let tensor_b = SiteTensorB::sec_vi();
        let tensor_c = SiteTensorC::sec_vi();
        for symmetry in D4Symmetry::ALL {
            for entry in tensor_b.entries() {
                assert!(
                    tensor_b.entries().iter().any(|candidate| {
                        candidate.alpha == entry.alpha
                            && candidate.value == entry.value
                            && candidate.legs == symmetry.transform_virtual_legs(entry.legs)
                    }),
                    "B is not invariant under {symmetry:?}: {entry:?}"
                );
            }
            for entry in tensor_c.entries() {
                assert!(
                    tensor_c.entries().iter().any(|candidate| {
                        candidate.value == entry.value
                            && candidate.legs == symmetry.transform_virtual_legs(entry.legs)
                    }),
                    "C is not invariant under {symmetry:?}: {entry:?}"
                );
            }
        }
    }

    #[test]
    fn only_identity_and_vertical_reflection_stabilize_interior_row_cuts() {
        for n in 2..=9 {
            for cut in 1..n {
                let stabilizer = D4Symmetry::ALL
                    .into_iter()
                    .filter(|symmetry| symmetry.stabilizes_top_row_cut(n, cut))
                    .collect::<Vec<_>>();
                assert_eq!(
                    stabilizer,
                    vec![D4Symmetry::Identity, D4Symmetry::ReflectVertical],
                    "N={n}, cut={cut}"
                );
            }
        }
    }

    #[test]
    fn top_row_orbit_weights_handle_even_pairs_and_odd_fixed_point() {
        for n in 1..=12 {
            let weights = (0..n)
                .filter_map(|column| {
                    top_row_vertical_orbit_weight(
                        n,
                        BoundaryState {
                            columns: 1_u64 << column,
                            diag_dr: 0,
                            diag_dl: 0,
                        },
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(weights.len(), n.div_ceil(2));
            assert_eq!(weights.iter().sum::<u128>(), n as u128);
            assert_eq!(weights.iter().filter(|&&weight| weight == 1).count(), n % 2);
        }
    }

    #[test]
    fn d4_orbit_slicing_is_exact_and_reduces_support_through_n11() {
        for n in 0..=11 {
            let dense = contract_rows_sort_reduce(n).unwrap();
            let orbit = contract_rows_d4_orbit_sort_reduce(n).unwrap();
            assert_eq!(orbit.count, dense.count, "count mismatch at N={n}");
            if n >= 2 {
                assert!(
                    orbit.peak_states < dense.peak_states,
                    "support did not fall at N={n}"
                );
                if n >= 4 {
                    assert!(
                        orbit.row_operator_matched < dense.row_operator_matched,
                        "accepted work did not fall at N={n}"
                    );
                }
            }
        }
    }

    #[test]
    fn parallel_d4_orbit_slicing_matches_serial_through_n10() {
        for threads in [1, 2, 4] {
            for n in 0..=10 {
                let serial = contract_rows_d4_orbit_sort_reduce(n).unwrap();
                let parallel = contract_rows_d4_orbit_parallel_sort_reduce(n, threads).unwrap();
                assert_eq!(parallel.count, serial.count, "N={n}, threads={threads}");
                assert_eq!(
                    parallel.peak_states, serial.peak_states,
                    "N={n}, threads={threads}"
                );
                assert_eq!(
                    parallel.row_operator_matched, serial.row_operator_matched,
                    "N={n}, threads={threads}"
                );
            }
        }
    }

    #[test]
    fn sparse_and_d4_ablation_variants_are_exact_through_n10() {
        for n in 0..=10 {
            let dense = contract_rows_sort_reduce(n).unwrap();
            let sparse = contract_rows_sparse_sort_reduce(n).unwrap();
            let d4 = contract_rows_d4_orbit_sort_reduce(n).unwrap();
            let d4_sparse = contract_rows_d4_sparse_sort_reduce(n).unwrap();
            assert_eq!(sparse.count, dense.count, "sparse count at N={n}");
            assert_eq!(d4.count, dense.count, "D4 count at N={n}");
            assert_eq!(d4_sparse.count, dense.count, "D4+sparse count at N={n}");
            assert_eq!(
                sparse.peak_states, dense.peak_states,
                "sparse support N={n}"
            );
            assert_eq!(d4_sparse.peak_states, d4.peak_states, "D4 support N={n}");
            assert_eq!(
                sparse.row_operator_candidates, sparse.row_operator_matched,
                "sparse iterator work N={n}"
            );
            assert_eq!(
                d4_sparse.row_operator_candidates, d4_sparse.row_operator_matched,
                "D4+sparse iterator work N={n}"
            );
        }
    }

    #[test]
    fn parallel_sparse_and_d4_interaction_matches_serial_through_n9() {
        for threads in [1, 2, 4] {
            for n in 0..=9 {
                let sparse = contract_rows_sparse_sort_reduce(n).unwrap();
                let sparse_parallel =
                    contract_rows_sparse_parallel_sort_reduce(n, threads).unwrap();
                assert_eq!(sparse_parallel.count, sparse.count, "N={n}, t={threads}");
                assert_eq!(
                    sparse_parallel.peak_states, sparse.peak_states,
                    "N={n}, t={threads}"
                );

                let d4_sparse = contract_rows_d4_sparse_sort_reduce(n).unwrap();
                let d4_sparse_parallel =
                    contract_rows_d4_sparse_parallel_sort_reduce(n, threads).unwrap();
                assert_eq!(
                    d4_sparse_parallel.count, d4_sparse.count,
                    "D4 N={n}, t={threads}"
                );
                assert_eq!(
                    d4_sparse_parallel.peak_states, d4_sparse.peak_states,
                    "D4 N={n}, t={threads}"
                );
            }
        }
    }
}

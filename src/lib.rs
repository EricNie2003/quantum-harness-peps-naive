//! Exact contraction of the N-Queens PEPS from Liu--Liao--Wang, Sec. VI.
//!
//! The rank-9 tensor `B` is constructed explicitly from its 17 non-zero
//! elements. Summing its physical index produces the rank-8 counting tensor
//! `C`. The solver applies sparse entries of `C` site by site and contracts a
//! complete row before moving the boundary down by one lattice spacing.

pub mod dfs_bitmask;

use std::collections::HashMap;
use std::collections::HashSet;
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

/// Packed hash key for the three groups of open virtual indices.
///
/// For a board of width `n`, bits `[0,n)`, `[n,2n)`, and `[2n,3n)` store
/// column, down-right diagonal, and down-left diagonal signals respectively.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PackedBoundary(u128);

type BoundaryMap = HashMap<PackedBoundary, u128>;
type BoundaryObserver<'a> = dyn FnMut(usize, &BoundaryMap) + 'a;

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

#[derive(Clone, Copy)]
enum RowBackend {
    Sitewise,
    Compiled,
}

/// Exactly contract the rank-8 `C` network row by row.
pub fn contract_rows(n: usize) -> Result<ContractionResult, String> {
    contract_rows_with_backend(n, RowBackend::Compiled, None)
}

/// Reference backend retained for tensor-level verification.
pub fn contract_rows_sitewise(n: usize) -> Result<ContractionResult, String> {
    contract_rows_with_backend(n, RowBackend::Sitewise, None)
}

fn contract_rows_with_backend(
    n: usize,
    row_backend: RowBackend,
    mut boundary_observer: Option<&mut BoundaryObserver<'_>>,
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
        if let Some(observer) = boundary_observer.as_deref_mut() {
            observer(row, &next);
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagramReduction {
    Add,
    Zdd,
}

#[derive(Clone, Copy, Debug)]
enum DiagramNode {
    Terminal(u128),
    Branch { variable: u8, low: u32, high: u32 },
}

struct DiagramBuilder {
    reduction: DiagramReduction,
    zero: u32,
    nodes: Vec<DiagramNode>,
    terminals: HashMap<u128, u32>,
    branches: HashMap<(u8, u32, u32), u32>,
}

impl DiagramBuilder {
    fn new(reduction: DiagramReduction) -> Self {
        let mut terminals = HashMap::new();
        terminals.insert(0, 0);
        Self {
            reduction,
            zero: 0,
            nodes: vec![DiagramNode::Terminal(0)],
            terminals,
            branches: HashMap::new(),
        }
    }

    fn terminal(&mut self, value: u128) -> u32 {
        if let Some(&node) = self.terminals.get(&value) {
            return node;
        }
        let node = self.nodes.len() as u32;
        self.nodes.push(DiagramNode::Terminal(value));
        self.terminals.insert(value, node);
        node
    }

    fn branch(&mut self, variable: u8, low: u32, high: u32) -> u32 {
        match self.reduction {
            DiagramReduction::Add if low == high => return low,
            DiagramReduction::Zdd if high == self.zero => return low,
            _ => {}
        }
        let key = (variable, low, high);
        if let Some(&node) = self.branches.get(&key) {
            return node;
        }
        let node = self.nodes.len() as u32;
        self.nodes.push(DiagramNode::Branch {
            variable,
            low,
            high,
        });
        self.branches.insert(key, node);
        node
    }

    fn build(&mut self, entries: &[(u128, u128)], bit_index: i32) -> u32 {
        if entries.is_empty() {
            return self.zero;
        }
        if bit_index < 0 {
            debug_assert_eq!(entries.len(), 1);
            return self.terminal(entries[0].1);
        }

        let selected = 1_u128 << bit_index;
        let middle = entries.partition_point(|(key, _)| key & selected == 0);
        let low = self.build(&entries[..middle], bit_index - 1);
        let high = self.build(&entries[middle..], bit_index - 1);
        self.branch(bit_index as u8, low, high)
    }
}

#[derive(Clone, Copy)]
enum BoundaryVariableOrder {
    Grouped,
    Interleaved,
}

pub struct WeightedBoundaryDiagram {
    reduction: DiagramReduction,
    nodes: Vec<DiagramNode>,
    root: u32,
    total_bits: usize,
}

impl WeightedBoundaryDiagram {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn evaluate(&self, key: u128) -> u128 {
        self.evaluate_node(self.root, key, self.total_bits as i32 - 1)
    }

    fn evaluate_node(&self, node: u32, key: u128, expected_variable: i32) -> u128 {
        match self.nodes[node as usize] {
            DiagramNode::Terminal(value) => {
                if self.reduction == DiagramReduction::Zdd
                    && expected_variable >= 0
                    && key & mask_through(expected_variable) != 0
                {
                    0
                } else {
                    value
                }
            }
            DiagramNode::Branch {
                variable,
                low,
                high,
            } => {
                if self.reduction == DiagramReduction::Zdd {
                    let skipped =
                        mask_through(expected_variable) & !mask_through(i32::from(variable));
                    if key & skipped != 0 {
                        return 0;
                    }
                }
                let child = if key & (1_u128 << variable) == 0 {
                    low
                } else {
                    high
                };
                self.evaluate_node(child, key, i32::from(variable) - 1)
            }
        }
    }
}

fn mask_through(bit_index: i32) -> u128 {
    if bit_index < 0 {
        0
    } else {
        (1_u128 << (bit_index + 1)) - 1
    }
}

fn reordered_boundary_key(packed: PackedBoundary, n: usize, order: BoundaryVariableOrder) -> u128 {
    match order {
        BoundaryVariableOrder::Grouped => packed.0,
        BoundaryVariableOrder::Interleaved => {
            let state = packed.unpack(n);
            let mut reordered = 0_u128;
            for column in 0..n {
                reordered |= u128::from(bit(state.columns, column)) << (3 * column);
                reordered |= u128::from(bit(state.diag_dr, column)) << (3 * column + 1);
                reordered |= u128::from(bit(state.diag_dl, column)) << (3 * column + 2);
            }
            reordered
        }
    }
}

fn build_boundary_diagram(
    boundary: &HashMap<PackedBoundary, u128>,
    n: usize,
    reduction: DiagramReduction,
    order: BoundaryVariableOrder,
) -> WeightedBoundaryDiagram {
    let mut entries: Vec<_> = boundary
        .iter()
        .map(|(&key, &value)| (reordered_boundary_key(key, n, order), value))
        .collect();
    entries.sort_unstable_by_key(|(key, _)| *key);
    let mut builder = DiagramBuilder::new(reduction);
    let root = builder.build(&entries, (3 * n) as i32 - 1);
    WeightedBoundaryDiagram {
        reduction,
        nodes: builder.nodes,
        root,
        total_bits: 3 * n,
    }
}

#[derive(Clone, Debug)]
pub struct BoundaryDiagramLayerMetric {
    pub row: usize,
    pub support: usize,
    pub unique_coefficients: usize,
    pub grouped_add_nodes: usize,
    pub grouped_zdd_nodes: usize,
    pub interleaved_add_nodes: usize,
    pub interleaved_zdd_nodes: usize,
    pub grouped_add_s: f64,
    pub grouped_zdd_s: f64,
    pub interleaved_add_s: f64,
    pub interleaved_zdd_s: f64,
    pub peak_rss_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct BoundaryDiagramProfile {
    pub n: usize,
    pub count: u128,
    pub total_elapsed: Duration,
    pub layers: Vec<BoundaryDiagramLayerMetric>,
}

/// Diagnostic-only exact ADD/ZDD construction over each explicit boundary.
///
/// This profiles whether a decision diagram can compress the exact boundary
/// coefficient function before investing in a diagram-native apply operator.
pub fn profile_boundary_diagrams(n: usize) -> Result<BoundaryDiagramProfile, String> {
    let mut layers = Vec::with_capacity(n);
    let mut observer = |row: usize, boundary: &HashMap<PackedBoundary, u128>| {
        let unique_coefficients = boundary.values().copied().collect::<HashSet<_>>().len();

        let start = Instant::now();
        let grouped_add = build_boundary_diagram(
            boundary,
            n,
            DiagramReduction::Add,
            BoundaryVariableOrder::Grouped,
        );
        let grouped_add_s = start.elapsed().as_secs_f64();

        let start = Instant::now();
        let grouped_zdd = build_boundary_diagram(
            boundary,
            n,
            DiagramReduction::Zdd,
            BoundaryVariableOrder::Grouped,
        );
        let grouped_zdd_s = start.elapsed().as_secs_f64();

        let start = Instant::now();
        let interleaved_add = build_boundary_diagram(
            boundary,
            n,
            DiagramReduction::Add,
            BoundaryVariableOrder::Interleaved,
        );
        let interleaved_add_s = start.elapsed().as_secs_f64();

        let start = Instant::now();
        let interleaved_zdd = build_boundary_diagram(
            boundary,
            n,
            DiagramReduction::Zdd,
            BoundaryVariableOrder::Interleaved,
        );
        let interleaved_zdd_s = start.elapsed().as_secs_f64();

        layers.push(BoundaryDiagramLayerMetric {
            row,
            support: boundary.len(),
            unique_coefficients,
            grouped_add_nodes: grouped_add.node_count(),
            grouped_zdd_nodes: grouped_zdd.node_count(),
            interleaved_add_nodes: interleaved_add.node_count(),
            interleaved_zdd_nodes: interleaved_zdd.node_count(),
            grouped_add_s,
            grouped_zdd_s,
            interleaved_add_s,
            interleaved_zdd_s,
            peak_rss_bytes: peak_rss_bytes(),
        });
    };

    let contraction = contract_rows_with_backend(n, RowBackend::Compiled, Some(&mut observer))?;
    Ok(BoundaryDiagramProfile {
        n,
        count: contraction.count,
        total_elapsed: contraction.elapsed,
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
mod tests {
    use super::{
        BoundaryState, BoundaryVariableOrder, CompiledRowOperator, DiagramReduction,
        PackedBoundary, RowCounters, SiteTensorB, SiteTensorC, VirtualLegs, build_boundary_diagram,
        contract_one_row_compiled, contract_one_row_sitewise, contract_rows,
        contract_rows_sitewise, known_count, profile_boundary_diagrams, reordered_boundary_key,
    };
    use std::collections::HashMap;

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
    fn weighted_add_and_zdd_exactly_represent_sparse_boundary_function() {
        let n = 4;
        let states = [
            (
                BoundaryState {
                    columns: 0b0001,
                    diag_dr: 0b0010,
                    diag_dl: 0b0100,
                },
                3_u128,
            ),
            (
                BoundaryState {
                    columns: 0b1010,
                    diag_dr: 0b0101,
                    diag_dl: 0b0011,
                },
                7_u128,
            ),
            (
                BoundaryState {
                    columns: 0b1111,
                    diag_dr: 0b0000,
                    diag_dl: 0b1000,
                },
                3_u128,
            ),
        ];
        let boundary: HashMap<_, _> = states
            .into_iter()
            .map(|(state, value)| (PackedBoundary::pack(state, n), value))
            .collect();

        for reduction in [DiagramReduction::Add, DiagramReduction::Zdd] {
            for order in [
                BoundaryVariableOrder::Grouped,
                BoundaryVariableOrder::Interleaved,
            ] {
                let diagram = build_boundary_diagram(&boundary, n, reduction, order);
                let expected: HashMap<_, _> = boundary
                    .iter()
                    .map(|(&key, &value)| (reordered_boundary_key(key, n, order), value))
                    .collect();
                for key in 0_u128..(1_u128 << (3 * n)) {
                    assert_eq!(
                        diagram.evaluate(key),
                        expected.get(&key).copied().unwrap_or(0),
                        "reduction={reduction:?}, key={key:012b}"
                    );
                }
            }
        }
    }

    #[test]
    fn diagram_profiler_preserves_q8() {
        let profile = profile_boundary_diagrams(8).unwrap();
        assert_eq!(profile.count, 92);
        assert_eq!(profile.layers.len(), 8);
        assert!(profile.layers.iter().all(|layer| layer.support > 0));
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
}

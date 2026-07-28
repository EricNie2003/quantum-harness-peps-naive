//! Exact algebraic decision-diagram contraction of the Sec. VI PEPS.
//!
//! The row relation is compiled mechanically from the explicit 17-entry `C`.
//! Relational product eliminates input virtual bits during recursion, before a
//! concrete frontier is materialized.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::{SiteTensorC, VirtualLegs, peak_rss_bytes};

type NodeId = usize;

pub const EDGE_QUOTIENT_PRIMES: [u64; 2] = [1_000_000_007, 1_000_000_009];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DdColumnOrder {
    Forward,
    Reverse,
    CenterOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DdOrderMode {
    SiteBlocked,
    Paired,
    FamilyPaired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DdOrderSpec {
    pub families: [usize; 3],
    pub columns: DdColumnOrder,
    pub mode: DdOrderMode,
}

impl Default for DdOrderSpec {
    fn default() -> Self {
        Self {
            families: [0, 1, 2],
            columns: DdColumnOrder::Forward,
            mode: DdOrderMode::SiteBlocked,
        }
    }
}

impl DdOrderSpec {
    pub fn label(self) -> String {
        format!(
            "{:?}-{:?}-{}{}{}",
            self.mode, self.columns, self.families[0], self.families[1], self.families[2]
        )
    }
}

struct VariableLayout {
    input: Vec<[u16; 3]>,
    output: Vec<[u16; 3]>,
    is_input: Vec<bool>,
    output_to_input: Vec<Option<u16>>,
}

impl VariableLayout {
    fn new(n: usize, spec: DdOrderSpec) -> Result<Self, String> {
        let mut columns = (0..n).collect::<Vec<_>>();
        match spec.columns {
            DdColumnOrder::Forward => {}
            DdColumnOrder::Reverse => columns.reverse(),
            DdColumnOrder::CenterOut => {
                columns.sort_unstable_by_key(|&column| (column.abs_diff((n - 1) / 2), column));
            }
        }
        let mut logical = Vec::<(usize, usize)>::with_capacity(3 * n);
        match spec.mode {
            DdOrderMode::FamilyPaired => {
                for family in spec.families {
                    for &column in &columns {
                        logical.push((column, family));
                    }
                }
            }
            DdOrderMode::SiteBlocked | DdOrderMode::Paired => {
                for &column in &columns {
                    for family in spec.families {
                        logical.push((column, family));
                    }
                }
            }
        }
        let mut input = vec![[0_u16; 3]; n];
        let mut output = vec![[0_u16; 3]; n];
        let mut next = 0_u16;
        if spec.mode == DdOrderMode::SiteBlocked {
            for chunk in logical.chunks(3) {
                for &(column, family) in chunk {
                    input[column][family] = next;
                    next += 1;
                }
                for &(column, family) in chunk {
                    output[column][family] = next;
                    next += 1;
                }
            }
        } else {
            for &(column, family) in &logical {
                input[column][family] = next;
                output[column][family] = next + 1;
                next += 2;
            }
        }
        if usize::from(next) != 6 * n {
            return Err("DD variable layout has the wrong size".to_owned());
        }
        let mut is_input = vec![false; 6 * n];
        let mut output_to_input = vec![None; 6 * n];
        for column in 0..n {
            for family in 0..3 {
                is_input[usize::from(input[column][family])] = true;
                output_to_input[usize::from(output[column][family])] = Some(input[column][family]);
            }
        }
        Ok(Self {
            input,
            output,
            is_input,
            output_to_input,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Node {
    Terminal(u128),
    Branch { var: u16, low: NodeId, high: NodeId },
}

#[derive(Default)]
struct AddStore {
    nodes: Vec<Node>,
    terminals: HashMap<u128, NodeId>,
    unique: HashMap<(u16, NodeId, NodeId), NodeId>,
    add_cache: HashMap<(NodeId, NodeId), NodeId>,
    mul_cache: HashMap<(NodeId, NodeId), NodeId>,
    relprod_cache: HashMap<(NodeId, NodeId, u16), NodeId>,
    unique_lookups: u128,
    unique_hits: u128,
    apply_lookups: u128,
    apply_hits: u128,
    relprod_lookups: u128,
    relprod_hits: u128,
}

impl AddStore {
    fn terminal(&mut self, value: u128) -> NodeId {
        if let Some(&node) = self.terminals.get(&value) {
            return node;
        }
        let node = self.nodes.len();
        self.nodes.push(Node::Terminal(value));
        self.terminals.insert(value, node);
        node
    }

    fn branch(&mut self, var: u16, low: NodeId, high: NodeId) -> NodeId {
        if low == high {
            return low;
        }
        self.unique_lookups += 1;
        if let Some(&node) = self.unique.get(&(var, low, high)) {
            self.unique_hits += 1;
            return node;
        }
        let node = self.nodes.len();
        self.nodes.push(Node::Branch { var, low, high });
        self.unique.insert((var, low, high), node);
        node
    }

    fn var(&self, node: NodeId) -> u16 {
        match self.nodes[node] {
            Node::Terminal(_) => u16::MAX,
            Node::Branch { var, .. } => var,
        }
    }

    fn children_at(&self, node: NodeId, var: u16) -> (NodeId, NodeId) {
        match self.nodes[node] {
            Node::Branch {
                var: node_var,
                low,
                high,
            } if node_var == var => (low, high),
            _ => (node, node),
        }
    }

    fn ordered_pair(left: NodeId, right: NodeId) -> (NodeId, NodeId) {
        if left <= right {
            (left, right)
        } else {
            (right, left)
        }
    }

    fn add(&mut self, left: NodeId, right: NodeId) -> Result<NodeId, String> {
        let zero = self.terminal(0);
        if left == zero {
            return Ok(right);
        }
        if right == zero {
            return Ok(left);
        }
        let key = Self::ordered_pair(left, right);
        self.apply_lookups += 1;
        if let Some(&node) = self.add_cache.get(&key) {
            self.apply_hits += 1;
            return Ok(node);
        }
        let result = match (self.nodes[left], self.nodes[right]) {
            (Node::Terminal(a), Node::Terminal(b)) => self.terminal(
                a.checked_add(b)
                    .ok_or_else(|| "ADD terminal addition overflow".to_owned())?,
            ),
            _ => {
                let var = self.var(left).min(self.var(right));
                let (left_low, left_high) = self.children_at(left, var);
                let (right_low, right_high) = self.children_at(right, var);
                let low = self.add(left_low, right_low)?;
                let high = self.add(left_high, right_high)?;
                self.branch(var, low, high)
            }
        };
        self.add_cache.insert(key, result);
        Ok(result)
    }

    fn mul(&mut self, left: NodeId, right: NodeId) -> Result<NodeId, String> {
        let zero = self.terminal(0);
        let one = self.terminal(1);
        if left == zero || right == zero {
            return Ok(zero);
        }
        if left == one {
            return Ok(right);
        }
        if right == one {
            return Ok(left);
        }
        let key = Self::ordered_pair(left, right);
        self.apply_lookups += 1;
        if let Some(&node) = self.mul_cache.get(&key) {
            self.apply_hits += 1;
            return Ok(node);
        }
        let result = match (self.nodes[left], self.nodes[right]) {
            (Node::Terminal(a), Node::Terminal(b)) => self.terminal(
                a.checked_mul(b)
                    .ok_or_else(|| "ADD terminal multiplication overflow".to_owned())?,
            ),
            _ => {
                let var = self.var(left).min(self.var(right));
                let (left_low, left_high) = self.children_at(left, var);
                let (right_low, right_high) = self.children_at(right, var);
                let low = self.mul(left_low, right_low)?;
                let high = self.mul(left_high, right_high)?;
                self.branch(var, low, high)
            }
        };
        self.mul_cache.insert(key, result);
        Ok(result)
    }

    fn relprod(
        &mut self,
        left: NodeId,
        right: NodeId,
        level: u16,
        variables: u16,
        is_input: &[bool],
    ) -> Result<NodeId, String> {
        let zero = self.terminal(0);
        if left == zero || right == zero {
            return Ok(zero);
        }
        if level == variables {
            return self.mul(left, right);
        }
        let pair = Self::ordered_pair(left, right);
        let key = (pair.0, pair.1, level);
        self.relprod_lookups += 1;
        if let Some(&node) = self.relprod_cache.get(&key) {
            self.relprod_hits += 1;
            return Ok(node);
        }
        let top = self.var(left).min(self.var(right));
        let result = if level < top {
            let child = self.relprod(left, right, level + 1, variables, is_input)?;
            if is_input[usize::from(level)] {
                self.add(child, child)?
            } else {
                child
            }
        } else {
            debug_assert_eq!(level, top);
            let (left_low, left_high) = self.children_at(left, top);
            let (right_low, right_high) = self.children_at(right, top);
            let low = self.relprod(left_low, right_low, level + 1, variables, is_input)?;
            let high = self.relprod(left_high, right_high, level + 1, variables, is_input)?;
            if is_input[usize::from(level)] {
                self.add(low, high)?
            } else {
                self.branch(level, low, high)
            }
        };
        self.relprod_cache.insert(key, result);
        Ok(result)
    }

    fn cube(&mut self, assignments: &[(u16, u8)], value: u128) -> Result<NodeId, String> {
        let mut sorted = assignments.to_vec();
        sorted.sort_unstable_by_key(|&(var, _)| var);
        for adjacent in sorted.windows(2) {
            if adjacent[0].0 == adjacent[1].0 && adjacent[0].1 != adjacent[1].1 {
                return Ok(self.terminal(0));
            }
        }
        sorted.dedup();
        let zero = self.terminal(0);
        let mut root = self.terminal(value);
        for &(var, bit) in sorted.iter().rev() {
            root = match bit {
                0 => self.branch(var, root, zero),
                1 => self.branch(var, zero, root),
                _ => return Err("ADD cube bit must be binary".to_owned()),
            };
        }
        Ok(root)
    }

    fn rename_output_to_input(
        &mut self,
        root: NodeId,
        cache: &mut HashMap<NodeId, NodeId>,
        layout: &VariableLayout,
    ) -> Result<NodeId, String> {
        if let Some(&renamed) = cache.get(&root) {
            return Ok(renamed);
        }
        let renamed = match self.nodes[root] {
            Node::Terminal(_) => root,
            Node::Branch { var, low, high } => {
                let Some(renamed_var) = layout.output_to_input[usize::from(var)] else {
                    return Err("relational product retained an input variable".to_owned());
                };
                let low = self.rename_output_to_input(low, cache, layout)?;
                let high = self.rename_output_to_input(high, cache, layout)?;
                self.branch(renamed_var, low, high)
            }
        };
        cache.insert(root, renamed);
        Ok(renamed)
    }

    fn restrict(
        &mut self,
        root: NodeId,
        target: u16,
        value: bool,
        cache: &mut HashMap<(NodeId, u16, bool), NodeId>,
    ) -> NodeId {
        if let Some(&restricted) = cache.get(&(root, target, value)) {
            return restricted;
        }
        let restricted = match self.nodes[root] {
            Node::Terminal(_) => root,
            Node::Branch { var, low, high } if var == target => {
                if value {
                    high
                } else {
                    low
                }
            }
            Node::Branch { var, low, high } if var < target => {
                let low = self.restrict(low, target, value, cache);
                let high = self.restrict(high, target, value, cache);
                self.branch(var, low, high)
            }
            Node::Branch { .. } => root,
        };
        cache.insert((root, target, value), restricted);
        restricted
    }

    fn sum_variable(
        &mut self,
        root: NodeId,
        target: u16,
        cache: &mut HashMap<(NodeId, u16), NodeId>,
    ) -> Result<NodeId, String> {
        if let Some(&summed) = cache.get(&(root, target)) {
            return Ok(summed);
        }
        let summed = match self.nodes[root] {
            Node::Terminal(_) => self.add(root, root)?,
            Node::Branch { var, low, high } if var == target => self.add(low, high)?,
            Node::Branch { var, low, high } if var < target => {
                let low = self.sum_variable(low, target, cache)?;
                let high = self.sum_variable(high, target, cache)?;
                self.branch(var, low, high)
            }
            Node::Branch { .. } => self.add(root, root)?,
        };
        cache.insert((root, target), summed);
        Ok(summed)
    }

    fn reachable_count(&self, roots: &[NodeId]) -> usize {
        let mut seen = HashSet::new();
        let mut stack = roots.to_vec();
        while let Some(node) = stack.pop() {
            if !seen.insert(node) {
                continue;
            }
            if let Node::Branch { low, high, .. } = self.nodes[node] {
                stack.push(low);
                stack.push(high);
            }
        }
        seen.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct WeightedEdge {
    weight: u64,
    node: usize,
}

#[derive(Default)]
struct WeightedQuotientStore {
    unique: HashMap<(u16, WeightedEdge, WeightedEdge), usize>,
    nonzero_edges: usize,
    field_multiplications: u128,
    inversions: u128,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeQuotientMetric {
    pub nodes: usize,
    pub nonzero_edges: usize,
    pub field_multiplications: u128,
    pub inversions: u128,
}

fn field_mul(left: u64, right: u64, prime: u64, operations: &mut u128) -> u64 {
    *operations += 1;
    ((u128::from(left) * u128::from(right)) % u128::from(prime)) as u64
}

fn field_pow(mut base: u64, mut exponent: u64, prime: u64, operations: &mut u128) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = field_mul(result, base, prime, operations);
        }
        base = field_mul(base, base, prime, operations);
        exponent >>= 1;
    }
    result
}

impl WeightedQuotientStore {
    fn branch(
        &mut self,
        var: u16,
        low: WeightedEdge,
        high: WeightedEdge,
        prime: u64,
    ) -> WeightedEdge {
        if low == high {
            return low;
        }
        if low.weight == 0 && high.weight == 0 {
            return WeightedEdge { weight: 0, node: 0 };
        }
        let pivot = if low.weight != 0 {
            low.weight
        } else {
            high.weight
        };
        self.inversions += 1;
        let inverse = field_pow(pivot, prime - 2, prime, &mut self.field_multiplications);
        let normalized_low = WeightedEdge {
            weight: field_mul(low.weight, inverse, prime, &mut self.field_multiplications),
            node: low.node,
        };
        let normalized_high = WeightedEdge {
            weight: field_mul(high.weight, inverse, prime, &mut self.field_multiplications),
            node: high.node,
        };
        if normalized_low == normalized_high {
            return WeightedEdge {
                weight: field_mul(
                    pivot,
                    normalized_low.weight,
                    prime,
                    &mut self.field_multiplications,
                ),
                node: normalized_low.node,
            };
        }
        let key = (var, normalized_low, normalized_high);
        let node = if let Some(&node) = self.unique.get(&key) {
            node
        } else {
            let node = self.unique.len() + 1;
            self.unique.insert(key, node);
            self.nonzero_edges += usize::from(normalized_low.weight != 0);
            self.nonzero_edges += usize::from(normalized_high.weight != 0);
            node
        };
        WeightedEdge {
            weight: pivot,
            node,
        }
    }
}

fn edge_quotient_metric(store: &AddStore, root: NodeId, prime: u64) -> EdgeQuotientMetric {
    fn transform(
        add: &AddStore,
        node: NodeId,
        prime: u64,
        weighted: &mut WeightedQuotientStore,
        cache: &mut HashMap<NodeId, WeightedEdge>,
    ) -> WeightedEdge {
        if let Some(&edge) = cache.get(&node) {
            return edge;
        }
        let edge = match add.nodes[node] {
            Node::Terminal(value) => WeightedEdge {
                weight: (value % u128::from(prime)) as u64,
                node: 0,
            },
            Node::Branch { var, low, high } => {
                let low = transform(add, low, prime, weighted, cache);
                let high = transform(add, high, prime, weighted, cache);
                weighted.branch(var, low, high, prime)
            }
        };
        cache.insert(node, edge);
        edge
    }

    let mut weighted = WeightedQuotientStore::default();
    let mut cache = HashMap::new();
    let _root = transform(store, root, prime, &mut weighted, &mut cache);
    EdgeQuotientMetric {
        nodes: weighted.unique.len() + 1,
        nonzero_edges: weighted.nonzero_edges,
        field_multiplications: weighted.field_multiplications,
        inversions: weighted.inversions,
    }
}

fn occupied(legs: VirtualLegs) -> bool {
    legs.column_in == 0
        && legs.column_out == 1
        && legs.row_in == 0
        && legs.row_out == 1
        && legs.diag_dr_in == 0
        && legs.diag_dr_out == 1
        && legs.diag_dl_in == 0
        && legs.diag_dl_out == 1
}

struct RelationBuild<'a> {
    n: usize,
    tensor: &'a SiteTensorC,
    layout: &'a VariableLayout,
    d4_top: bool,
    memo: HashMap<(usize, u8), NodeId>,
    entries_examined: u128,
    entries_accepted: u128,
}

impl RelationBuild<'_> {
    fn suffix(
        &mut self,
        store: &mut AddStore,
        column: usize,
        row_signal: u8,
    ) -> Result<NodeId, String> {
        if column == self.n {
            return Ok(store.terminal(u128::from(row_signal == 1)));
        }
        if let Some(&root) = self.memo.get(&(column, row_signal)) {
            return Ok(root);
        }
        let zero = store.terminal(0);
        let mut result = zero;
        for entry in self.tensor.entries() {
            self.entries_examined += 1;
            if entry.legs.row_in != row_signal {
                continue;
            }
            let is_occupied = occupied(entry.legs);
            let weight = if self.d4_top && is_occupied {
                let mirror = self.n - 1 - column;
                if column > mirror {
                    continue;
                } else if column < mirror {
                    2
                } else {
                    1
                }
            } else {
                1
            };
            self.entries_accepted += 1;
            let mut assignments = vec![
                (self.layout.input[column][0], entry.legs.column_in),
                (self.layout.input[column][1], entry.legs.diag_dr_in),
                (self.layout.input[column][2], entry.legs.diag_dl_in),
                (self.layout.output[column][0], entry.legs.column_out),
            ];
            if column + 1 < self.n {
                assignments.push((self.layout.output[column + 1][1], entry.legs.diag_dr_out));
            }
            if column > 0 {
                assignments.push((self.layout.output[column - 1][2], entry.legs.diag_dl_out));
            }
            let local = store.cube(
                &assignments,
                entry
                    .value
                    .checked_mul(weight)
                    .ok_or_else(|| "D4 relation weight overflow".to_owned())?,
            )?;
            let suffix = self.suffix(store, column + 1, entry.legs.row_out)?;
            let term = store.mul(local, suffix)?;
            result = store.add(result, term)?;
        }
        self.memo.insert((column, row_signal), result);
        Ok(result)
    }
}

fn build_relation(
    store: &mut AddStore,
    n: usize,
    tensor: &SiteTensorC,
    layout: &VariableLayout,
    d4_top: bool,
) -> Result<(NodeId, u128, u128), String> {
    let mut build = RelationBuild {
        n,
        tensor,
        layout,
        d4_top,
        memo: HashMap::new(),
        entries_examined: 0,
        entries_accepted: 0,
    };
    let relation = build.suffix(store, 0, 0)?;
    let fixed_edges = store.cube(&[(layout.output[0][1], 0), (layout.output[n - 1][2], 0)], 1)?;
    Ok((
        store.mul(relation, fixed_edges)?,
        build.entries_examined,
        build.entries_accepted,
    ))
}

#[derive(Clone, Debug)]
pub struct DdLayerMetric {
    pub row: usize,
    pub boundary_nodes: usize,
    pub edge_quotients: [EdgeQuotientMetric; 2],
}

#[derive(Clone, Debug)]
pub struct WeightedDdResult {
    pub n: usize,
    pub order: String,
    pub count: u128,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
    pub peak_live_nodes: usize,
    pub peak_boundary_nodes: usize,
    pub peak_edge_quotient_nodes: [usize; 2],
    pub edge_quotient_field_multiplications: [u128; 2],
    pub edge_quotient_inversions: [u128; 2],
    pub peak_relation_nodes: usize,
    pub allocated_nodes: usize,
    pub terminal_count: usize,
    pub tensor_entries_examined: u128,
    pub tensor_entries_accepted: u128,
    pub unique_lookups: u128,
    pub unique_hits: u128,
    pub apply_lookups: u128,
    pub apply_hits: u128,
    pub relprod_lookups: u128,
    pub relprod_hits: u128,
    pub layers: Vec<DdLayerMetric>,
}

pub fn contract_weighted_dd_with_order(
    n: usize,
    order: DdOrderSpec,
) -> Result<WeightedDdResult, String> {
    if n == 0 {
        return Ok(WeightedDdResult {
            n,
            order: order.label(),
            count: 1,
            elapsed: Duration::ZERO,
            peak_rss_bytes: peak_rss_bytes(),
            peak_live_nodes: 1,
            peak_boundary_nodes: 1,
            peak_edge_quotient_nodes: [1, 1],
            edge_quotient_field_multiplications: [0, 0],
            edge_quotient_inversions: [0, 0],
            peak_relation_nodes: 1,
            allocated_nodes: 1,
            terminal_count: 1,
            tensor_entries_examined: 0,
            tensor_entries_accepted: 0,
            unique_lookups: 0,
            unique_hits: 0,
            apply_lookups: 0,
            apply_hits: 0,
            relprod_lookups: 0,
            relprod_hits: 0,
            layers: Vec::new(),
        });
    }
    if n > 21 {
        return Err("weighted DD currently supports N<=21 variable indices".to_owned());
    }
    let start = Instant::now();
    let layout = VariableLayout::new(n, order)?;
    let tensor = SiteTensorC::sec_vi();
    let mut store = AddStore::default();
    let (normal_relation, normal_examined, normal_accepted) =
        build_relation(&mut store, n, &tensor, &layout, false)?;
    let (top_relation, top_examined, top_accepted) =
        build_relation(&mut store, n, &tensor, &layout, true)?;
    let mut initial = Vec::with_capacity(3 * n);
    for column in 0..n {
        for family in 0..3 {
            initial.push((layout.input[column][family], 0_u8));
        }
    }
    let mut boundary = store.cube(&initial, 1)?;
    let peak_relation_nodes = store
        .reachable_count(&[normal_relation])
        .max(store.reachable_count(&[top_relation]));
    let variables = (6 * n) as u16;
    let mut peak_live_nodes = store.reachable_count(&[boundary, normal_relation, top_relation]);
    let mut peak_boundary_nodes = store.reachable_count(&[boundary]);
    let mut peak_edge_quotient_nodes = [1_usize; 2];
    let mut edge_quotient_field_multiplications = [0_u128; 2];
    let mut edge_quotient_inversions = [0_u128; 2];
    let mut layers = Vec::with_capacity(n);

    for row in 0..n {
        let relation = if row == 0 {
            top_relation
        } else {
            normal_relation
        };
        let output = store.relprod(boundary, relation, 0, variables, &layout.is_input)?;
        let mut rename_cache = HashMap::new();
        boundary = store.rename_output_to_input(output, &mut rename_cache, &layout)?;
        let boundary_nodes = store.reachable_count(&[boundary]);
        let edge_quotients =
            EDGE_QUOTIENT_PRIMES.map(|prime| edge_quotient_metric(&store, boundary, prime));
        for index in 0..2 {
            peak_edge_quotient_nodes[index] =
                peak_edge_quotient_nodes[index].max(edge_quotients[index].nodes);
            edge_quotient_field_multiplications[index] +=
                edge_quotients[index].field_multiplications;
            edge_quotient_inversions[index] += edge_quotients[index].inversions;
        }
        peak_boundary_nodes = peak_boundary_nodes.max(boundary_nodes);
        peak_live_nodes = peak_live_nodes.max(store.reachable_count(&[boundary, relation]));
        layers.push(DdLayerMetric {
            row,
            boundary_nodes,
            edge_quotients,
        });
    }

    let mut final_root = boundary;
    let mut restrict_cache = HashMap::new();
    for column in 0..n {
        final_root = store.restrict(
            final_root,
            layout.input[column][0],
            true,
            &mut restrict_cache,
        );
    }
    let mut sum_cache = HashMap::new();
    for column in 0..n {
        final_root = store.sum_variable(final_root, layout.input[column][1], &mut sum_cache)?;
        final_root = store.sum_variable(final_root, layout.input[column][2], &mut sum_cache)?;
    }
    let count = match store.nodes[final_root] {
        Node::Terminal(value) => value,
        Node::Branch { .. } => {
            return Err("final weighted DD contraction retained virtual variables".to_owned());
        }
    };
    Ok(WeightedDdResult {
        n,
        order: order.label(),
        count,
        elapsed: start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
        peak_live_nodes,
        peak_boundary_nodes,
        peak_edge_quotient_nodes,
        edge_quotient_field_multiplications,
        edge_quotient_inversions,
        peak_relation_nodes,
        allocated_nodes: store.nodes.len(),
        terminal_count: store.terminals.len(),
        tensor_entries_examined: normal_examined + top_examined,
        tensor_entries_accepted: normal_accepted + top_accepted,
        unique_lookups: store.unique_lookups,
        unique_hits: store.unique_hits,
        apply_lookups: store.apply_lookups,
        apply_hits: store.apply_hits,
        relprod_lookups: store.relprod_lookups,
        relprod_hits: store.relprod_hits,
        layers,
    })
}

pub fn contract_weighted_dd(n: usize) -> Result<WeightedDdResult, String> {
    contract_weighted_dd_with_order(n, DdOrderSpec::default())
}

#[cfg(test)]
mod tests {
    use super::contract_weighted_dd;
    use crate::known_count;

    #[test]
    fn weighted_dd_matches_known_counts_through_n6() {
        for n in 0..=6 {
            let result = contract_weighted_dd(n).unwrap();
            assert_eq!(result.count, known_count(n).unwrap(), "N={n}");
            assert_eq!(
                result.peak_edge_quotient_nodes[0], result.peak_edge_quotient_nodes[1],
                "N={n}"
            );
            assert!(result.peak_edge_quotient_nodes[0] <= result.peak_boundary_nodes);
        }
    }
}

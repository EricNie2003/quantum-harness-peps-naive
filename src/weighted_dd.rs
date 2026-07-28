//! Exact algebraic decision-diagram contraction of the Sec. VI PEPS.
//!
//! The row relation is compiled mechanically from the explicit 17-entry `C`.
//! Relational product eliminates input virtual bits during recursion, before a
//! concrete frontier is materialized.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::{SiteTensorC, VirtualLegs, peak_rss_bytes};

type NodeId = usize;

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
            let child = self.relprod(left, right, level + 1, variables)?;
            if level % 6 < 3 {
                self.add(child, child)?
            } else {
                child
            }
        } else {
            debug_assert_eq!(level, top);
            let (left_low, left_high) = self.children_at(left, top);
            let (right_low, right_high) = self.children_at(right, top);
            let low = self.relprod(left_low, right_low, level + 1, variables)?;
            let high = self.relprod(left_high, right_high, level + 1, variables)?;
            if level % 6 < 3 {
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
    ) -> Result<NodeId, String> {
        if let Some(&renamed) = cache.get(&root) {
            return Ok(renamed);
        }
        let renamed = match self.nodes[root] {
            Node::Terminal(_) => root,
            Node::Branch { var, low, high } => {
                if var % 6 < 3 {
                    return Err("relational product retained an input variable".to_owned());
                }
                let low = self.rename_output_to_input(low, cache)?;
                let high = self.rename_output_to_input(high, cache)?;
                self.branch(var - 3, low, high)
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

fn input_var(column: usize, family: usize) -> u16 {
    (6 * column + family) as u16
}

fn output_var(column: usize, family: usize) -> u16 {
    (6 * column + 3 + family) as u16
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
                (input_var(column, 0), entry.legs.column_in),
                (input_var(column, 1), entry.legs.diag_dr_in),
                (input_var(column, 2), entry.legs.diag_dl_in),
                (output_var(column, 0), entry.legs.column_out),
            ];
            if column + 1 < self.n {
                assignments.push((output_var(column + 1, 1), entry.legs.diag_dr_out));
            }
            if column > 0 {
                assignments.push((output_var(column - 1, 2), entry.legs.diag_dl_out));
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
    d4_top: bool,
) -> Result<(NodeId, u128, u128), String> {
    let mut build = RelationBuild {
        n,
        tensor,
        d4_top,
        memo: HashMap::new(),
        entries_examined: 0,
        entries_accepted: 0,
    };
    let relation = build.suffix(store, 0, 0)?;
    let fixed_edges = store.cube(&[(output_var(0, 1), 0), (output_var(n - 1, 2), 0)], 1)?;
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
}

#[derive(Clone, Debug)]
pub struct WeightedDdResult {
    pub n: usize,
    pub count: u128,
    pub elapsed: Duration,
    pub peak_rss_bytes: u64,
    pub peak_live_nodes: usize,
    pub peak_boundary_nodes: usize,
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

pub fn contract_weighted_dd(n: usize) -> Result<WeightedDdResult, String> {
    if n == 0 {
        return Ok(WeightedDdResult {
            n,
            count: 1,
            elapsed: Duration::ZERO,
            peak_rss_bytes: peak_rss_bytes(),
            peak_live_nodes: 1,
            peak_boundary_nodes: 1,
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
    let tensor = SiteTensorC::sec_vi();
    let mut store = AddStore::default();
    let (normal_relation, normal_examined, normal_accepted) =
        build_relation(&mut store, n, &tensor, false)?;
    let (top_relation, top_examined, top_accepted) = build_relation(&mut store, n, &tensor, true)?;
    let initial = (0..n)
        .flat_map(|column| (0..3).map(move |family| (input_var(column, family), 0_u8)))
        .collect::<Vec<_>>();
    let mut boundary = store.cube(&initial, 1)?;
    let peak_relation_nodes = store
        .reachable_count(&[normal_relation])
        .max(store.reachable_count(&[top_relation]));
    let variables = (6 * n) as u16;
    let mut peak_live_nodes = store.reachable_count(&[boundary, normal_relation, top_relation]);
    let mut peak_boundary_nodes = store.reachable_count(&[boundary]);
    let mut layers = Vec::with_capacity(n);

    for row in 0..n {
        let relation = if row == 0 {
            top_relation
        } else {
            normal_relation
        };
        let output = store.relprod(boundary, relation, 0, variables)?;
        let mut rename_cache = HashMap::new();
        boundary = store.rename_output_to_input(output, &mut rename_cache)?;
        let boundary_nodes = store.reachable_count(&[boundary]);
        peak_boundary_nodes = peak_boundary_nodes.max(boundary_nodes);
        peak_live_nodes = peak_live_nodes.max(store.reachable_count(&[boundary, relation]));
        layers.push(DdLayerMetric {
            row,
            boundary_nodes,
        });
    }

    let mut final_root = boundary;
    let mut restrict_cache = HashMap::new();
    for column in 0..n {
        final_root = store.restrict(final_root, input_var(column, 0), true, &mut restrict_cache);
    }
    let mut sum_cache = HashMap::new();
    for column in 0..n {
        final_root = store.sum_variable(final_root, input_var(column, 1), &mut sum_cache)?;
        final_root = store.sum_variable(final_root, input_var(column, 2), &mut sum_cache)?;
    }
    let count = match store.nodes[final_root] {
        Node::Terminal(value) => value,
        Node::Branch { .. } => {
            return Err("final weighted DD contraction retained virtual variables".to_owned());
        }
    };
    Ok(WeightedDdResult {
        n,
        count,
        elapsed: start.elapsed(),
        peak_rss_bytes: peak_rss_bytes(),
        peak_live_nodes,
        peak_boundary_nodes,
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

#[cfg(test)]
mod tests {
    use super::contract_weighted_dd;
    use crate::known_count;

    #[test]
    fn weighted_dd_matches_known_counts_through_n6() {
        for n in 0..=6 {
            assert_eq!(
                contract_weighted_dd(n).unwrap().count,
                known_count(n).unwrap(),
                "N={n}"
            );
        }
    }
}

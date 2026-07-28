//! Independent exact bitboard DFS comparator for N-Queens.
//!
//! This is deliberately separate from the Sec. VI tensor-network contraction.
//! It is an optimized conventional search oracle/baseline and is not a PEPS
//! implementation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The implementation uses one bit per column in a `u64`. We deliberately
/// cap the public comparator at the largest known-count validation point in
/// this repository; every recursive accumulation is nevertheless checked.
pub const MAX_N: usize = 27;

#[derive(Clone, Copy, Debug)]
struct Task {
    columns: u64,
    diag_left: u64,
    diag_right: u64,
    symmetry_weight: u8,
    rows_placed: u8,
}

#[derive(Clone, Debug)]
pub struct DfsResult {
    pub n: usize,
    pub count: u128,
    pub elapsed: Duration,
    pub threads: usize,
    pub split_depth: usize,
    pub tasks: usize,
    pub recursive_nodes: u128,
    pub candidate_placements: u128,
    pub metrics_collected: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct SearchMetrics {
    recursive_nodes: u64,
    candidate_placements: u64,
}

impl SearchMetrics {
    #[inline(always)]
    fn record_node(&mut self) {
        self.recursive_nodes = self
            .recursive_nodes
            .checked_add(1)
            .expect("DFS node counter overflow");
    }

    #[inline(always)]
    fn record_candidates(&mut self, count: u32) {
        self.candidate_placements = self
            .candidate_placements
            .checked_add(u64::from(count))
            .expect("DFS candidate counter overflow");
    }
}

/// Richards-style bitboard recursion.
///
/// `diag_left` and `diag_right` are the squares attacked in the current row.
/// Only the low `n` bits selected by `board_mask` participate. The last row is
/// counted directly, avoiding a final recursive call per solution.
#[inline(always)]
fn search<const TRACK_METRICS: bool>(
    board_mask: u64,
    columns: u64,
    diag_left: u64,
    diag_right: u64,
    rows_left: u8,
    metrics: &mut SearchMetrics,
) -> u64 {
    if TRACK_METRICS {
        metrics.record_node();
    }
    let mut available = board_mask & !(columns | diag_left | diag_right);

    if rows_left == 1 {
        let has_leaf = u64::from(available != 0);
        if TRACK_METRICS {
            metrics.record_candidates(has_leaf as u32);
        }
        return has_leaf;
    }

    if TRACK_METRICS {
        metrics.record_candidates(available.count_ones());
    }
    let mut count = 0_u64;
    while available != 0 {
        // Isolate and clear the least-significant available column.
        let bit = available & available.wrapping_neg();
        available ^= bit;
        let child = search::<TRACK_METRICS>(
            board_mask,
            columns | bit,
            ((diag_left | bit) << 1) & board_mask,
            (diag_right | bit) >> 1,
            rows_left - 1,
            metrics,
        );
        count = count
            .checked_add(child)
            .expect("DFS subtree count overflowed u64");
    }
    count
}

fn seed_tasks(n: usize) -> Vec<Task> {
    if n == 0 {
        return Vec::new();
    }

    let mut tasks = Vec::with_capacity(n.div_ceil(2));
    for column in 0..(n / 2) {
        let bit = 1_u64 << column;
        tasks.push(Task {
            columns: bit,
            diag_left: bit << 1,
            diag_right: bit >> 1,
            symmetry_weight: 2,
            rows_placed: 1,
        });
    }
    if n % 2 == 1 {
        let bit = 1_u64 << (n / 2);
        tasks.push(Task {
            columns: bit,
            diag_left: bit << 1,
            diag_right: bit >> 1,
            symmetry_weight: 1,
            rows_placed: 1,
        });
    }
    tasks
}

fn split_once(board_mask: u64, tasks: Vec<Task>) -> Vec<Task> {
    let mut children = Vec::new();
    for task in tasks {
        let mut available = board_mask & !(task.columns | task.diag_left | task.diag_right);
        while available != 0 {
            let bit = available & available.wrapping_neg();
            available ^= bit;
            children.push(Task {
                columns: task.columns | bit,
                diag_left: ((task.diag_left | bit) << 1) & board_mask,
                diag_right: (task.diag_right | bit) >> 1,
                symmetry_weight: task.symmetry_weight,
                rows_placed: task.rows_placed + 1,
            });
        }
    }
    children
}

fn make_tasks(n: usize, requested_threads: usize) -> (Vec<Task>, usize, SearchMetrics) {
    let board_mask = (1_u64 << n) - 1;
    let mut tasks = seed_tasks(n);
    let mut prefix_metrics = SearchMetrics {
        recursive_nodes: 1,
        candidate_placements: tasks.len() as u64,
    };
    let target_tasks = if requested_threads == 1 {
        1
    } else {
        requested_threads.saturating_mul(64)
    };
    let mut split_depth = usize::from(tasks.first().map_or(0, |task| task.rows_placed));

    // One task is optimal for low-overhead serial search. Parallel runs split
    // far enough to give dynamic scheduling useful load-balancing granularity.
    while tasks.len() < target_tasks && split_depth < n.saturating_sub(1) {
        prefix_metrics.recursive_nodes = prefix_metrics
            .recursive_nodes
            .checked_add(tasks.len() as u64)
            .expect("DFS prefix node counter overflow");
        tasks = split_once(board_mask, tasks);
        prefix_metrics.candidate_placements = prefix_metrics
            .candidate_placements
            .checked_add(tasks.len() as u64)
            .expect("DFS prefix candidate counter overflow");
        split_depth += 1;
        if tasks.is_empty() {
            break;
        }
    }

    // Harder-looking prefixes first reduce the chance that one expensive task
    // is left at the tail of a parallel run.
    tasks.sort_unstable_by_key(|task| {
        let available = board_mask & !(task.columns | task.diag_left | task.diag_right);
        std::cmp::Reverse(available.count_ones())
    });
    (tasks, split_depth, prefix_metrics)
}

fn run_task<const TRACK_METRICS: bool>(
    board_mask: u64,
    n: usize,
    task: Task,
) -> (u128, SearchMetrics) {
    let mut metrics = SearchMetrics::default();
    let rows_left = (n - usize::from(task.rows_placed)) as u8;
    let raw_count = if rows_left == 0 {
        1
    } else {
        search::<TRACK_METRICS>(
            board_mask,
            task.columns,
            task.diag_left,
            task.diag_right,
            rows_left,
            &mut metrics,
        )
    };
    let weighted = u128::from(raw_count)
        .checked_mul(u128::from(task.symmetry_weight))
        .expect("symmetry-weighted DFS count overflow");
    (weighted, metrics)
}

/// Count N-Queens solutions with an independent symmetry-reduced bitboard DFS.
///
/// The timer includes prefix generation, worker creation, search, and result
/// reduction. `threads=1` runs without creating a worker thread.
fn count_dfs_bitmask_impl<const TRACK_METRICS: bool>(
    n: usize,
    threads: usize,
) -> Result<DfsResult, String> {
    if n > MAX_N {
        return Err(format!("DFS bitmask baseline supports N <= {MAX_N}"));
    }
    if threads == 0 {
        return Err("thread count must be positive".to_owned());
    }

    let started = Instant::now();
    if n == 0 {
        return Ok(DfsResult {
            n,
            count: 1,
            elapsed: started.elapsed(),
            threads: 1,
            split_depth: 0,
            tasks: 1,
            recursive_nodes: 1,
            candidate_placements: 0,
            metrics_collected: TRACK_METRICS,
        });
    }

    let board_mask = (1_u64 << n) - 1;
    let (tasks, split_depth, prefix_metrics) = make_tasks(n, threads);
    let worker_count = threads.min(tasks.len()).max(1);
    let mut total_count = 0_u128;
    let mut total_nodes = if TRACK_METRICS {
        u128::from(prefix_metrics.recursive_nodes)
    } else {
        0
    };
    let mut total_candidates = if TRACK_METRICS {
        u128::from(prefix_metrics.candidate_placements)
    } else {
        0
    };

    if worker_count == 1 {
        for &task in &tasks {
            let (count, metrics) = run_task::<TRACK_METRICS>(board_mask, n, task);
            total_count = total_count
                .checked_add(count)
                .ok_or_else(|| "total DFS count overflowed u128".to_owned())?;
            total_nodes = total_nodes
                .checked_add(u128::from(metrics.recursive_nodes))
                .ok_or_else(|| "total DFS node count overflowed u128".to_owned())?;
            total_candidates = total_candidates
                .checked_add(u128::from(metrics.candidate_placements))
                .ok_or_else(|| "total DFS candidate count overflowed u128".to_owned())?;
        }
    } else {
        let next_task = AtomicUsize::new(0);
        let partials = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                handles.push(scope.spawn(|| {
                    let mut subtotal = 0_u128;
                    let mut nodes = 0_u128;
                    let mut candidates = 0_u128;
                    loop {
                        let index = next_task.fetch_add(1, Ordering::Relaxed);
                        let Some(&task) = tasks.get(index) else {
                            break;
                        };
                        let (count, metrics) = run_task::<TRACK_METRICS>(board_mask, n, task);
                        subtotal = subtotal
                            .checked_add(count)
                            .expect("worker DFS count overflowed u128");
                        nodes = nodes
                            .checked_add(u128::from(metrics.recursive_nodes))
                            .expect("worker DFS node count overflowed u128");
                        candidates = candidates
                            .checked_add(u128::from(metrics.candidate_placements))
                            .expect("worker DFS candidate count overflowed u128");
                    }
                    (subtotal, nodes, candidates)
                }));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().expect("DFS worker panicked"))
                .collect::<Vec<_>>()
        });
        for (count, nodes, candidates) in partials {
            total_count = total_count
                .checked_add(count)
                .ok_or_else(|| "total DFS count overflowed u128".to_owned())?;
            total_nodes = total_nodes
                .checked_add(nodes)
                .ok_or_else(|| "total DFS node count overflowed u128".to_owned())?;
            total_candidates = total_candidates
                .checked_add(candidates)
                .ok_or_else(|| "total DFS candidate count overflowed u128".to_owned())?;
        }
    }

    Ok(DfsResult {
        n,
        count: total_count,
        elapsed: started.elapsed(),
        threads: worker_count,
        split_depth,
        tasks: tasks.len(),
        recursive_nodes: total_nodes,
        candidate_placements: total_candidates,
        metrics_collected: TRACK_METRICS,
    })
}

/// Fast benchmark path. Expensive per-node counters are compiled out.
pub fn count_dfs_bitmask(n: usize, threads: usize) -> Result<DfsResult, String> {
    count_dfs_bitmask_impl::<false>(n, threads)
}

/// Instrumented path used once per benchmark point to obtain processed-state
/// metrics. Its elapsed time is intentionally not used as the performance
/// sample because incrementing counters materially changes the hot loop.
pub fn profile_dfs_bitmask(n: usize, threads: usize) -> Result<DfsResult, String> {
    count_dfs_bitmask_impl::<true>(n, threads)
}

#[cfg(test)]
mod tests {
    use super::{MAX_N, count_dfs_bitmask, profile_dfs_bitmask};
    use crate::known_count;

    #[test]
    fn dfs_matches_known_counts_through_sixteen() {
        for n in 0..=16 {
            let result = count_dfs_bitmask(n, 1).unwrap();
            assert_eq!(result.count, known_count(n).unwrap(), "N={n}");
        }
    }

    #[test]
    fn parallel_and_serial_search_agree() {
        for n in 8..=14 {
            assert_eq!(
                count_dfs_bitmask(n, 1).unwrap().count,
                count_dfs_bitmask(n, 4).unwrap().count,
                "N={n}"
            );
        }
    }

    #[test]
    fn profiling_counts_the_same_search_tree_across_task_splits() {
        let serial = profile_dfs_bitmask(12, 1).unwrap();
        let parallel = profile_dfs_bitmask(12, 4).unwrap();
        assert_eq!(serial.recursive_nodes, parallel.recursive_nodes);
        assert_eq!(serial.candidate_placements, parallel.candidate_placements);
    }

    #[test]
    fn rejects_zero_threads_and_oversized_boards() {
        assert!(count_dfs_bitmask(8, 0).is_err());
        assert!(count_dfs_bitmask(MAX_N + 1, 1).is_err());
    }
}

#!/usr/bin/env python3
"""Normalize the same-node Issue #34 comparison and draw publication SVGs.

The input CSVs remain the authoritative raw measurements.  This script joins
each row to the GNU ``time -v`` record from the same fresh process, preserves
both TreeSA planning and executor costs, and refuses duplicate N values within
an algorithm family.
"""

from __future__ import annotations

import argparse
import csv
import html
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


@dataclass(frozen=True)
class Family:
    key: str
    label: str
    csv_prefix: str
    role: str


DFS = Family(
    "dfs",
    "DFS bitmask comparator",
    "comparison_dfs_scnet_",
    "independent_non_tensor_comparator",
)
NAIVE = Family(
    "naive_peps",
    "Naive exact PEPS (20b5334)",
    "comparison_naive20b_scnet_",
    "explicit_C_hashmap_row_contraction",
)
PEPS = Family(
    "latest_peps",
    "Latest exact PEPS, no TreeSA",
    "comparison_e50_scnet_",
    "proved_equivalent_explicit_C_CRT_contraction",
)
TREESA = Family(
    "treesa_peps",
    "Exact site-tree PEPS with TreeSA",
    "comparison_treesa_scnet_",
    "explicit_C_D4_site_tree_contraction",
)


# Audited against the repository's exact u128 table and OEIS A000170.  Keep this
# independent of the input CSVs: some production E50 rows intentionally omit a
# ``known_count`` column, so defaulting the reference to the measured value
# would make ``verified=true`` circular.
KNOWN_COUNTS = {
    0: "1",
    1: "1",
    2: "0",
    3: "0",
    4: "2",
    5: "10",
    6: "4",
    7: "40",
    8: "92",
    9: "352",
    10: "724",
    11: "2680",
    12: "14200",
    13: "73712",
    14: "365596",
    15: "2279184",
    16: "14772512",
    17: "95815104",
    18: "666090624",
    19: "4968057848",
    20: "39029188884",
    21: "314666222712",
    22: "2691008701644",
    23: "24233937684440",
    24: "227514171973736",
    25: "2207893435808352",
    26: "22317699616364044",
    27: "234907967154122528",
}

EXPECTED_N = {
    "dfs": set(range(1, 23)),
    "naive_peps": set(range(1, 17)),
    "latest_peps": set(range(1, 23)),
    "treesa_peps": set(range(2, 12)),
}
EXPECTED_SOURCE = {
    "dfs": ("b89f4f1320bdbe9e0fcce700b9d98b879f012bea", ""),
    "naive_peps": ("20b5334f55819ab0b4bdce7aa701527de736c3dc", ""),
    "latest_peps": ("fc0921b00f1b700b3f6a3930a43cb48806afd3b8", "ea5b985"),
    "treesa_peps": ("c715e36e835a1890055c09fad34ca7c2e854bf0d", "e9a80a5"),
}
EXPECTED_REQUESTED_THREADS = {
    "dfs": "128",
    "naive_peps": "1",
    "latest_peps": "128",
    "treesa_peps": "1",
}
EXPECTED_NODE = "b10r4n19"


def expand_csvs(inputs: Iterable[Path], family: Family) -> list[Path]:
    paths: list[Path] = []
    for item in inputs:
        if item.is_dir():
            paths.extend(sorted(item.glob(f"{family.csv_prefix}*.csv")))
        elif item.is_file():
            paths.append(item)
        else:
            raise FileNotFoundError(item)
    if not paths:
        raise ValueError(f"no CSV inputs found for {family.key}")
    return paths


def read_key_values(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key.strip()] = value.strip()
    return values


def parse_wall_clock(value: str) -> float:
    parts = value.strip().split(":")
    if len(parts) == 2:
        minutes, seconds = parts
        return 60.0 * float(minutes) + float(seconds)
    if len(parts) == 3:
        hours, minutes, seconds = parts
        return 3600.0 * float(hours) + 60.0 * float(minutes) + float(seconds)
    raise ValueError(f"unrecognized GNU time elapsed value: {value!r}")


def read_gnu_time(path: Path) -> tuple[float, int]:
    elapsed: float | None = None
    rss_kib: int | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped.startswith("Elapsed (wall clock) time"):
            elapsed = parse_wall_clock(stripped.split("):", 1)[1])
        elif stripped.startswith("Maximum resident set size (kbytes):"):
            rss_kib = int(stripped.rsplit(":", 1)[1].strip())
    if elapsed is None or rss_kib is None:
        raise ValueError(f"{path}: incomplete GNU time -v record")
    return elapsed, rss_kib * 1024


def point_time_path(csv_path: Path, n: int, suffix: str = "time.txt") -> Path:
    point_path = csv_path.parent / f"{csv_path.stem}_points" / f"n{n}.{suffix}"
    if point_path.exists():
        return point_path
    whole_job_path = csv_path.with_suffix(".time.txt")
    if whole_job_path.exists():
        return whole_job_path
    raise FileNotFoundError(f"no GNU time record for N={n} beside {csv_path}")


def optional(row: dict[str, str], key: str, default: str = "") -> str:
    value = row.get(key, default)
    return default if value is None else value


def load_family(inputs: Iterable[Path], family: Family) -> list[dict[str, str]]:
    normalized: list[dict[str, str]] = []
    seen: set[int] = set()
    for csv_path in expand_csvs(inputs, family):
        metadata = read_key_values(csv_path.with_suffix(".meta.txt"))
        with csv_path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
        if not rows:
            raise ValueError(f"{csv_path}: empty CSV")
        for row in rows:
            n = int(row["N"])
            if n in seen:
                raise ValueError(f"duplicate {family.key} N={n}: {csv_path}")
            seen.add(n)

            if optional(row, "verified").lower() != "true":
                raise ValueError(f"{csv_path}: unverified {family.key} N={n}")

            if family is TREESA:
                plan_time_path = point_time_path(csv_path, n, "plan.time.txt")
                executor_time_path = point_time_path(csv_path, n, "executor.time.txt")
                plan_wall, plan_rss = read_gnu_time(plan_time_path)
                executor_wall, executor_rss = read_gnu_time(executor_time_path)
                execution = float(row["executor_elapsed_s"])
                execution_min = row["executor_elapsed_s"]
                execution_p10 = row["executor_elapsed_s"]
                execution_p90 = row["executor_elapsed_s"]
                planning = float(row["optimization_seconds"])
                total_process_wall = plan_wall + executor_wall
                repeats = "1"
                warmup = "0"
                sampling_policy = "one_deterministic_plan_and_one_exact_execution"
                requested_threads = metadata.get("slurm_cpus_per_task", "1")
                reported_threads = "1"
                peak_support = row["peak_support"]
                examined = row["local_tensor_entries_examined"]
                accepted = row["local_tensor_entries_accepted"]
                metrics_collected = "true"
                work_metric_scope = "full_site_tree_and_row_reference"
                recursive_nodes = ""
                candidate_placements = ""
                cartesian_pair_upper_bound = row["cartesian_pair_upper_bound"]
                matching_entry_pairs = row["matching_entry_pairs"]
                peak_tensor_rank = row["peak_rank"]
            else:
                time_path = point_time_path(csv_path, n)
                executor_wall, executor_rss = read_gnu_time(time_path)
                plan_wall = 0.0
                plan_rss = 0
                execution = float(row["median_elapsed_s"])
                execution_min = optional(row, "min_elapsed_s")
                execution_p10 = optional(row, "p10_elapsed_s")
                execution_p90 = optional(row, "p90_elapsed_s")
                planning = 0.0
                total_process_wall = executor_wall
                repeats = optional(row, "repeats", metadata.get("repeats", ""))
                warmup = optional(row, "warmup", "0")
                sampling_policy = (
                    "repeated_median" if int(repeats or "1") > 1 else "single_exact_sample"
                )
                requested_threads = metadata.get("slurm_cpus_per_task", "1")
                reported_threads = optional(row, "threads", "1")
                if family is DFS:
                    peak_support = "NA"
                    examined = "NA"
                    accepted = "NA"
                    metrics_collected = "true"
                    work_metric_scope = "full_DFS_profile_non_tensor"
                    recursive_nodes = row["recursive_nodes"]
                    candidate_placements = row["candidate_placements"]
                    cartesian_pair_upper_bound = ""
                    matching_entry_pairs = ""
                    peak_tensor_rank = "NA"
                elif family is NAIVE:
                    peak_support = row["peak_states"]
                    examined = row["tensor_entries_examined"]
                    accepted = row["tensor_entries_matched"]
                    metrics_collected = "true"
                    work_metric_scope = "full_explicit_C_row_contraction"
                    recursive_nodes = ""
                    candidate_placements = ""
                    cartesian_pair_upper_bound = ""
                    matching_entry_pairs = ""
                    peak_tensor_rank = "NA"
                else:
                    peak_support = row["peak_sparse_support"]
                    examined = row["local_tensor_entries_examined"]
                    accepted = row["local_tensor_entries_accepted"]
                    metrics_collected = optional(row, "metrics_collected", "false")
                    work_metric_scope = (
                        "full_profile_replay"
                        if metrics_collected.lower() == "true"
                        else "seed_and_C_certificate_only_in_timed_run"
                    )
                    recursive_nodes = optional(row, "recursive_nodes")
                    candidate_placements = ""
                    cartesian_pair_upper_bound = ""
                    matching_entry_pairs = ""
                    peak_tensor_rank = "NA"

            count = row["count"]
            csv_known = optional(row, "known_count")
            audited_known = KNOWN_COUNTS.get(n)
            if audited_known is None:
                raise ValueError(f"{csv_path}: no independent audited count for N={n}")
            if csv_known and csv_known != audited_known:
                raise ValueError(
                    f"{csv_path}: CSV known_count mismatch at N={n}: "
                    f"{csv_known} != {audited_known}"
                )
            known = audited_known
            if count != known:
                raise ValueError(
                    f"{csv_path}: measured count mismatch at N={n}: {count} != {known}"
                )

            normalized.append(
                {
                    "algorithm_key": family.key,
                    "algorithm_label": family.label,
                    "method_role": family.role,
                    "N": str(n),
                    "count": count,
                    "known_count": known,
                    "verified": row["verified"],
                    "execution_time_s": f"{execution:.12g}",
                    "execution_min_time_s": (
                        f"{float(execution_min):.12g}" if execution_min else ""
                    ),
                    "execution_p10_time_s": (
                        f"{float(execution_p10):.12g}" if execution_p10 else ""
                    ),
                    "execution_p90_time_s": (
                        f"{float(execution_p90):.12g}" if execution_p90 else ""
                    ),
                    "planning_time_s": f"{planning:.12g}",
                    "plan_plus_execution_time_s": f"{planning + execution:.12g}",
                    "execution_process_wall_s": f"{executor_wall:.12g}",
                    "planning_process_wall_s": f"{plan_wall:.12g}",
                    "end_to_end_process_wall_s": f"{total_process_wall:.12g}",
                    "execution_peak_rss_bytes": str(executor_rss),
                    "planning_peak_rss_bytes": str(plan_rss),
                    "end_to_end_peak_rss_bytes": str(max(executor_rss, plan_rss)),
                    "peak_sparse_support": peak_support,
                    "local_tensor_entries_examined": examined,
                    "local_tensor_entries_accepted": accepted,
                    "metrics_collected": metrics_collected,
                    "work_metric_scope": work_metric_scope,
                    "recursive_nodes": recursive_nodes,
                    "candidate_placements": candidate_placements,
                    "cartesian_pair_upper_bound": cartesian_pair_upper_bound,
                    "matching_entry_pairs": matching_entry_pairs,
                    "peak_tensor_rank": peak_tensor_rank,
                    "threads": requested_threads,
                    "algorithm_reported_threads": reported_threads,
                    "repeats": repeats,
                    "warmup": warmup,
                    "sampling_policy": sampling_policy,
                    "source_revision": metadata.get("source_revision", ""),
                    "algorithm_revision": metadata.get(
                        "algorithm_revision",
                        metadata.get("implementation_revision", ""),
                    ),
                    "slurm_job_id": metadata.get("slurm_job_id", ""),
                    "slurm_node": metadata.get("slurm_node_list", ""),
                    "slurm_memory_per_node_mb": metadata.get(
                        "slurm_memory_per_node_mb", ""
                    ),
                    "benchmark_command": metadata.get(
                        "command",
                        "; ".join(
                            value
                            for value in (
                                metadata.get("planner_command", ""),
                                metadata.get("executor_command", ""),
                            )
                            if value
                        ),
                    ),
                    "build_command": metadata.get(
                        "build_command", metadata.get("executor_build_command", "")
                    ),
                    "build_compiler": metadata.get(
                        "build_compiler", metadata.get("executor_build_compiler", "")
                    ),
                    "exit_status": metadata.get("exit_status", ""),
                    "raw_csv": csv_path.as_posix(),
                    "memory_method": metadata.get(
                        "memory_method",
                        "per-N GNU time -v Maximum resident set size",
                    ),
                }
            )
    normalized.sort(key=lambda row: int(row["N"]))
    return normalized


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def add_pairwise_ratios(rows: list[dict[str, str]]) -> None:
    by_n: dict[int, dict[str, dict[str, str]]] = {}
    for row in rows:
        by_n.setdefault(int(row["N"]), {})[row["algorithm_key"]] = row
    for row in rows:
        peers = by_n[int(row["N"])]
        dfs = peers.get("dfs")
        latest = peers.get("latest_peps")
        execution = float(row["execution_time_s"])
        inclusive = float(row["plan_plus_execution_time_s"])
        execution_rss = float(row["execution_peak_rss_bytes"])

        row["dfs_time_over_execution_time"] = (
            f"{float(dfs['execution_time_s']) / execution:.12g}" if dfs and execution > 0 else ""
        )
        row["dfs_time_over_plan_plus_execution_time"] = (
            f"{float(dfs['execution_time_s']) / inclusive:.12g}" if dfs and inclusive > 0 else ""
        )
        row["execution_rss_over_dfs_rss"] = (
            f"{execution_rss / float(dfs['execution_peak_rss_bytes']):.12g}"
            if dfs and float(dfs["execution_peak_rss_bytes"]) > 0
            else ""
        )
        row["latest_peps_time_over_execution_time"] = (
            f"{float(latest['execution_time_s']) / execution:.12g}"
            if latest and execution > 0
            else ""
        )


def compact_integer_ranges(values: list[int]) -> str:
    if not values:
        return ""
    ranges: list[tuple[int, int]] = []
    start = previous = values[0]
    for value in values[1:]:
        if value == previous + 1:
            previous = value
            continue
        ranges.append((start, previous))
        start = previous = value
    ranges.append((start, previous))
    return ",".join(
        str(start) if start == end else f"{start}-{end}" for start, end in ranges
    )


def validate_provenance(rows: list[dict[str, str]]) -> dict[str, list[int]]:
    observed: dict[str, set[int]] = {key: set() for key in EXPECTED_N}
    for row in rows:
        key = row["algorithm_key"]
        if key not in EXPECTED_N:
            raise ValueError(f"unexpected algorithm family: {key}")
        n = int(row["N"])
        if n not in EXPECTED_N[key]:
            raise ValueError(f"unexpected {key} N={n}")
        observed[key].add(n)

        expected_source, expected_algorithm = EXPECTED_SOURCE[key]
        if row["source_revision"] != expected_source:
            raise ValueError(
                f"{key} N={n}: source revision {row['source_revision']!r} "
                f"!= {expected_source!r}"
            )
        if row["algorithm_revision"] != expected_algorithm:
            raise ValueError(
                f"{key} N={n}: algorithm revision {row['algorithm_revision']!r} "
                f"!= {expected_algorithm!r}"
            )
        if row["threads"] != EXPECTED_REQUESTED_THREADS[key]:
            raise ValueError(
                f"{key} N={n}: requested threads {row['threads']!r} "
                f"!= {EXPECTED_REQUESTED_THREADS[key]!r}"
            )
        if row["slurm_node"] != EXPECTED_NODE:
            raise ValueError(
                f"{key} N={n}: node {row['slurm_node']!r} != {EXPECTED_NODE!r}"
            )
        if row["exit_status"] != "0":
            raise ValueError(f"{key} N={n}: exit status {row['exit_status']!r}")

    return {
        key: sorted(expected - observed[key]) for key, expected in EXPECTED_N.items()
    }


def missing_summary(missing: dict[str, list[int]]) -> str:
    display = {
        "dfs": "DFS",
        "naive_peps": "naive PEPS",
        "latest_peps": "latest PEPS",
        "treesa_peps": "TreeSA PEPS",
    }
    return "; ".join(
        f"{display[key]} N={compact_integer_ranges(values)}"
        for key, values in missing.items()
        if values
    )


def text_element(x: float, y: float, value: str, **attrs: object) -> str:
    rendered = " ".join(
        f'{key.replace("_", "-")}="{html.escape(str(item))}"'
        for key, item in attrs.items()
    )
    return f'<text x="{x:.2f}" y="{y:.2f}" {rendered}>{html.escape(value)}</text>'


def line_plot(
    output: Path,
    rows: list[dict[str, str]],
    *,
    kind: str,
    provisional_note: str,
) -> None:
    if kind == "time":
        title = "Exact N-Queens algorithms on one SCNet node: execution-time scaling"
        subtitle = (
            "AMD EPYC 7742 node b10r4n19; internal wall clock excludes process startup; "
            "TreeSA inclusive curve adds optimizer time"
        )
        y_label = "Execution / planning time (seconds, log scale)"
        series = [
            ("dfs", "DFS bitmask comparator", "execution_time_s", "#52606b", ""),
            ("naive_peps", "Naive exact PEPS (20b5334)", "execution_time_s", "#c24b35", ""),
            ("latest_peps", "Latest exact PEPS, no TreeSA", "execution_time_s", "#0b6e99", ""),
            ("treesa_peps", "TreeSA PEPS: executor only", "execution_time_s", "#7654a8", ""),
            ("treesa_peps", "TreeSA PEPS: plan + executor", "plan_plus_execution_time_s", "#b67b18", "6 4"),
        ]
    elif kind == "rss":
        title = "Exact N-Queens algorithms on one SCNet node: peak-RSS scaling"
        subtitle = (
            "AMD EPYC 7742 node b10r4n19; release builds; each N measured in a fresh process"
        )
        y_label = "Peak RSS (MiB, log scale)"
        series = [
            ("dfs", "DFS bitmask comparator", "execution_peak_rss_bytes", "#52606b", ""),
            ("naive_peps", "Naive exact PEPS (20b5334)", "execution_peak_rss_bytes", "#c24b35", ""),
            ("latest_peps", "Latest exact PEPS, no TreeSA", "execution_peak_rss_bytes", "#0b6e99", ""),
            ("treesa_peps", "TreeSA PEPS: executor", "execution_peak_rss_bytes", "#7654a8", ""),
            ("treesa_peps", "TreeSA PEPS: end-to-end max", "end_to_end_peak_rss_bytes", "#b67b18", "6 4"),
        ]
    else:
        raise ValueError(kind)

    if provisional_note:
        title = f"PROVISIONAL — {title}"
        subtitle = (
            f"Incomplete: {provisional_note}. Regenerate with --require-complete before use."
        )

    width, height = 1200, 760
    left, right, top, bottom = 105, 35, 138, 88
    plot_width = width - left - right
    plot_height = height - top - bottom
    n_min, n_max = 1, 22

    plotted: list[tuple[str, str, str, str, list[tuple[int, float]]]] = []
    all_values: list[float] = []
    for key, label, field, color, dash in series:
        points: list[tuple[int, float]] = []
        for row in rows:
            if row["algorithm_key"] != key:
                continue
            value = float(row[field])
            if kind == "rss":
                value /= 1024.0 * 1024.0
            if value > 0 and math.isfinite(value):
                points.append((int(row["N"]), value))
                all_values.append(value)
        if points:
            plotted.append((label, color, dash, field, sorted(points)))
    if not all_values:
        raise ValueError("no positive values to plot")

    log_min = math.floor(math.log10(min(all_values)))
    log_max = math.ceil(math.log10(max(all_values)))
    if log_min == log_max:
        log_max += 1

    def x_position(n: int) -> float:
        return left + (n - n_min) * plot_width / (n_max - n_min)

    def y_position(value: float) -> float:
        return top + (log_max - math.log10(value)) * plot_height / (log_max - log_min)

    elements = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="#fbfaf7"/>',
        text_element(left, 38, title, fill="#17212b", font_size=23, font_weight=700),
        text_element(
            left,
            67,
            subtitle,
            fill="#52606b",
            font_size=13,
        ),
        f'<rect x="{left}" y="{top}" width="{plot_width}" height="{plot_height}" fill="#ffffff" stroke="#aeb8c0"/>',
    ]

    for exponent in range(log_min, log_max + 1):
        value = 10.0**exponent
        y = y_position(value)
        elements.append(
            f'<line x1="{left}" x2="{width-right}" y1="{y:.2f}" y2="{y:.2f}" stroke="#e1e5e8"/>'
        )
        elements.append(
            text_element(
                left - 12,
                y + 5,
                f"1e{exponent}",
                fill="#52606b",
                font_size=12,
                text_anchor="end",
            )
        )
    for n in range(n_min, n_max + 1):
        x = x_position(n)
        if n % 2 == 0 or n in (1, 22):
            elements.append(
                text_element(x, top + plot_height + 26, str(n), fill="#34424e", font_size=12, text_anchor="middle")
            )

    for label, color, dash, _field, points in plotted:
        path_data = " ".join(
            ("M" if index == 0 else "L") + f" {x_position(n):.2f} {y_position(value):.2f}"
            for index, (n, value) in enumerate(points)
        )
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        elements.append(
            f'<path d="{path_data}" fill="none" stroke="{color}" stroke-width="2.4"{dash_attr}/>'
        )
        for n, value in points:
            elements.append(
                f'<circle cx="{x_position(n):.2f}" cy="{y_position(value):.2f}" r="3.6" fill="{color}"/>'
            )

    legend_x, legend_y = left, 92
    for label, color, dash, _field, _points in plotted:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        elements.append(
            f'<line x1="{legend_x}" x2="{legend_x+28}" y1="{legend_y}" y2="{legend_y}" stroke="{color}" stroke-width="2.5"{dash_attr}/>'
        )
        elements.append(text_element(legend_x + 35, legend_y + 4, label, fill="#34424e", font_size=12))
        legend_x += 230 if "TreeSA" not in label else 242
        if legend_x > width - 250:
            legend_x = left
            legend_y += 20

    elements.append(
        text_element(
            left + plot_width / 2,
            height - 28,
            "Board size N",
            fill="#17212b",
            font_size=15,
            text_anchor="middle",
        )
    )
    elements.append(
        f'<text x="25" y="{top + plot_height/2:.2f}" fill="#17212b" font-size="15" text-anchor="middle" transform="rotate(-90 25 {top + plot_height/2:.2f})">{html.escape(y_label)}</text>'
    )
    elements.append("</svg>")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(elements) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dfs", nargs="+", required=True, type=Path)
    parser.add_argument("--naive", nargs="+", required=True, type=Path)
    parser.add_argument("--peps", nargs="+", required=True, type=Path)
    parser.add_argument("--treesa", nargs="+", required=True, type=Path)
    parser.add_argument("--output-csv", required=True, type=Path)
    parser.add_argument("--output-time-svg", required=True, type=Path)
    parser.add_argument("--output-rss-svg", required=True, type=Path)
    parser.add_argument(
        "--require-complete",
        action="store_true",
        help="refuse output unless every preregistered family/N point is present",
    )
    args = parser.parse_args()

    rows: list[dict[str, str]] = []
    rows.extend(load_family(args.dfs, DFS))
    rows.extend(load_family(args.naive, NAIVE))
    rows.extend(load_family(args.peps, PEPS))
    rows.extend(load_family(args.treesa, TREESA))
    rows.sort(key=lambda row: (int(row["N"]), row["algorithm_key"]))
    add_pairwise_ratios(rows)
    missing = validate_provenance(rows)
    provisional_note = missing_summary(missing)
    if args.require_complete and provisional_note:
        raise ValueError(f"incomplete preregistered comparison: {provisional_note}")
    write_csv(args.output_csv, rows)
    line_plot(
        args.output_time_svg,
        rows,
        kind="time",
        provisional_note=provisional_note,
    )
    line_plot(
        args.output_rss_svg,
        rows,
        kind="rss",
        provisional_note=provisional_note,
    )


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Normalize APX1 CSV files and render the chi/error/time tradeoff as SVG.

The directory and input filenames retain their legacy ``e51`` provenance ID.
"""

from __future__ import annotations

import argparse
import csv
import html
import math
from collections import defaultdict
from pathlib import Path


KNOWN_COUNTS = {
    5: 10,
    6: 4,
    7: 40,
    8: 92,
    9: 352,
    10: 724,
    11: 2680,
    12: 14200,
    13: 73712,
    14: 365596,
    15: 2279184,
    16: 14772512,
    17: 95815104,
    18: 666090624,
    19: 4968057848,
    20: 39029188884,
}


def finite_float(value: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError(f"non-finite numeric field: {value!r}")
    return parsed


def load_rows(paths: list[Path]) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    seen: set[tuple[int, int]] = set()
    for path in paths:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            required = {
                "algorithm_class",
                "N",
                "chi",
                "estimate",
                "exact_count",
                "absolute_error",
                "relative_error",
                "median_elapsed_s",
                "peak_rss_bytes",
                "peak_dense_mps_elements",
                "peak_retained_bond",
                "peak_pretruncate_rank",
                "truncated_svd_calls",
                "max_discarded_fraction",
                "sum_discarded_fraction",
                "tensor_entries_examined",
                "tensor_entries_accepted",
                "truncation_occurred",
            }
            missing = required.difference(reader.fieldnames or ())
            if missing:
                raise ValueError(f"{path}: missing columns {sorted(missing)}")
            for row in reader:
                if row["algorithm_class"] != "truncated_boundary_mps_float64":
                    raise ValueError(f"{path}: mixed algorithm class")
                n = int(row["N"])
                chi = int(row["chi"])
                key = (n, chi)
                if key in seen:
                    raise ValueError(f"duplicate N/chi point: {key}")
                seen.add(key)

                if n not in KNOWN_COUNTS:
                    raise ValueError(f"{path}: no audited exact count for N={n}")
                exact_count = int(row["exact_count"])
                if exact_count != KNOWN_COUNTS[n]:
                    raise ValueError(
                        f"{path}: exact reference mismatch at N={n}: "
                        f"{exact_count} != {KNOWN_COUNTS[n]}"
                    )
                if chi < 0:
                    raise ValueError(f"{path}: negative chi at N={n}")

                estimate = finite_float(row["estimate"])
                absolute_error = finite_float(row["absolute_error"])
                relative_error = finite_float(row["relative_error"])
                elapsed = finite_float(row["median_elapsed_s"])
                if elapsed <= 0:
                    raise ValueError(f"{path}: non-positive time at N={n}, chi={chi}")
                derived_absolute = abs(estimate - exact_count)
                derived_relative = derived_absolute / exact_count
                if not math.isclose(absolute_error, derived_absolute, rel_tol=1e-12, abs_tol=1e-9):
                    raise ValueError(f"{path}: absolute error mismatch at N={n}, chi={chi}")
                if not math.isclose(relative_error, derived_relative, rel_tol=1e-12, abs_tol=1e-15):
                    raise ValueError(f"{path}: relative error mismatch at N={n}, chi={chi}")

                if int(row["peak_rss_bytes"]) <= 0:
                    raise ValueError(f"{path}: non-positive RSS at N={n}, chi={chi}")
                examined = int(row["tensor_entries_examined"])
                accepted = int(row["tensor_entries_accepted"])
                if not 0 <= accepted <= examined:
                    raise ValueError(f"{path}: invalid tensor work counters at N={n}, chi={chi}")
                if row["truncation_occurred"] not in {"true", "false"}:
                    raise ValueError(f"{path}: invalid truncation flag at N={n}, chi={chi}")
                expected_status = (
                    "approximate_diagnostic"
                    if row["truncation_occurred"] == "true"
                    else "floating_uncapped_check_pass"
                )
                if row["status"] != expected_status:
                    raise ValueError(
                        f"{path}: status/truncation mismatch at N={n}, chi={chi}"
                    )
                rows.append(row)
    rows.sort(key=lambda row: (int(row["N"]), int(row["chi"]) == 0, int(row["chi"])))
    return rows


def normalized_rows(rows: list[dict[str, str]]) -> list[dict[str, str]]:
    groups: dict[int, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        groups[int(row["N"])].append(row)

    normalized: list[dict[str, str]] = []
    for n, group in sorted(groups.items()):
        uncapped = next((row for row in group if int(row["chi"]) == 0), None)
        capped = [row for row in group if int(row["chi"]) > 0]
        largest = max(capped, key=lambda row: int(row["chi"])) if capped else None
        uncapped_time = finite_float(uncapped["median_elapsed_s"]) if uncapped else math.nan
        largest_time = finite_float(largest["median_elapsed_s"]) if largest else math.nan
        for row in group:
            elapsed = finite_float(row["median_elapsed_s"])
            relative_error = finite_float(row["relative_error"])
            absolute_error = finite_float(row["absolute_error"])
            if math.isfinite(relative_error):
                error_metric = relative_error
                error_kind = "relative_error"
            else:
                error_metric = absolute_error
                error_kind = "absolute_error_for_zero_exact_count"
            normalized.append(
                {
                    "N": str(n),
                    "chi": row["chi"],
                    "status": row["status"],
                    "estimate": row["estimate"],
                    "exact_count": row["exact_count"],
                    "absolute_error": row["absolute_error"],
                    "relative_error": row["relative_error"],
                    "error_metric": f"{error_metric:.17g}",
                    "error_metric_kind": error_kind,
                    "median_elapsed_s": row["median_elapsed_s"],
                    "speedup_vs_uncapped": (
                        f"{uncapped_time / elapsed:.17g}" if math.isfinite(uncapped_time) else ""
                    ),
                    "speedup_vs_largest_capped_chi": (
                        f"{largest_time / elapsed:.17g}" if math.isfinite(largest_time) else ""
                    ),
                    "peak_rss_bytes": row["peak_rss_bytes"],
                    "peak_dense_mps_elements": row["peak_dense_mps_elements"],
                    "peak_retained_bond": row["peak_retained_bond"],
                    "peak_working_bond": row.get("peak_working_bond", row["peak_retained_bond"]),
                    "peak_pretruncate_rank": row["peak_pretruncate_rank"],
                    "svd_qr_fallbacks": row.get("svd_qr_fallbacks", "0"),
                    "truncated_svd_calls": row["truncated_svd_calls"],
                    "max_discarded_fraction": row["max_discarded_fraction"],
                    "sum_discarded_fraction": row["sum_discarded_fraction"],
                }
            )
    return normalized


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def svg_text(x: float, y: float, text: str, **attributes: object) -> str:
    attrs = " ".join(f'{key.replace("_", "-")}="{value}"' for key, value in attributes.items())
    return f'<text x="{x:.2f}" y="{y:.2f}" {attrs}>{html.escape(text)}</text>'


def render_svg(
    path: Path,
    rows: list[dict[str, str]],
    selected_ns: set[int] | None = None,
) -> None:
    if selected_ns is not None:
        rows = [row for row in rows if int(row["N"]) in selected_ns]
    if not rows:
        raise ValueError("no rows selected for chi tradeoff plot")

    width, height = 1120, 800
    left, right = 92, 28
    top = 102
    panel_height = 270
    gap = 92
    plot_width = width - left - right
    colors = [
        "#0b6e99",
        "#c24b35",
        "#5c8d3b",
        "#7654a8",
        "#b67b18",
        "#008a7a",
        "#9c3f76",
        "#52606b",
    ]
    groups: dict[int, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        groups[int(row["N"])].append(row)
    chi_values = sorted({int(row["chi"]) for row in rows if int(row["chi"]) > 0})
    if any(int(row["chi"]) == 0 for row in rows):
        chi_values.append(0)
    x_index = {chi: index for index, chi in enumerate(chi_values)}

    def x_position(chi: int) -> float:
        if len(chi_values) == 1:
            return left + plot_width / 2
        return left + x_index[chi] * plot_width / (len(chi_values) - 1)

    def panel_values(field: str) -> list[float]:
        return [max(float(row[field]), 1e-16) for row in rows if math.isfinite(float(row[field]))]

    panels = [
        ("error_metric", "Approximation error", top),
        ("median_elapsed_s", "Median contraction time (s)", top + panel_height + gap),
    ]
    elements = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="#fbfaf7"/>',
        svg_text(left, 34, "APX1 boundary-MPS truncation: accuracy and cost versus bond cap", fill="#17212b", font_size="22", font_weight="700"),
    ]

    for field, title, panel_top in panels:
        values = panel_values(field)
        log_min = math.floor(math.log10(min(values)))
        log_max = math.ceil(math.log10(max(values)))
        if log_min == log_max:
            log_max += 1

        def y_position(value: float) -> float:
            log_value = math.log10(max(value, 1e-16))
            return panel_top + panel_height * (log_max - log_value) / (log_max - log_min)

        elements.append(svg_text(left, panel_top - 16, title, fill="#17212b", font_size="17", font_weight="700"))
        elements.append(
            f'<rect x="{left}" y="{panel_top}" width="{plot_width}" height="{panel_height}" fill="#ffffff" stroke="#bac2c9"/>'
        )
        for exponent in range(log_min, log_max + 1):
            y = y_position(10.0**exponent)
            elements.append(f'<line x1="{left}" x2="{width-right}" y1="{y:.2f}" y2="{y:.2f}" stroke="#e4e7e9"/>')
            elements.append(svg_text(left - 10, y + 5, f"1e{exponent}", fill="#52606b", font_size="12", text_anchor="end"))
        for n_index, (n, group) in enumerate(sorted(groups.items())):
            color = colors[n_index % len(colors)]
            ordered = sorted(group, key=lambda row: x_index[int(row["chi"])])
            points = [
                (x_position(int(row["chi"])), y_position(float(row[field])))
                for row in ordered
                if math.isfinite(float(row[field]))
            ]
            if points:
                path_data = " ".join(
                    ("M" if index == 0 else "L") + f" {x:.2f} {y:.2f}"
                    for index, (x, y) in enumerate(points)
                )
                elements.append(f'<path d="{path_data}" fill="none" stroke="{color}" stroke-width="2"/>')
                for x, y in points:
                    elements.append(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="4" fill="{color}"/>')

    x_axis_y = top + 2 * panel_height + gap
    for chi in chi_values:
        label = "no cap" if chi == 0 else str(chi)
        x = x_position(chi)
        elements.append(svg_text(x, x_axis_y + 28, label, fill="#34424e", font_size="13", text_anchor="middle"))
    elements.append(svg_text(left + plot_width / 2, x_axis_y + 56, "Maximum MPS bond dimension chi", fill="#17212b", font_size="15", text_anchor="middle"))

    legend_x = left
    legend_y = 70
    for n_index, n in enumerate(sorted(groups)):
        color = colors[n_index % len(colors)]
        elements.append(f'<circle cx="{legend_x:.2f}" cy="{legend_y:.2f}" r="4" fill="{color}"/>')
        elements.append(svg_text(legend_x + 10, legend_y + 4, f"N={n}", fill="#34424e", font_size="12"))
        legend_x += 72
        if legend_x > width - right - 55:
            legend_x = left
            legend_y += 20
    elements.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(elements) + "\n", encoding="utf-8")


def render_scaling_svg(path: Path, rows: list[dict[str, str]]) -> None:
    rows = [row for row in rows if int(row["chi"]) > 0]
    if not rows:
        raise ValueError("no capped rows selected for scaling plot")

    width, height = 1120, 800
    left, right = 92, 28
    top = 102
    panel_height = 270
    gap = 92
    plot_width = width - left - right
    colors = ["#0b6e99", "#c24b35", "#5c8d3b", "#7654a8", "#b67b18", "#008a7a"]
    groups: dict[int, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        groups[int(row["chi"])].append(row)
    n_values = sorted({int(row["N"]) for row in rows})

    def x_position(n: int) -> float:
        if len(n_values) == 1:
            return left + plot_width / 2
        return left + (n - n_values[0]) * plot_width / (n_values[-1] - n_values[0])

    panels = [
        ("error_metric", "Approximation error", top),
        ("median_elapsed_s", "Median contraction time (s)", top + panel_height + gap),
    ]
    elements = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="#fbfaf7"/>',
        svg_text(left, 34, "APX1 fixed-bond boundary-MPS scaling on SCNet", fill="#17212b", font_size="22", font_weight="700"),
    ]

    for field, title, panel_top in panels:
        values = [
            max(float(row[field]), 1e-16)
            for row in rows
            if math.isfinite(float(row[field]))
        ]
        log_min = math.floor(math.log10(min(values)))
        log_max = math.ceil(math.log10(max(values)))
        if log_min == log_max:
            log_max += 1

        def y_position(value: float) -> float:
            log_value = math.log10(max(value, 1e-16))
            return panel_top + panel_height * (log_max - log_value) / (log_max - log_min)

        elements.append(svg_text(left, panel_top - 16, title, fill="#17212b", font_size="17", font_weight="700"))
        elements.append(
            f'<rect x="{left}" y="{panel_top}" width="{plot_width}" height="{panel_height}" fill="#ffffff" stroke="#bac2c9"/>'
        )
        for exponent in range(log_min, log_max + 1):
            y = y_position(10.0**exponent)
            elements.append(f'<line x1="{left}" x2="{width-right}" y1="{y:.2f}" y2="{y:.2f}" stroke="#e4e7e9"/>')
            elements.append(svg_text(left - 10, y + 5, f"1e{exponent}", fill="#52606b", font_size="12", text_anchor="end"))
        for chi_index, (chi, group) in enumerate(sorted(groups.items())):
            color = colors[chi_index % len(colors)]
            ordered = sorted(group, key=lambda row: int(row["N"]))
            points = [
                (x_position(int(row["N"])), y_position(float(row[field])))
                for row in ordered
                if math.isfinite(float(row[field]))
            ]
            if points:
                path_data = " ".join(
                    ("M" if index == 0 else "L") + f" {x:.2f} {y:.2f}"
                    for index, (x, y) in enumerate(points)
                )
                elements.append(f'<path d="{path_data}" fill="none" stroke="{color}" stroke-width="2"/>')
                for x, y in points:
                    elements.append(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="4" fill="{color}"/>')

    x_axis_y = top + 2 * panel_height + gap
    for n in n_values:
        x = x_position(n)
        elements.append(svg_text(x, x_axis_y + 28, str(n), fill="#34424e", font_size="13", text_anchor="middle"))
    elements.append(svg_text(left + plot_width / 2, x_axis_y + 56, "Board size N", fill="#17212b", font_size="15", text_anchor="middle"))

    legend_x = left
    legend_y = 70
    for chi_index, chi in enumerate(sorted(groups)):
        color = colors[chi_index % len(colors)]
        elements.append(f'<circle cx="{legend_x:.2f}" cy="{legend_y:.2f}" r="4" fill="{color}"/>')
        elements.append(svg_text(legend_x + 10, legend_y + 4, f"chi={chi}", fill="#34424e", font_size="12"))
        legend_x += 88
    elements.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(elements) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="+", type=Path)
    parser.add_argument("--output-raw-csv", required=True, type=Path)
    parser.add_argument("--output-csv", required=True, type=Path)
    parser.add_argument("--output-svg", required=True, type=Path)
    parser.add_argument("--output-scaling-svg", type=Path)
    parser.add_argument(
        "--plot-n",
        help="comma-separated N values for the chi plot; the CSV still retains every row",
    )
    args = parser.parse_args()
    raw_rows = load_rows(args.inputs)
    if not raw_rows:
        raise SystemExit("no APX1 rows found")
    write_csv(args.output_raw_csv, raw_rows)
    rows = normalized_rows(raw_rows)
    write_csv(args.output_csv, rows)
    selected_ns = None
    if args.plot_n:
        selected_ns = {int(value) for value in args.plot_n.split(",")}
    render_svg(args.output_svg, rows, selected_ns)
    if args.output_scaling_svg:
        render_scaling_svg(args.output_scaling_svg, rows)


if __name__ == "__main__":
    main()

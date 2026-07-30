#!/usr/bin/env python3
"""Generate the Chinese Issue #34 submission figures from audited CSV data.

This script intentionally keeps three evidence families separate:

* the serialized same-node comparison (currently complete through N=21 for
  DFS/latest PEPS, N=16 for naive PEPS, and N=11 for TreeSA);
* the production E50 scaling series through the independently profiled N=22;
* the APX1 floating-point boundary-MPS truncation diagnostic.

It refuses silent provenance changes and writes a machine-readable projection
table alongside the SVG figures.  Projections are sensitivity analyses, not
exact counts or complexity theorems.
"""

from __future__ import annotations

import csv
import math
from collections import defaultdict
from pathlib import Path

import matplotlib as mpl
import matplotlib.pyplot as plt
from matplotlib import font_manager
import numpy as np


ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "benchmarks"
OUT = ROOT / "docs" / "assets" / "issue34_submission"

KNOWN = {
    1: 1,
    2: 0,
    3: 0,
    4: 2,
    5: 10,
    6: 4,
    7: 40,
    8: 92,
    9: 352,
    10: 724,
    11: 2_680,
    12: 14_200,
    13: 73_712,
    14: 365_596,
    15: 2_279_184,
    16: 14_772_512,
    17: 95_815_104,
    18: 666_090_624,
    19: 4_968_057_848,
    20: 39_029_188_884,
    21: 314_666_222_712,
    22: 2_691_008_701_644,
}

EXPECTED_COMPARISON = {
    "dfs": set(range(1, 23)),
    "naive_peps": set(range(1, 17)),
    "latest_peps": set(range(1, 23)),
    "treesa_peps": set(range(2, 12)),
}

COLORS = {
    "dfs": "#177ddc",
    "naive_peps": "#dd5a4f",
    "latest_peps": "#087f5b",
    "treesa_peps": "#7b4ab5",
    "treesa_total": "#b071d1",
    "projection_a": "#087f5b",
    "projection_b": "#d97706",
    "projection_c": "#7b4ab5",
}


def read_csv(path: Path) -> list[dict[str, str]]:
    if not path.is_file():
        raise FileNotFoundError(path)
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def configure_plotting() -> None:
    cjk_font = Path("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc")
    if not cjk_font.is_file():
        raise FileNotFoundError(f"required CJK font is missing: {cjk_font}")
    font_manager.fontManager.addfont(cjk_font)
    cjk_family = font_manager.FontProperties(fname=cjk_font).get_name()
    mpl.rcParams.update(
        {
            "font.family": "sans-serif",
            "font.sans-serif": [cjk_family, "DejaVu Sans"],
            "axes.unicode_minus": False,
            # Embed glyph outlines so the Chinese figures render identically on
            # machines that do not have the build host's CJK fonts installed.
            "svg.fonttype": "path",
            "figure.facecolor": "white",
            "axes.facecolor": "#fbfdff",
            "axes.edgecolor": "#b8c7d9",
            "axes.labelcolor": "#18324d",
            "xtick.color": "#415a73",
            "ytick.color": "#415a73",
            "grid.color": "#dbe5ef",
            "grid.linewidth": 0.8,
            "legend.frameon": False,
        }
    )


def save_svg(fig: plt.Figure, path: Path) -> None:
    """Save deterministic SVG text without Matplotlib's path-line whitespace."""
    fig.savefig(path, bbox_inches="tight")
    svg = path.read_text(encoding="utf-8")
    path.write_text(
        "\n".join(line.rstrip() for line in svg.splitlines()) + "\n",
        encoding="utf-8",
    )


def validate_count(row: dict[str, str], *, field: str = "count") -> None:
    n = int(row["N"])
    if n not in KNOWN:
        raise ValueError(f"no audited count for N={n}")
    if int(row[field]) != KNOWN[n]:
        raise ValueError(f"count mismatch at N={n}: {row[field]} != {KNOWN[n]}")
    if row.get("verified", "true").lower() != "true":
        raise ValueError(f"unverified row at N={n}")


def load_comparison() -> tuple[dict[str, list[dict[str, str]]], dict[str, list[int]]]:
    path = BENCH / "issue34_same_node_algorithm_comparison_scnet.csv"
    rows = read_csv(path)
    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    seen: set[tuple[str, int]] = set()
    for row in rows:
        validate_count(row)
        if row["slurm_node"] != "b10r4n19" or row["exit_status"] != "0":
            raise ValueError(f"invalid same-node provenance: {row['algorithm_key']} N={row['N']}")
        key = (row["algorithm_key"], int(row["N"]))
        if key in seen:
            raise ValueError(f"duplicate comparison point: {key}")
        seen.add(key)
        grouped[key[0]].append(row)

    missing: dict[str, list[int]] = {}
    for family, expected in EXPECTED_COMPARISON.items():
        actual = {int(row["N"]) for row in grouped[family]}
        gap = sorted(expected - actual)
        if gap:
            missing[family] = gap
    allowed_gap = {"dfs": [22], "latest_peps": [22]}
    if missing and missing != allowed_gap:
        raise ValueError(f"unexpected comparison gaps: {missing}")
    for values in grouped.values():
        values.sort(key=lambda row: int(row["N"]))
    return grouped, missing


def plot_comparison(grouped: dict[str, list[dict[str, str]]], missing: dict[str, list[int]]) -> None:
    fig, axes = plt.subplots(1, 2, figsize=(13.5, 5.3), constrained_layout=True)
    labels = {
        "dfs": "DFS 位掩码（独立对照）",
        "naive_peps": "朴素显式-C PEPS",
        "latest_peps": "最新精确 PEPS（无 TreeSA）",
        "treesa_peps": "TreeSA 路径的精确执行",
    }
    markers = {"dfs": "o", "naive_peps": "s", "latest_peps": "D", "treesa_peps": "^"}

    for family in ("dfs", "naive_peps", "latest_peps", "treesa_peps"):
        rows = [row for row in grouped[family] if int(row["N"]) >= 4]
        ns = [int(row["N"]) for row in rows]
        times = [float(row["execution_time_s"]) for row in rows]
        axes[0].plot(
            ns,
            times,
            marker=markers[family],
            markersize=5,
            linewidth=2,
            color=COLORS[family],
            label=labels[family],
        )
        rss = [float(row["execution_peak_rss_bytes"]) / 2**20 for row in rows]
        axes[1].plot(
            ns,
            rss,
            marker=markers[family],
            markersize=5,
            linewidth=2,
            color=COLORS[family],
            label=labels[family],
        )

    tree_rows = [row for row in grouped["treesa_peps"] if int(row["N"]) >= 4]
    tree_n = [int(row["N"]) for row in tree_rows]
    axes[0].plot(
        tree_n,
        [float(row["plan_plus_execution_time_s"]) for row in tree_rows],
        linestyle="--",
        marker="v",
        linewidth=1.8,
        color=COLORS["treesa_total"],
        label="TreeSA 搜索 + 精确执行",
    )
    axes[1].plot(
        tree_n,
        [float(row["end_to_end_peak_rss_bytes"]) / 2**20 for row in tree_rows],
        linestyle="--",
        marker="v",
        linewidth=1.8,
        color=COLORS["treesa_total"],
        label="TreeSA 端到端峰值",
    )

    for ax, ylabel, title in (
        (axes[0], "执行时间 / 秒", "耗时随 N 的增长"),
        (axes[1], "峰值 RSS / MiB", "内存随 N 的增长"),
    ):
        ax.set_yscale("log")
        ax.set_xlabel("棋盘规模 N")
        ax.set_ylabel(ylabel)
        ax.set_title(title, fontsize=14, fontweight="bold", color="#102a43")
        ax.grid(True, which="both", alpha=0.75)
        ax.set_xticks(range(4, 23, 2))
        ax.legend(fontsize=8.5, loc="best")

    missing_text = "；".join(f"{key}: N={','.join(map(str, values))}" for key, values in missing.items())
    fig.suptitle(
        "同一 SCNet 节点上的四类算法对照（精确结果）",
        fontsize=17,
        fontweight="bold",
        color="#102a43",
    )
    fig.text(
        0.5,
        0.005,
        f"节点 b10r4n19；当前待补点：{missing_text}。不同实现的线程并行度不同，图示为整机实现吞吐而非算法同构对照。",
        ha="center",
        fontsize=8.5,
        color="#526b82",
    )
    save_svg(fig, OUT / "algorithm_time_rss_zh.svg")
    plt.close(fig)


def load_e50_scaling() -> tuple[dict[int, float], float]:
    rows = read_csv(BENCH / "scnet_e50_scaling_n1_n19_release.csv")
    rows += read_csv(BENCH / "scnet_e50_calibration_release.csv")
    n22_rows = read_csv(
        BENCH / "raw" / "scnet_e50_n22" / "e50_scalar3_profile_once_scnet_n22_job41497149.csv"
    )
    if len(n22_rows) != 1:
        raise ValueError("expected one N=22 profile row")
    n22 = n22_rows[0]
    validate_count(n22)

    measured: dict[int, float] = {}
    for row in rows:
        n = int(row["N"])
        validate_count(row)
        if row["mode"] != "scalar" or row["lanes"] != "3":
            raise ValueError(f"mixed E50 scaling backend at N={n}")
        if n in measured:
            raise ValueError(f"duplicate E50 scaling row at N={n}")
        measured[n] = float(row["median_elapsed_s"])
    measured[22] = float(n22["count_elapsed_s"])
    if set(range(1, 23)) - measured.keys():
        raise ValueError("E50 scaling series is incomplete through N=22")

    lane_rows = read_csv(BENCH / "scnet_e50_crt_lane_pair_release.csv")
    by_n: dict[int, dict[int, float]] = defaultdict(dict)
    for row in lane_rows:
        by_n[int(row["N"])][int(row["lanes"])] = float(row["median_elapsed_s"])
    ratios = [values[4] / values[3] for values in by_n.values() if 3 in values and 4 in values]
    if len(ratios) != 2:
        raise ValueError("expected paired 3/4-lane measurements at N=18,19")
    lane_overhead = math.prod(ratios) ** (1 / len(ratios))
    return measured, lane_overhead


def projection_models(measured: dict[int, float], lane_overhead: float) -> dict[str, dict[int, float]]:
    fit_n = np.array(list(range(18, 23)), dtype=float)
    fit_t = np.array([measured[int(n)] for n in fit_n], dtype=float)
    log_t = np.log(fit_t)
    slope_n = float(np.polyfit(fit_n, log_t, 1)[0])
    slope_nlogn = float(np.polyfit(fit_n * np.log(fit_n), log_t, 1)[0])
    latest_ratio = measured[22] / measured[21]

    predictions: dict[str, dict[int, float]] = {
        "logT=a+bN": {22: measured[22]},
        "logT=a+bNlogN": {22: measured[22]},
        "hold_latest_ratio": {22: measured[22]},
    }
    for n in range(23, 29):
        overhead = lane_overhead if n == 28 else 1.0
        predictions["logT=a+bN"][n] = (
            measured[22] * math.exp(slope_n * (n - 22)) * overhead
        )
        predictions["logT=a+bNlogN"][n] = (
            measured[22]
            * math.exp(slope_nlogn * (n * math.log(n) - 22 * math.log(22)))
            * overhead
        )
        predictions["hold_latest_ratio"][n] = measured[22] * latest_ratio ** (n - 22) * overhead
    return predictions


def write_projection_csv(
    measured: dict[int, float], predictions: dict[str, dict[int, float]], lane_overhead: float
) -> None:
    path = BENCH / "issue34_submission_q28_projection.csv"
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "record_type",
                "model",
                "N",
                "seconds",
                "years",
                "status",
                "fit_window",
                "anchor_N",
                "lane_overhead_at_N28",
            ]
        )
        for n in range(16, 23):
            seconds = measured[n]
            writer.writerow(
                ["measurement", "E50_scalar3_CRT_last6", n, f"{seconds:.12g}", "", "exact_verified", "", "", ""]
            )
        for model, values in predictions.items():
            for n in range(23, 29):
                seconds = values[n]
                writer.writerow(
                    [
                        "projection",
                        model,
                        n,
                        f"{seconds:.12g}",
                        f"{seconds / (365.25 * 86400):.12g}",
                        "sensitivity_analysis_not_exact_count",
                        "18-22",
                        22,
                        f"{lane_overhead:.12g}" if n == 28 else "1",
                    ]
                )


def plot_projection(measured: dict[int, float], predictions: dict[str, dict[int, float]]) -> None:
    fig, ax = plt.subplots(figsize=(10.5, 5.5), constrained_layout=True)
    measured_n = list(range(16, 23))
    measured_t = [measured[n] for n in measured_n]
    ax.plot(
        measured_n,
        measured_t,
        color="#102a43",
        marker="o",
        markersize=6,
        linewidth=2.4,
        label="E50 精确测量（128 核）",
        zorder=5,
    )
    model_style = {
        "logT=a+bN": ("log T = a + bN", COLORS["projection_a"], "--"),
        "logT=a+bNlogN": ("log T = a + bN log N", COLORS["projection_b"], "-."),
        "hold_latest_ratio": ("保持 N=21→22 比率", COLORS["projection_c"], ":"),
    }
    label_offsets = {
        "logT=a+bN": (-42, -14),
        "logT=a+bNlogN": (-42, 12),
        "hold_latest_ratio": (-42, -1),
    }
    for model, values in predictions.items():
        label, color, linestyle = model_style[model]
        ns = list(range(22, 29))
        ts = [values[n] for n in ns]
        ax.plot(ns, ts, color=color, linestyle=linestyle, linewidth=2.1, label=label)
        years = values[28] / (365.25 * 86400)
        offset = label_offsets[model]
        ax.annotate(
            f"{years:.0f} 年",
            (28, values[28]),
            xytext=offset,
            textcoords="offset points",
            ha="right",
            va="bottom",
            fontsize=9,
            color=color,
            fontweight="bold",
        )

    ax.axvline(22.5, color="#94a9bd", linewidth=1, alpha=0.8)
    ax.text(22.62, 0.16, "虚线区域均为外推", transform=ax.get_xaxis_transform(), color="#6b7f93", fontsize=9)
    ax.set_yscale("log")
    ax.set_ylim(min(measured_t) * 0.45, max(values[28] for values in predictions.values()) * 2.2)
    ax.set_xlabel("棋盘规模 N")
    ax.set_ylabel("单节点计数时间 / 秒（对数轴）")
    ax.set_xticks(range(16, 29))
    ax.grid(True, which="both", alpha=0.75)
    ax.set_title("当前精确 PEPS 后端到 Q(28) 的资源外推", fontsize=16, fontweight="bold", color="#102a43")
    ax.legend(loc="upper left", fontsize=9)
    fig.text(
        0.5,
        -0.015,
        "拟合窗口 N=18–22，并锚定实测 N=22；N=28 施加实测 4/3-prime 几何开销。结果是敏感性分析，不是复杂度定理或置信区间。",
        ha="center",
        fontsize=8.5,
        color="#526b82",
    )
    save_svg(fig, OUT / "q28_projection_zh.svg")
    plt.close(fig)


def plot_truncation() -> None:
    rows = read_csv(BENCH / "e51_truncated_boundary_mps_tradeoff.csv")
    rows = [row for row in rows if int(row["N"]) == 14 and int(row["chi"]) in {4, 8, 16, 32, 64, 128}]
    rows.sort(key=lambda row: int(row["chi"]))
    if [int(row["chi"]) for row in rows] != [4, 8, 16, 32, 64, 128]:
        raise ValueError("incomplete N=14 chi sweep")
    for row in rows:
        if int(row["exact_count"]) != KNOWN[14]:
            raise ValueError("APX1 exact reference mismatch")

    chi = [int(row["chi"]) for row in rows]
    estimate = [float(row["estimate"]) for row in rows]
    elapsed = [float(row["median_elapsed_s"]) for row in rows]
    exact = KNOWN[14]
    exact_peps_s = 0.015146118

    fig, axes = plt.subplots(1, 2, figsize=(13, 5.1), constrained_layout=True)
    axes[0].plot(chi, estimate, marker="o", linewidth=2.3, color="#7b4ab5", label="截断后的浮点估计")
    axes[0].axhline(exact, color="#d94841", linestyle="--", linewidth=2.1, label=f"精确值 Q(14)={exact:,}")
    axes[0].set_yscale("symlog", linthresh=0.5, linscale=0.8, base=10)
    axes[0].set_xlabel("最大边界维数 χ")
    axes[0].set_ylabel("收缩得到的计数估计（symlog）")
    axes[0].set_title("增大 χ 仍未恢复计数", fontsize=14, fontweight="bold", color="#102a43")
    axes[0].grid(True, which="both", alpha=0.75)
    axes[0].legend(fontsize=9, loc="upper left")
    for x, y in zip(chi, estimate):
        label = "≈0" if abs(y) < 1e-6 else f"{y:.3g}"
        axes[0].annotate(label, (x, y), xytext=(0, 7), textcoords="offset points", ha="center", fontsize=8)

    axes[1].plot(chi, elapsed, marker="s", linewidth=2.3, color="#d97706", label="截断 boundary-MPS")
    axes[1].axhline(exact_peps_s, color="#087f5b", linestyle="--", linewidth=2.1, label="同节点精确 PEPS：0.0151 s")
    axes[1].set_yscale("log")
    axes[1].set_xlabel("最大边界维数 χ")
    axes[1].set_ylabel("时间 / 秒（对数轴）")
    axes[1].set_title("精度未恢复，成本却快速上升", fontsize=14, fontweight="bold", color="#102a43")
    axes[1].grid(True, which="both", alpha=0.75)
    axes[1].legend(fontsize=9, loc="upper left")
    axes[1].annotate(
        "χ=128：198.4 s\n仍低估 99.9963%",
        (128, elapsed[-1]),
        xytext=(-90, -28),
        textcoords="offset points",
        arrowprops={"arrowstyle": "->", "color": "#6b4b1f"},
        fontsize=9,
        color="#6b4b1f",
    )

    fig.suptitle("APX1：N=14 有限 PEPS 截断的精度—成本诊断", fontsize=17, fontweight="bold", color="#102a43")
    save_svg(fig, OUT / "truncation_n14_zh.svg")
    plt.close(fig)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    configure_plotting()
    grouped, missing = load_comparison()
    plot_comparison(grouped, missing)
    measured, lane_overhead = load_e50_scaling()
    predictions = projection_models(measured, lane_overhead)
    write_projection_csv(measured, predictions, lane_overhead)
    plot_projection(measured, predictions)
    plot_truncation()
    print(f"comparison_rows={sum(len(rows) for rows in grouped.values())}")
    print(f"comparison_missing={missing}")
    print(f"N22_seconds={measured[22]:.9f}")
    print(f"lane_overhead={lane_overhead:.9f}")
    for model, values in predictions.items():
        print(f"Q28_projection_years[{model}]={values[28] / (365.25 * 86400):.6f}")


if __name__ == "__main__":
    main()

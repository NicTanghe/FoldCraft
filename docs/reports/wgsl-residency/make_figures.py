#!/usr/bin/env python3
"""Render recorded measurements; never starts a simulation or requests a GPU."""
import json
from pathlib import Path
from statistics import median
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

HERE = Path(__file__).resolve().parent
DATA = HERE.parents[1] / "wgsl-performance-results.json"
data = json.loads(DATA.read_text())
rows = {r["case"]: r for r in data["rows"]}
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 9,
    "axes.spines.top": False, "axes.spines.right": False,
    "svg.fonttype": "path", "svg.hashsalt": "CC-WGSL-2026-01", "pdf.fonttype": 42})
colors = {"Before": "#9aa8b5", "Optimized": "#245c7a", "Hybrid": "#a0522d"}
fig, axes = plt.subplots(1, 2, figsize=(7.1, 2.85), sharey=False)
for ax, suffix, title in zip(axes, ["", "-draped"], ["Assembly · 29 warm steps", "Draped · 19 warm steps"]):
    x = np.arange(2)
    for offset, variant, label in [(-.19, "before", "Before"), (.19, "after", "Optimized")]:
        vals = [rows[f"repeat-{variant}-{b}{suffix}"]["warm_median_ms"] for b in ["ipc", "al"]]
        bars = ax.bar(x + offset, vals, width=.36, color=colors[label], label=label, zorder=3)
        for bar, value in zip(bars, vals):
            ax.text(bar.get_x() + bar.get_width()/2, value + (8 if suffix else 4), f"{value:.1f}", ha="center", fontsize=8)
    control = rows[f"repeat-hybrid{suffix}"]["warm_median_ms"]
    ax.axhline(control, color=colors["Hybrid"], linestyle="--", linewidth=1.2, label="Hybrid control", zorder=4)
    ax.set_xticks(x, ["IPC-style", "Barrier AL"])
    ax.set_title(title, loc="left", fontsize=10, weight="bold", pad=10)
    ax.set_ylabel("Complete-step warm median (ms)")
    ax.set_ylim(0, 320 if not suffix else 650)
    ax.grid(axis="y", color="#e4e8ec", zorder=0)
handles, labels = axes[0].get_legend_handles_labels()
fig.legend(handles, labels, loc="upper center", bbox_to_anchor=(.5, 1.02), ncol=3, frameon=False, fontsize=8)
fig.tight_layout(w_pad=2, rect=(0, 0, 1, .91))
fig.savefig(HERE / "figures" / "complete-step-cost.svg", bbox_inches="tight", metadata={"Date": None})
plt.close(fig)

categories = ["PCG incl. initialization", "Detection / incidence", "Trial energy", "Material gradient cache", "Proposal / path bounds", "Other"]
stage_colors = ["#245c7a", "#658ca4", "#a0522d", "#c8a064", "#778c68", "#c8ced3"]
profiles = data["profiles"]
values = []
for key in ["profile-before", "profile-active-final"]:
    phase_names = set().union(*(p["phases_ms"] for p in profiles[key]))
    med = {k: median(p["phases_ms"].get(k,0) for p in profiles[key]) for k in phase_names}
    cg = sum(v for k,v in med.items() if k == "pcg" or k.startswith("cg_"))
    detection = med.get("detection",0) + med.get("coupled_incidence",0)
    main = [cg, detection, med["line_search"], med["material_cache"], med["proposal_bounds"]]
    values.append(main + [sum(med.values()) - sum(main)])
fig, ax = plt.subplots(figsize=(7.1, 2.65))
left = np.zeros(2)
for i, (label, color) in enumerate(zip(categories, stage_colors)):
    widths = np.array([v[i] for v in values])
    ax.barh([1,0], widths, left=left, height=.48, label=label, color=color)
    for y,l,w in zip([1,0],left,widths):
        if w>20: ax.text(l+w/2,y,f"{w:.0f}",ha="center",va="center",fontsize=8,color="white" if i in [0,2] else "#15202b")
    left += widths
ax.set_yticks([1,0],["Before", "Optimized"])
ax.set_xlim(0,610)
ax.set_xlabel("Sum of per-phase device-time medians (ms)")
ax.grid(axis="x",color="#e4e8ec",zorder=0)
ax.set_axisbelow(True)
ax.legend(loc="lower center",bbox_to_anchor=(.5,1.01),ncol=3,frameon=False,fontsize=7.4)
fig.tight_layout()
fig.savefig(HERE / "figures" / "phase-cost.svg", bbox_inches="tight", metadata={"Date": None})
plt.close(fig)
print("Rendered two vector figures from", DATA)

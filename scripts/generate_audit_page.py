#!/usr/bin/env python3
"""Generate a churn-vs-LOC HTML audit report from git history."""

from __future__ import annotations

import argparse
import html
import json
import subprocess
from pathlib import Path


FILES = [
    ("grammar.pest", "crates/mtg-grammar/src/grammar.pest", "grammar"),
    ("ast.rs", "crates/mtg-grammar/src/ast.rs", "grammar"),
    ("parse.rs", "crates/mtg-grammar/src/parse.rs", "grammar"),
    ("unparse.rs", "crates/mtg-grammar/src/unparse.rs", "grammar"),
    ("semantic/ir.rs", "crates/mtg-semantic/src/ir.rs", "semantic"),
    ("semantic/lower.rs", "crates/mtg-semantic/src/lower.rs", "semantic"),
    ("grammar/tests/prop.rs", "crates/mtg-grammar/tests/prop.rs", "tests"),
    ("semantic/tests/prop.rs", "crates/mtg-semantic/tests/prop.rs", "tests"),
    ("corpus_status.json", "corpus_status.json", "corpus"),
]


DEFAULT_REFS = "d6cb122:Baseline,HEAD:Current"


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True)


def short_ref(ref: str) -> str:
    return git("rev-parse", "--short", ref).strip()


def loc_at(ref: str, path: str) -> int:
    try:
        text = git("show", f"{ref}:{path}")
    except subprocess.CalledProcessError:
        return 0
    return len(text.splitlines())


def churn_at(ref: str, path: str) -> int:
    names = git("log", "--format=", "--name-only", "-n", "200", ref, "--", path).splitlines()
    return sum(1 for name in names if name == path)


def commit_index() -> dict[str, int]:
    """Map every commit's 7-char sha to its 1-based chronological index."""
    out = git("log", "--reverse", "--format=%H")
    return {sha[:7]: i + 1 for i, sha in enumerate(out.splitlines())}


def history(path: str) -> list[dict[str, int | str]]:
    """Cumulative LOC per commit that touched `path`, oldest first."""
    out = git("log", "--follow", "--reverse", "--format=__C__ %H %at", "--numstat", "--", path)
    points: list[dict[str, int | str]] = []
    loc = 0
    cur_sha: str | None = None
    cur_ts = 0
    added = 0
    deleted = 0
    for line in out.splitlines():
        if line.startswith("__C__ "):
            if cur_sha is not None:
                loc += added - deleted
                points.append({"sha": cur_sha[:7], "ts": cur_ts, "loc": loc})
            _, cur_sha, ts_str = line.split(maxsplit=2)
            cur_ts = int(ts_str)
            added = deleted = 0
        elif line.strip() and cur_sha is not None:
            parts = line.split("\t")
            if len(parts) >= 2 and parts[0] != "-":
                added += int(parts[0])
                deleted += int(parts[1])
    if cur_sha is not None:
        loc += added - deleted
        points.append({"sha": cur_sha[:7], "ts": cur_ts, "loc": loc})
    return points


def parse_refs(raw: str) -> list[dict[str, str]]:
    refs = []
    for item in raw.split(","):
        item = item.strip()
        if not item:
            continue
        if ":" in item:
            ref, label = item.split(":", 1)
        else:
            ref, label = item, item
        refs.append({"ref": ref.strip(), "label": label.strip(), "sha": short_ref(ref.strip())})
    if len(refs) < 2:
        raise SystemExit("--refs must contain at least two refs")
    return refs


def collect(refs: list[dict[str, str]]) -> list[dict[str, object]]:
    idx_map = commit_index()
    rows = []
    for display, path, group in FILES:
        points = []
        for ref in refs:
            points.append(
                {
                    "ref": ref["ref"],
                    "label": ref["label"],
                    "sha": ref["sha"],
                    "churn": churn_at(ref["ref"], path),
                    "loc": loc_at(ref["ref"], path),
                }
            )
        hist = history(path)
        for point in hist:
            point["idx"] = idx_map.get(point["sha"], 0)  # type: ignore[index]
        rows.append(
            {
                "file": display,
                "path": path,
                "group": group,
                "points": points,
                "history": hist,
            }
        )
    return rows


def render(refs: list[dict[str, str]], rows: list[dict[str, object]]) -> str:
    baseline_idx = 0
    current_idx = len(refs) - 1
    total_delta = sum(
        row["points"][current_idx]["loc"] - row["points"][baseline_idx]["loc"]  # type: ignore[index]
        for row in rows
    )
    parse_row = next(row for row in rows if row["file"] == "parse.rs")
    parse_delta = parse_row["points"][current_idx]["loc"] - parse_row["points"][baseline_idx]["loc"]  # type: ignore[index]
    payload = json.dumps({"refs": refs, "rows": rows}).replace("</", "<\\/")
    ref_metrics = "\n".join(
        f'    <div class="metric"><strong>{html.escape(ref["sha"])}</strong>'
        f'<span>{html.escape(ref["label"])}</span></div>'
        for ref in refs
    )
    title = "MTG Parser Audit: Churn vs Lines of Code"
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html.escape(title)}</title>
  <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/hammerjs@2.0.8/hammer.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/chartjs-plugin-zoom@2.0.1/dist/chartjs-plugin-zoom.min.js"></script>
  <style>
    :root {{
      --bg: #f6f7f9; --panel: #fff; --ink: #16181d; --muted: #5b6472;
      --border: #e3e7ee;
    }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; background: var(--bg); color: var(--ink); font: 14px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
    main {{ max-width: 1180px; margin: 0 auto; padding: 28px; }}
    h1 {{ margin: 0 0 6px; font-size: 28px; }}
    h2 {{ margin: 0 0 14px; font-size: 18px; }}
    p {{ margin: 0; color: var(--muted); }}
    section, .metric {{ background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04); }}
    section {{ padding: 18px; margin-top: 16px; }}
    .summary {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; margin: 22px 0; }}
    .refs {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 12px; margin: 0 0 22px; }}
    .metric {{ padding: 14px 16px; }}
    .metric strong {{ display: block; font-size: 24px; line-height: 1.1; }}
    .metric span {{ display: block; margin-top: 5px; color: var(--muted); font-size: 12px; }}
    .chart-wrap {{ position: relative; height: 480px; }}
    .chart-wrap.tall {{ height: 520px; }}
    .chart-wrap.short {{ height: 360px; }}
    .note {{ margin-top: 10px; font-size: 13px; }}
    .hint {{ font-size: 12px; color: var(--muted); margin-bottom: 8px; }}
    table {{ width: 100%; border-collapse: collapse; margin-top: 10px; font-variant-numeric: tabular-nums; }}
    th, td {{ padding: 8px 10px; border-bottom: 1px solid #e8ebf0; text-align: right; white-space: nowrap; }}
    th:first-child, td:first-child {{ text-align: left; white-space: normal; }}
    th {{ color: var(--muted); font-weight: 600; font-size: 12px; }}
    .neg {{ color: #047857; font-weight: 600; }} .pos {{ color: #b91c1c; font-weight: 600; }} .zero {{ color: var(--muted); }}
    @media (max-width: 780px) {{ main {{ padding: 18px; }} .summary {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }} .chart-wrap {{ height: 360px; }} th, td {{ padding: 7px 6px; font-size: 12px; }} }}
  </style>
</head>
<body>
<main>
  <h1>{html.escape(title)}</h1>
  <p>Generated from git. Churn is file touches in the last 200 commits at each ref; complexity is file LOC.</p>
  <div class="summary">
    <div class="metric"><strong>{total_delta:+}</strong><span>tracked LOC delta vs baseline</span></div>
    <div class="metric"><strong>{parse_delta:+}</strong><span>parse.rs LOC delta vs baseline</span></div>
    <div class="metric"><strong>{html.escape(refs[0]["sha"])}</strong><span>baseline ref</span></div>
    <div class="metric"><strong>{html.escape(refs[-1]["sha"])}</strong><span>current ref</span></div>
  </div>
  <div class="refs">
{ref_metrics}
  </div>
  <section>
    <h2>Churn vs LOC Hotspot Movement</h2>
    <div class="hint">Hover for details. Click legend entries to toggle. Scroll to zoom, drag to pan.</div>
    <div class="chart-wrap"><canvas id="scatter"></canvas></div>
    <p class="note">A lower point means less code in that hotspot. A point further right means the file was touched more often in the sampled history.</p>
  </section>
  <section>
    <h2>Lines of Code Delta by File</h2>
    <div class="chart-wrap short"><canvas id="bars"></canvas></div>
  </section>
  <section>
    <h2>LOC Over Time (per commit)</h2>
    <div class="hint">X-axis is the chronological commit number across the whole repo. Hover, toggle files in the legend, scroll to zoom, drag to pan.</div>
    <div class="chart-wrap tall"><canvas id="series"></canvas></div>
    <p class="note">Each line is a file's LOC at every commit that touched it. Dashed lines are linear regressions over each file's full history — a flat or downward trend means the cleanup is winning.</p>
  </section>
  <section>
    <h2>Raw Data</h2>
    <table><thead id="head"></thead><tbody id="rows"></tbody></table>
  </section>
</main>
<script id="audit-data" type="application/json">{payload}</script>
<script>
const data = JSON.parse(document.getElementById("audit-data").textContent);
const filePalette = ["#2563eb", "#dc2626", "#059669", "#7c3aed", "#d97706", "#0891b2", "#db2777", "#65a30d", "#475569"];
const refPalette = ["#6b7280", "#2563eb", "#dc2626", "#059669", "#7c3aed"];
const zoomOpts = {{ zoom: {{ wheel: {{ enabled: true }}, pinch: {{ enabled: true }}, mode: "xy" }}, pan: {{ enabled: true, mode: "xy" }} }};

function linreg(xs, ys) {{
  const n = xs.length;
  if (n < 2) return null;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let num = 0, den = 0;
  for (let i = 0; i < n; i++) {{ num += (xs[i] - mx) * (ys[i] - my); den += (xs[i] - mx) ** 2; }}
  if (den === 0) return null;
  return {{ slope: num / den, intercept: my - (num / den) * mx }};
}}

function buildScatter() {{
  const datasets = data.refs.map((ref, idx) => ({{
    label: `${{ref.label}} (${{ref.sha}})`,
    data: data.rows.map(row => ({{ x: row.points[idx].churn, y: row.points[idx].loc, file: row.file }})),
    backgroundColor: refPalette[idx % refPalette.length] + "55",
    borderColor: refPalette[idx % refPalette.length],
    borderWidth: 2,
    pointRadius: 7,
    pointHoverRadius: 10,
  }}));
  new Chart(document.getElementById("scatter"), {{
    type: "scatter",
    data: {{ datasets }},
    options: {{
      maintainAspectRatio: false,
      scales: {{
        x: {{ title: {{ display: true, text: "Churn (touches in last 200 commits)" }}, beginAtZero: true }},
        y: {{ title: {{ display: true, text: "LOC" }}, beginAtZero: true }},
      }},
      plugins: {{
        tooltip: {{ callbacks: {{ label: ctx => `${{ctx.raw.file}} — churn ${{ctx.parsed.x}}, LOC ${{ctx.parsed.y}}` }} }},
        zoom: zoomOpts,
      }},
    }},
  }});
}}

function buildBars() {{
  const lastIdx = data.refs.length - 1;
  const deltas = data.rows.map(r => r.points[lastIdx].loc - r.points[0].loc);
  new Chart(document.getElementById("bars"), {{
    type: "bar",
    data: {{
      labels: data.rows.map(r => r.file),
      datasets: [{{
        label: "LOC delta",
        data: deltas,
        backgroundColor: deltas.map(d => d <= 0 ? "rgba(5,150,105,0.72)" : "rgba(220,38,38,0.72)"),
        borderColor: deltas.map(d => d <= 0 ? "#059669" : "#dc2626"),
        borderWidth: 1,
      }}],
    }},
    options: {{
      indexAxis: "y",
      maintainAspectRatio: false,
      plugins: {{
        legend: {{ display: false }},
        tooltip: {{ callbacks: {{ label: ctx => `${{ctx.parsed.x > 0 ? "+" : ""}}${{ctx.parsed.x}} LOC` }} }},
      }},
      scales: {{
        x: {{ title: {{ display: true, text: `LOC change: ${{data.refs[0].label}} → ${{data.refs[lastIdx].label}}` }} }},
      }},
    }},
  }});
}}

function buildSeries() {{
  const datasets = [];
  data.rows.forEach((row, i) => {{
    if (row.history.length === 0) return;
    const color = filePalette[i % filePalette.length];
    datasets.push({{
      label: row.file,
      data: row.history.map(p => ({{ x: p.idx, y: p.loc, sha: p.sha }})),
      borderColor: color,
      backgroundColor: color,
      borderWidth: 1.6,
      pointRadius: 2,
      pointHoverRadius: 5,
      tension: 0,
    }});
    const xs = row.history.map(p => p.idx);
    const ys = row.history.map(p => p.loc);
    const reg = linreg(xs, ys);
    if (reg) {{
      const x1 = xs[0], x2 = xs[xs.length - 1];
      datasets.push({{
        label: `${{row.file}} trend`,
        data: [{{ x: x1, y: reg.slope * x1 + reg.intercept }}, {{ x: x2, y: reg.slope * x2 + reg.intercept }}],
        borderColor: color,
        backgroundColor: color,
        borderDash: [6, 6],
        borderWidth: 1,
        pointRadius: 0,
        tension: 0,
      }});
    }}
  }});
  new Chart(document.getElementById("series"), {{
    type: "line",
    data: {{ datasets }},
    options: {{
      maintainAspectRatio: false,
      interaction: {{ mode: "nearest", axis: "x", intersect: false }},
      scales: {{
        x: {{ type: "linear", title: {{ display: true, text: "Commit number (chronological)" }}, ticks: {{ precision: 0 }} }},
        y: {{ title: {{ display: true, text: "LOC" }}, beginAtZero: true }},
      }},
      plugins: {{
        tooltip: {{ callbacks: {{ label: ctx => ctx.raw.sha
          ? `${{ctx.dataset.label}}: ${{ctx.parsed.y}} LOC @ ${{ctx.raw.sha}} (commit #${{ctx.parsed.x}})`
          : `${{ctx.dataset.label}}: ${{Math.round(ctx.parsed.y)}} LOC` }} }},
        zoom: zoomOpts,
      }},
    }},
  }});
}}

function fillTable() {{
  const head = document.getElementById("head");
  head.innerHTML = `<tr><th>File</th>${{data.refs.map(r => `<th>${{r.label}}<br>churn / LOC</th>`).join("")}}<th>LOC delta</th></tr>`;
  const rows = document.getElementById("rows");
  data.rows.forEach(row => {{
    const delta = row.points[data.refs.length - 1].loc - row.points[0].loc;
    const cls = delta < 0 ? "neg" : delta > 0 ? "pos" : "zero";
    const tr = document.createElement("tr");
    tr.innerHTML = `<td>${{row.file}}</td>${{row.points.map(p => `<td>${{p.churn}} / ${{p.loc}}</td>`).join("")}}<td class="${{cls}}">${{delta > 0 ? "+" : ""}}${{delta}}</td>`;
    rows.appendChild(tr);
  }});
}}

buildScatter();
buildBars();
buildSeries();
fillTable();
</script>
</body>
</html>
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="audit-churn-complexity.html")
    parser.add_argument("--refs", default=DEFAULT_REFS)
    args = parser.parse_args()

    refs = parse_refs(args.refs)
    rows = collect(refs)
    Path(args.out).write_text(render(refs, rows), encoding="utf-8")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()

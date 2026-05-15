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
        rows.append(
            {
                "file": display,
                "path": path,
                "group": group,
                "points": points,
                "history": history(path),
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
  <style>
    :root {{
      --bg: #f6f7f9; --panel: #fff; --ink: #16181d; --muted: #5b6472;
      --grid: #d8dde6; --border: #e3e7ee;
      --c0: #6b7280; --c1: #2563eb; --c2: #dc2626; --c3: #059669; --c4: #7c3aed;
    }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; background: var(--bg); color: var(--ink); font: 14px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
    main {{ max-width: 1180px; margin: 0 auto; padding: 28px; }}
    h1 {{ margin: 0 0 6px; font-size: 28px; letter-spacing: 0; }}
    h2 {{ margin: 0 0 14px; font-size: 18px; letter-spacing: 0; }}
    p {{ margin: 0; color: var(--muted); }}
    section, .metric {{ background: var(--panel); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04); }}
    section {{ padding: 18px; margin-top: 16px; }}
    .summary {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; margin: 22px 0; }}
    .refs {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 12px; margin: 0 0 22px; }}
    .metric {{ padding: 14px 16px; }}
    .metric strong {{ display: block; font-size: 24px; line-height: 1.1; }}
    .metric span {{ display: block; margin-top: 5px; color: var(--muted); font-size: 12px; }}
    .legend {{ display: flex; flex-wrap: wrap; gap: 10px 16px; margin: 10px 0 4px; color: var(--muted); font-size: 12px; }}
    .legend span {{ display: inline-flex; align-items: center; gap: 6px; }}
    .dot {{ width: 10px; height: 10px; border-radius: 999px; display: inline-block; }}
    svg {{ width: 100%; height: auto; display: block; overflow: visible; }}
    .axis {{ stroke: #8b95a5; stroke-width: 1; }}
    .grid {{ stroke: var(--grid); stroke-width: 1; stroke-dasharray: 2 4; }}
    .label {{ fill: var(--muted); font-size: 12px; }}
    .file-label {{ fill: var(--ink); font-size: 11px; }}
    .note {{ margin-top: 10px; font-size: 13px; }}
    table {{ width: 100%; border-collapse: collapse; margin-top: 10px; font-variant-numeric: tabular-nums; }}
    th, td {{ padding: 8px 10px; border-bottom: 1px solid #e8ebf0; text-align: right; white-space: nowrap; }}
    th:first-child, td:first-child {{ text-align: left; white-space: normal; }}
    th {{ color: var(--muted); font-weight: 600; font-size: 12px; }}
    .neg {{ color: #047857; font-weight: 600; }} .pos {{ color: #b91c1c; font-weight: 600; }} .zero {{ color: var(--muted); }}
    @media (max-width: 780px) {{ main {{ padding: 18px; }} .summary {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }} th, td {{ padding: 7px 6px; font-size: 12px; }} }}
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
    <div class="legend" id="legend"></div>
    <svg id="scatter" viewBox="0 0 1080 520" role="img" aria-label="Scatter plot of churn versus file LOC"></svg>
    <p class="note">A lower point means less code in that hotspot. A point further right means the file was touched more often in the sampled history.</p>
  </section>
  <section>
    <h2>Lines of Code Delta by File</h2>
    <svg id="bars" viewBox="0 0 1080 430" role="img" aria-label="Bar chart of LOC deltas"></svg>
  </section>
  <section>
    <h2>LOC Over Time (per commit)</h2>
    <div class="legend" id="series-legend"></div>
    <svg id="series" viewBox="0 0 1080 520" role="img" aria-label="Per-file LOC over commit history"></svg>
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
const colors = ["var(--c0)", "var(--c1)", "var(--c2)", "var(--c3)", "var(--c4)"];
function add(svg, name, attrs = {{}}, text = null) {{
  const el = document.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attrs)) el.setAttribute(key, value);
  if (text !== null) el.textContent = text;
  svg.appendChild(el);
  return el;
}}
function point(row, idx) {{ return row.points[idx]; }}
function drawLegend() {{
  const legend = document.getElementById("legend");
  data.refs.forEach((ref, idx) => {{
    const span = document.createElement("span");
    span.innerHTML = `<i class="dot" style="background:${{colors[idx % colors.length]}}"></i>${{ref.label}} (${{ref.sha}})`;
    legend.appendChild(span);
  }});
}}
function drawScatter() {{
  const svg = document.getElementById("scatter");
  const W = 1080, H = 520, L = 78, R = 32, T = 28, B = 62;
  const maxChurn = Math.max(140, ...data.rows.flatMap(r => r.points.map(p => p.churn))) + 5;
  const maxLoc = Math.max(4000, ...data.rows.flatMap(r => r.points.map(p => p.loc)));
  const x = churn => L + (churn / maxChurn) * (W - L - R);
  const y = loc => T + (1 - loc / maxLoc) * (H - T - B);
  [1000, 2000, 3000, 4000].forEach(loc => {{
    add(svg, "line", {{ class: "grid", x1: L, x2: W - R, y1: y(loc), y2: y(loc) }});
    add(svg, "text", {{ class: "label", x: 18, y: y(loc) + 4 }}, String(loc));
  }});
  [40, 80, 120].forEach(churn => {{
    add(svg, "line", {{ class: "grid", x1: x(churn), x2: x(churn), y1: T, y2: H - B }});
    add(svg, "text", {{ class: "label", x: x(churn) - 8, y: H - B + 24 }}, String(churn));
  }});
  add(svg, "line", {{ class: "axis", x1: L, x2: W - R, y1: H - B, y2: H - B }});
  add(svg, "line", {{ class: "axis", x1: L, x2: L, y1: T, y2: H - B }});
  add(svg, "text", {{ class: "label", x: W / 2, y: H - 16, "text-anchor": "middle" }}, "churn: touches in last 200 commits");
  add(svg, "text", {{ class: "label", x: 20, y: 22 }}, "LOC");
  data.rows.forEach(row => {{
    row.points.forEach((p, idx) => {{
      const jitter = (idx - (data.refs.length - 1) / 2) * 6;
      add(svg, "circle", {{ cx: x(p.churn) + jitter, cy: y(p.loc) + jitter, r: row.group === "grammar" ? 8 : 6, fill: colors[idx % colors.length], "fill-opacity": "0.25", stroke: colors[idx % colors.length], "stroke-width": 2 }});
    }});
  }});
  data.rows.forEach(row => {{
    const p = row.points[row.points.length - 1];
    const anchor = p.churn > 110 ? "end" : "start";
    const dx = p.churn > 110 ? -15 : 15;
    add(svg, "text", {{ class: "file-label", x: x(p.churn) + dx, y: y(p.loc) + 4, "text-anchor": anchor }}, row.file);
  }});
}}
function drawBars() {{
  const svg = document.getElementById("bars");
  const W = 1080, H = 430, L = 220, T = 24, B = 35, rowH = 38;
  const deltas = data.rows.map(r => point(r, data.refs.length - 1).loc - point(r, 0).loc);
  const minDelta = Math.min(-10, ...deltas);
  const maxDelta = Math.max(10, ...deltas);
  const zero = L + (Math.abs(minDelta) / (maxDelta - minDelta)) * 760;
  const scale = 760 / (maxDelta - minDelta);
  add(svg, "line", {{ class: "axis", x1: zero, x2: zero, y1: T, y2: H - B }});
  data.rows.forEach((row, i) => {{
    const y = T + i * rowH + 8;
    const delta = point(row, data.refs.length - 1).loc - point(row, 0).loc;
    const w = Math.abs(delta) * scale;
    add(svg, "text", {{ class: "file-label", x: 0, y: y + 16 }}, row.file);
    add(svg, "rect", {{ x: delta < 0 ? zero - w : zero, y, width: Math.max(w, 1), height: 18, rx: 2, fill: delta <= 0 ? "#059669" : "#dc2626", "fill-opacity": "0.72" }});
    add(svg, "text", {{ class: "label", x: delta < 0 ? zero - w - 8 : zero + w + 8, y: y + 14, "text-anchor": delta < 0 ? "end" : "start" }}, `${{delta > 0 ? "+" : ""}}${{delta}}`);
  }});
}}
const filePalette = ["#2563eb", "#dc2626", "#059669", "#7c3aed", "#d97706", "#0891b2", "#db2777", "#65a30d", "#475569"];
function linreg(xs, ys) {{
  const n = xs.length;
  if (n < 2) return null;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let num = 0, den = 0;
  for (let i = 0; i < n; i++) {{ num += (xs[i] - mx) * (ys[i] - my); den += (xs[i] - mx) ** 2; }}
  if (den === 0) return null;
  const slope = num / den;
  return {{ slope, intercept: my - slope * mx }};
}}
function drawSeries() {{
  const svg = document.getElementById("series");
  const W = 1080, H = 520, L = 78, R = 32, T = 28, B = 56;
  const all = data.rows.flatMap(r => r.history);
  if (all.length === 0) return;
  const minTs = Math.min(...all.map(p => p.ts));
  const maxTs = Math.max(...all.map(p => p.ts));
  const maxLoc = Math.max(100, ...all.map(p => p.loc));
  const x = ts => L + ((ts - minTs) / (maxTs - minTs || 1)) * (W - L - R);
  const y = loc => T + (1 - loc / maxLoc) * (H - T - B);
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {{
    const loc = Math.round((maxLoc / ticks) * i);
    add(svg, "line", {{ class: "grid", x1: L, x2: W - R, y1: y(loc), y2: y(loc) }});
    add(svg, "text", {{ class: "label", x: 18, y: y(loc) + 4 }}, String(loc));
  }}
  const months = 6;
  for (let i = 0; i <= months; i++) {{
    const ts = minTs + ((maxTs - minTs) / months) * i;
    add(svg, "line", {{ class: "grid", x1: x(ts), x2: x(ts), y1: T, y2: H - B }});
    const d = new Date(ts * 1000);
    const label = `${{d.getFullYear()}}-${{String(d.getMonth() + 1).padStart(2, "0")}}`;
    add(svg, "text", {{ class: "label", x: x(ts), y: H - B + 20, "text-anchor": "middle" }}, label);
  }}
  add(svg, "line", {{ class: "axis", x1: L, x2: W - R, y1: H - B, y2: H - B }});
  add(svg, "line", {{ class: "axis", x1: L, x2: L, y1: T, y2: H - B }});
  add(svg, "text", {{ class: "label", x: 20, y: 22 }}, "LOC");
  const legend = document.getElementById("series-legend");
  data.rows.forEach((row, i) => {{
    const color = filePalette[i % filePalette.length];
    if (row.history.length === 0) return;
    const path = row.history.map((p, idx) => `${{idx === 0 ? "M" : "L"}} ${{x(p.ts).toFixed(1)}} ${{y(p.loc).toFixed(1)}}`).join(" ");
    add(svg, "path", {{ d: path, fill: "none", stroke: color, "stroke-width": 1.6, "stroke-opacity": 0.85 }});
    const reg = linreg(row.history.map(p => p.ts), row.history.map(p => p.loc));
    if (reg) {{
      const x1 = row.history[0].ts, x2 = row.history[row.history.length - 1].ts;
      const y1 = reg.slope * x1 + reg.intercept;
      const y2 = reg.slope * x2 + reg.intercept;
      add(svg, "line", {{ x1: x(x1), x2: x(x2), y1: y(y1), y2: y(y2), stroke: color, "stroke-width": 1.2, "stroke-dasharray": "4 4", "stroke-opacity": 0.65 }});
    }}
    const span = document.createElement("span");
    span.innerHTML = `<i class="dot" style="background:${{color}}"></i>${{row.file}}`;
    legend.appendChild(span);
  }});
}}
function fillTable() {{
  const head = document.getElementById("head");
  head.innerHTML = `<tr><th>File</th>${{data.refs.map(r => `<th>${{r.label}}<br>churn / LOC</th>`).join("")}}<th>LOC delta</th></tr>`;
  const rows = document.getElementById("rows");
  data.rows.forEach(row => {{
    const delta = point(row, data.refs.length - 1).loc - point(row, 0).loc;
    const cls = delta < 0 ? "neg" : delta > 0 ? "pos" : "zero";
    const tr = document.createElement("tr");
    tr.innerHTML = `<td>${{row.file}}</td>${{row.points.map(p => `<td>${{p.churn}} / ${{p.loc}}</td>`).join("")}}<td class="${{cls}}">${{delta > 0 ? "+" : ""}}${{delta}}</td>`;
    rows.appendChild(tr);
  }});
}}
drawLegend(); drawScatter(); drawBars(); drawSeries(); fillTable();
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

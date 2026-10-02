"""Accessible, same-workload comparisons of validated benchmark snapshots.

This is a view over observations, never a performance acceptance gate. Ratios
use the baseline and subject in the same snapshot, with identical workload,
context, sample unit and measurement policy. No aggregate speed score is made.
"""
from __future__ import annotations

from collections import Counter
import html
import math
import re


BASELINES = {"pillow": "pillow", "fontdone": "FreeType", "jpeg": "libjpeg-turbo"}
BASELINE_FOR = {
    "python-cpu": "pillow",
    "python-simd": "pillow",
    "python-gpu": "pillow",
    "python-parallel-cpu": "pillow",
}
NAMES = {
    "pillow": "Pillow",
    "python-cpu": "pillow-rs · CPU",
    "python-simd": "pillow-rs · SIMD",
    "python-gpu": "pillow-rs · GPU",
    "python-parallel-cpu": "pillow-rs · Parallel CPU",
}


def escape(value: object) -> str:
    return html.escape(str(value), quote=True)


def duration(value: float | None) -> str:
    if value is None:
        return "Not measured"
    if value == 0:
        return "0 µs (below resolution)"
    scale, unit = (1_000_000, "s") if value >= 1_000_000 else (1000, "ms") if value >= 1000 else (1, "µs") if value >= 1 else (.001, "ns")
    return f"{value / scale:,.3g} {unit}"


def friendly(value: str) -> str:
    return re.sub(r"[._-]+", " ", value).strip().capitalize()


def describe(row: dict, kind: str) -> tuple[str, str, str]:
    """Names describe recorded IDs/context, not current source implementations."""
    workload, context = row["workload"], row["context"]
    if kind == "jpeg":
        operation = workload.split(".")[0].capitalize()
        size = f"{context.get('width', '?')} × {context.get('height', '?')}"
        mode = str(context.get("mode", "")).upper()
        options = [size, mode, f"quality {context.get('quality', '?')}"]
        subsampling = context.get("subsampling")
        if subsampling and mode == "RGB":
            options.append(":".join(str(subsampling)))
        for key, label in (("progressive", "progressive"), ("optimize", "optimized")):
            if str(context.get(key)) == "1":
                options.append(label)
        if str(context.get("restart_rows", "0")) != "0":
            options.append(f"restart every {context['restart_rows']} rows")
        return f"{operation} JPEG", operation, " · ".join(options)
    if kind == "fontdone":
        operation = re.sub(r"^(dejavusans|generated_cjk)_|_20$", "", workload)
        labels = {
            "load": ("Load a font", "Font setup"),
            "getname": ("Read the font name", "Font metadata"),
            "getmetrics": ("Read font metrics", "Font metadata"),
            "getlength_av": ('Measure text width: “AV”', "Text measurement"),
            "getbbox_hello": ('Measure text bounds: “Hello”', "Text measurement"),
            "getmask_a": ('Render grayscale glyph: “A”', "Glyph rendering"),
            "getmask_force_autohint_a": ('Render “A” with forced autohinting', "Glyph rendering"),
            "glyph_metrics_a": ('Read glyph metrics: “A”', "Text measurement"),
            "render_mono_a": ('Render monochrome glyph: “A”', "Glyph rendering"),
            "render_lcd_a": ('Render LCD glyph: “A”', "Glyph rendering"),
            "getmask_hani": ("Render a CJK glyph", "Glyph rendering"),
        }
        title, group = labels.get(operation, (friendly(workload), "Other operations"))
        context_text = ("Generated CJK font" if workload.startswith("generated_cjk") else "DejaVu Sans") + " · size 20"
        return title, group, context_text
    group = "Pipelines" if workload.startswith(("pipeline-chain.", "pipeline.quick.")) else "Lifecycle" if workload.startswith("pipeline-lifecycle.") else "Operations"
    name = re.sub(r"^pipeline-(?:op|chain|matrix|lifecycle)\.|^pipeline\.quick\.", "", workload)
    name = re.sub(r"^(?:reviewed|expanded)\.", "", name)
    name = re.sub(r"\.benchmark-materialized$|\.matrix-\d+x\d+$|\.\d+x\d+$|\.rgb-\d+$", "", name)
    if group == "Pipelines":
        title = friendly(name).replace("resize rotate crop", "resize → rotate → crop").replace("draw filter invert", "draw → filter → invert")
    else:
        title = friendly(name)
    aliases = {"Gaussianblur": "Gaussian blur", "Boxblur": "Box blur", "Alphacomposite": "Alpha composite",
               "Autocontrast": "Auto contrast", "Colorsaturation": "Color saturation", "Filter3x3": "3 × 3 convolution",
               "Filter5x5": "5 × 5 convolution", "Medianfilter": "Median filter", "Drawrectangle": "Draw rectangle"}
    title = aliases.get(title, title)
    parts = []
    if context.get("size"):
        parts.append(" × ".join(map(str, context["size"])))
    if context.get("mode"):
        parts.append(str(context["mode"]))
    if context.get("chain_length"):
        count = context["chain_length"]
        parts.append(f"{count} operation{'s' if count != 1 else ''}")
    parts.append("whole workflow" if row["policy"].get("boundary") == "whole_workflow" else str(row["policy"].get("boundary", "")))
    return title, group, " · ".join(filter(None, parts))


def facets(row: dict, kind: str) -> tuple[str, str, str, str, str]:
    """Return table, type, mode, title, and context for searchable rows."""
    title, group, context = describe(row, kind)
    mode = str(row["context"].get("mode") or "Not recorded")
    if kind != "pillow":
        return "operations", group, mode, title, context
    context = " · ".join(part for part in context.split(" · ") if part != mode)

    workload = row["workload"]
    if workload.startswith("pipeline-op."):
        operation_class = str(row["context"].get("operation_class") or "operation")
        operation_class = operation_class.replace("_", " ")
        return "operations", operation_class, mode, title, context
    if workload.startswith("pipeline-chain."):
        return "pipelines", "Composed chain", mode, title, context
    if workload.startswith("pipeline-matrix."):
        return "pipelines", "Matrix case", mode, title, context
    if workload.startswith("pipeline-lifecycle."):
        return "pipelines", "Lifecycle", mode, title, context
    if workload.startswith("pipeline.quick."):
        return "pipelines", "Quick workflow", mode, title, context
    return "pipelines", group, mode, title, context


def evidence(row: dict) -> tuple[str, str]:
    correctness = row["correctness"]
    if correctness in {"timing_only", "successful_execution: pass"}:
        return "timing", "Timing only · equal output not established"
    if correctness == "output hash match: True; byte length match: True":
        return "checked", "Output hash and size match"
    if correctness in {"parity_pass: pass", "source_target_match: pass", "exact_match"}:
        return "checked", "Output comparison passed"
    if "False" in correctness or correctness.endswith(": fail"):
        return "unavailable", "Output differs · speed comparison withheld"
    return "unavailable", f"Correctness not established: {correctness}"


def compare(row: dict, baseline: dict | None) -> tuple[float | None, str, str]:
    if baseline is None:
        return None, "unavailable", "Baseline not measured"
    if row["workload"] != baseline["workload"]:
        return None, "unavailable", "Different workloads"
    for key in ("policy", "context", "sample_unit", "sample_count"):
        if row[key] != baseline[key]:
            return None, "unavailable", "Measurement conditions differ"
    if row.get("comparison_group", "default") != baseline.get("comparison_group", "default"):
        return None, "unavailable", "Different benchmark cohorts"
    for item in (row, baseline):
        if item["status"] not in {"completed", "passed", "ok"}:
            return None, "unavailable", f"Run {item['status']}"
        value = item["median_us"]
        if value is None or not math.isfinite(value) or value <= 0:
            return None, "unavailable", "Positive measured time unavailable"
        state, label = evidence(item)
        if state == "unavailable":
            return None, state, label
        if item["requested_backend"] == "gpu" and item["actual_backend"] != "gpu":
            return None, "unavailable", "GPU request used a fallback; native GPU performance is not established"
        if item["requested_backend"] == "gpu" and item["terminal_complete"] is not True:
            return None, "unavailable", "GPU completion not established"
    state = "checked" if all(evidence(item)[0] == "checked" for item in (row, baseline)) else "timing"
    ratio = baseline["median_us"] / row["median_us"]
    if not math.isfinite(ratio) or ratio <= 0:
        return None, "unavailable", "Ratio is outside the supported numeric range"
    return ratio, state, evidence(row)[1] if state == evidence(row)[0] else evidence(baseline)[1]


def speed_label(ratio: float | None) -> str:
    if ratio is None:
        return "Not comparable"
    if ratio == 1:
        return "Same median time"
    factor = max(ratio, 1 / ratio)
    if factor < 1.005:
        return f"{abs(1 - 1 / ratio) * 100:.3g}% {'less' if ratio > 1 else 'more'} time"
    return f"{factor:,.3g}× {'faster' if ratio > 1 else 'slower'}"


def subject_name(row: dict) -> str:
    name = NAMES.get(row["subject"], row["subject"])
    if row["requested_backend"] != row["actual_backend"]:
        name += f" (actual: {row['actual_backend'] or 'unknown'})"
    return name


def render_dashboard(snapshot: dict, config: dict) -> str:
    kind = config["benchmark"]["kind"]
    primary_baseline = BASELINES[kind]
    grouped: dict[str, list[dict]] = {}
    for row in snapshot["rows"]:
        grouped.setdefault(row["workload"], []).append(row)
    groups = sorted({facets(rows[0], kind)[1] for rows in grouped.values()})
    modes = sorted({facets(rows[0], kind)[2] for rows in grouped.values()})
    all_subjects = {row["subject"] for row in snapshot["rows"]}
    if kind == "pillow":
        targets = [subject for subject in ("python-cpu", "python-simd", "python-gpu", "python-parallel-cpu")
                   if subject in all_subjects or subject.startswith("python-")]
        baseline_ids = ["pillow"]
    else:
        targets = list(dict.fromkeys(row["subject"] for row in snapshot["rows"] if row["subject"] != primary_baseline))
        baseline_ids = [primary_baseline]
    subjects = [primary_baseline]
    if "python-cpu" in targets:
        subjects.append("python-cpu")
    if "python-simd" in targets:
        subjects.append("python-simd")
    if "python-gpu" in targets:
        subjects.append("python-gpu")
    if kind == "pillow":
        subjects.append("python-parallel-cpu")
    subjects.extend(subject for subject in targets if subject not in subjects)
    counts = {subject: Counter() for subject in targets}
    table_rows = {"pipelines": [], "operations": []}
    for index, (workload, rows) in enumerate(grouped.items()):
        by_subject = {row["subject"]: row for row in rows}
        table_kind, workload_type, mode, title, context = facets(rows[0], kind)
        maximum = max((r["median_us"] or 0 for r in rows), default=0)
        cells = []
        quality_labels = set()
        comparable = []
        for subject in subjects:
            row = by_subject.get(subject)
            is_baseline = subject in baseline_ids
            if row is None:
                cells.append(f'<td data-subject="{escape(subject)}" data-role="{"baseline" if subject in baseline_ids else "target"}" data-direction="unavailable"><span class="bench-unavailable">Not measured</span></td>')
                if not is_baseline:
                    counts[subject]["unavailable"] += 1
                continue
            baseline_id = BASELINE_FOR.get(subject, primary_baseline)
            baseline = row if is_baseline else by_subject.get(baseline_id)
            ratio, quality, note = (None, "baseline", "Baseline") if is_baseline else compare(row, baseline)
            direction = "unavailable" if ratio is None else "faster" if ratio > 1 else "slower" if ratio < 1 else "tie"
            if not is_baseline:
                counts[subject][direction] += 1
                quality_labels.add(note)
            if ratio is not None:
                comparable.append(row)
            name = subject_name(row)
            width = (row["median_us"] or 0) / maximum * 100 if maximum else 0
            label = "Baseline" if is_baseline else speed_label(ratio)
            route = ""
            if row["requested_backend"] != row["actual_backend"]:
                route = f'<small class="bench-route">Actual: {escape(row["actual_backend"] or "unknown")}</small>'
            value = '' if row['median_us'] is None else str(row['median_us'])
            cells.append(f'<td data-subject="{escape(subject)}" data-role="{"baseline" if is_baseline else "target"}" data-value="{value}" data-ratio="{ratio if ratio is not None else ""}" data-direction="{direction}" data-quality="{quality}" data-note="{escape(note)}">'
                         f'<strong class="bench-time">{duration(row["median_us"])}</strong>'
                         f'<span class="bench-ratio {"baseline" if is_baseline else direction}">{label}</span>'
                         f'<span class="bench-track {"is-baseline" if is_baseline else direction}" aria-hidden="true"><span style="width:{width:.6f}%"></span></span>'
                         f'{route}<span class="bench-sr-only">{escape(name)}. {escape(note)}.</span></td>')
        # A row can be numerically fastest without providing matched-output proof.
        # Wording deliberately reports an observation, never a significance test.
        if len(comparable) >= 2:
            best = min(row["median_us"] for row in comparable)
            fastest = " / ".join(subject_name(row) for row in comparable if row["median_us"] == best)
        else:
            fastest = "Not comparable"
        detail_rows = []
        for row in rows:
            detail_rows.append(f'<tr><td>{escape(subject_name(row))}</td><td>{duration(row["median_us"])}</td><td>{duration(row["p90_us"])}</td><td>{duration(row["p95_us"])}</td><td>{escape(row["sample_count"])}</td><td>{escape(row["status"])}; {escape(row["correctness"])}</td></tr>')
        boundary = rows[0]["policy"].get("boundary", "Not recorded")
        category_cell = f'<td data-sort-value="{escape(workload_type)}">{escape(workload_type)}</td>'
        mode_cell = f'<td data-sort-value="{escape(mode)}">{escape(mode)}</td>'
        title_cell = f'<th scope="row"><span class="bench-workload-name">{escape(title)}</span><small>{escape(context)}</small></th>'
        status = f'<td class="bench-conclusion"><strong data-fastest>{escape(fastest)}</strong><small data-quality-summary>{escape("; ".join(sorted(quality_labels)) or "Comparison unavailable")}</small><button type="button" class="bench-expand" aria-expanded="false" aria-controls="bench-detail-{index}" hidden>Details</button></td>'
        search = " ".join([workload, title, workload_type, mode, context])
        table_rows[table_kind].append(f'<tr class="bench-workload" data-kind="{table_kind}" data-workload="{escape(workload)}" data-group="{escape(workload_type)}" data-mode="{escape(mode)}" data-name="{escape(title + " " + context)}" data-search="{escape(search)}" data-order="{index}">{title_cell}{category_cell}{mode_cell}{"".join(cells)}{status}</tr>')
        table_rows[table_kind].append(f'<tr class="bench-expanded" id="bench-detail-{index}" hidden><td colspan="{len(subjects)+4}"><p><strong>{escape(title)}</strong> · Timing boundary: {escape(boundary)}. Sample unit: {escape(rows[0]["sample_unit"])}. Percentiles show spread, not confidence intervals.</p>'
                    '<div class="bench-detail-table"><table><thead><tr><th>Implementation</th><th>Median</th><th>P90</th><th>P95</th><th>Samples</th><th>Recorded result</th></tr></thead><tbody>'
                    + "".join(detail_rows) + '</tbody></table></div>'
                    + f'<p>Workload ID: <code>{escape(workload)}</code></p></td></tr>')
    summary = []
    for subject, count in counts.items():
        baseline_id = BASELINE_FOR.get(subject, primary_baseline)
        baseline_name = NAMES.get(baseline_id, baseline_id)
        summary.append(f'<div class="bench-score" data-score-subject="{escape(subject)}"><strong>{escape(NAMES.get(subject, subject))}</strong>'
                       f'<span><b data-score="faster">{count["faster"]}</b> faster · <b data-score="slower">{count["slower"]}</b> slower</span>'
                       f'<small>vs {escape(baseline_name)} · <span data-score="tie">{count["tie"]}</span> equal · <span data-score="unavailable">{count["unavailable"]}</span> not comparable</small></div>')
    scope = {"pillow": "Python API operations and complete pipelines, including their declared setup and output steps. CPU, SIMD and GPU are separate implementations.",
             "fontdone": "Font loading, metadata, text measurement and glyph rendering. Complete multi-step text-layout pipelines are not measured in this snapshot.",
             "jpeg": "JPEG encode and decode across image sizes, quality and color settings. Other codecs and complete decode–encode pipelines are not measured in this snapshot."}[kind]
    options = ''.join(f'<option value="{escape(group)}">{escape(group)}</option>' for group in groups)
    mode_options = ''.join(f'<option value="{escape(mode)}">{escape(mode)}</option>' for mode in modes)
    subject_options = ''.join(f'<option value="{escape(subject)}">{escape(NAMES.get(subject, subject))}</option>' for subject in targets)
    headers = ''.join(f'<th scope="col" data-subject="{escape(subject)}" data-baseline-for="{escape(BASELINE_FOR.get(subject, subject))}" aria-sort="none"><button type="button" class="bench-sort" data-sort="{escape(subject)}">{escape(NAMES.get(subject, subject))}<span aria-hidden="true"> ↕</span></button><small>{"Baseline" if subject in baseline_ids else "Median · vs " + escape(NAMES.get(BASELINE_FOR.get(subject, primary_baseline), primary_baseline))}</small></th>' for subject in subjects)
    environment = snapshot["environment"]
    host = environment.get("platform") or environment.get("os") or "Host not recorded"
    table_head = ('<thead><tr>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="name">Operation / pipeline<span aria-hidden="true"> ↕</span></button></th>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="type">Type<span aria-hidden="true"> ↕</span></button></th>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="mode">Mode<span aria-hidden="true"> ↕</span></button></th>'
            + headers + '<th scope="col">Lowest median<small>Among comparable implementations</small></th></tr></thead>')
    pipeline_count = sum(1 for rows in grouped.values() if facets(rows[0], kind)[0] == "pipelines")
    operation_count = len(grouped) - pipeline_count
    return (f'<div class="benchmark-dashboard" data-baseline="{escape(primary_baseline)}">'
            f'<p class="bench-intro">Compare like-for-like workload results with the recorded Pillow baseline. CPU, SIMD, GPU, and Parallel CPU all use <strong>ordinary Pillow</strong>; Parallel CPU is measured separately with the opt-in Rayon feature. '
            '<strong>Lower time is better.</strong></p>'
            f'<div class="bench-summary">{"".join(summary)}</div>'
            '<p class="bench-summary-note">Observed median comparisons, not an overall score. Timing-only rows do not establish equal output; small differences may be noise.</p>'
            '<div class="bench-toolbar" hidden>'
            '<label class="bench-search">Find a workload<input id="evidence-filter" type="search" placeholder="Search operations, pipelines, sizes…" autocomplete="off"></label>'
            f'<label>Type<select id="bench-group"><option value="">All types</option>{options}</select></label>'
            f'<label>Mode<select id="bench-mode"><option value="">All modes</option>{mode_options}</select></label>'
            f'<label>Compare<select id="bench-subject"><option value="">All implementations</option>{subject_options}</select></label>'
            '<button type="button" id="bench-reset">Reset</button><output id="bench-count" aria-live="polite"></output></div>'
            '<p class="bench-chart-key">Shorter bars = less time within a row. Sort by type, mode, operation or pipeline, or backend latency. Scroll each table horizontally on small screens.</p>'
            '<section class="bench-section" data-table-kind="pipelines"><h2>Pipeline benchmarks</h2><p>Composed, matrix, lifecycle and quick workloads. Current snapshot: ' + str(pipeline_count) + ' pipelines.</p>'
            '<div class="bench-table-scroll" role="region" aria-label="Pipeline benchmark comparisons" tabindex="0">'
            '<table class="bench-comparison"><caption>Median time per pipeline workload; each backend is compared with its workload-matched Pillow timing.</caption>'
            + table_head + '<tbody>' + "".join(table_rows["pipelines"]) + '</tbody></table></div></section>'
            '<section class="bench-section" data-table-kind="operations"><h2>Individual operation benchmarks</h2><p>Single-operation workloads; each row retains its declared timing boundary. Current snapshot: ' + str(operation_count) + ' operation workloads.</p>'
            '<div class="bench-table-scroll" role="region" aria-label="Individual operation benchmark comparisons" tabindex="0">'
            '<table class="bench-comparison"><caption>Median time per individual operation workload; mode and operation type are explicit columns.</caption>'
            + table_head + '<tbody>' + "".join(table_rows["operations"]) + '</tbody></table></div></section>'
            '<p id="bench-empty" hidden>No workloads match these filters. Try another search or reset the filters.</p>'
            f'<p class="bench-host"><strong>{len(grouped)} recorded workloads: {pipeline_count} pipeline and {operation_count} individual-operation rows</strong> · {escape(snapshot["measured_at"][:10])} · {escape(host)}</p>'
            f'<p class="bench-scope">{escape(scope)}</p></div>')

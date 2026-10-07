"""Accessible, same-workload comparisons of validated benchmark snapshots.

This is a view over observations, never a performance acceptance gate. Ratios
use the baseline and subject in the same runner cohort, with identical
workload, context, sample unit and measurement policy. Overall summaries show
the per-workload distribution; they do not combine raw timings across hosts.
"""
from __future__ import annotations

from collections import Counter
import html
import math
import re
import statistics


BASELINES = {"pillow": "pillow", "fontdone": "FreeType", "jpeg": "libjpeg-turbo"}
BASELINE_FOR = {
    "python-cpu": "pillow",
    "python-simd": "pillow",
    "python-gpu": "pillow",
    "python-parallel-cpu": "pillow",
    "pillow-simd": "pillow",
}
NAMES = {
    "pillow": "Pillow",
    "python-cpu": "pillow-rs · CPU",
    "python-simd": "pillow-rs · SIMD",
    "python-gpu": "pillow-rs · GPU",
    "python-parallel-cpu": "pillow-rs · Parallel CPU",
    "pillow-simd": "Pillow-SIMD · SSE4",
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
    if row.get("machine_id") != baseline.get("machine_id") and (
        row.get("machine_id") is not None or baseline.get("machine_id") is not None
    ):
        return None, "unavailable", "Different runner cohorts"
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
        if item["requested_backend"] in {"cpu", "simd"} and item["actual_backend"] != item["requested_backend"]:
            return None, "unavailable", "Requested CPU/SIMD backend used a fallback; native performance is not established"
        if item["requested_backend"] in {"cpu", "simd", "gpu"} and item.get("fallback_reasons"):
            return None, "unavailable", "Backend fallback was recorded; native performance is not established"
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


def machine_catalog(snapshot: dict) -> dict[str, dict]:
    machines = snapshot.get("machines") or ([snapshot["machine"]] if snapshot.get("machine") else [])
    if machines:
        return {machine["id"]: machine for machine in machines}
    environment = snapshot.get("environment", {})
    label = environment.get("runner") or environment.get("os") or environment.get("platform") or "Runner not recorded"
    return {"legacy": {"id": "legacy", "label": label}}


def quantile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    position = (len(ordered) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def workload_distributions(grouped: dict, machines: dict[str, dict], targets: list[str],
                           kind: str) -> list[dict]:
    observations: dict[tuple[str, str, str, str | None], list[float]] = {}
    for (machine_id, _workload), rows in grouped.items():
        by_subject = {row["subject"]: row for row in rows}
        scope = facets(rows[0], kind)[0] if kind == "pillow" else None
        comparisons = [(subject, BASELINE_FOR.get(subject, BASELINES[kind])) for subject in targets]
        if kind == "pillow" and "pillow-simd" in by_subject:
            comparisons.extend((subject, "pillow-simd") for subject in ("python-cpu", "python-simd"))
        for subject, baseline_id in comparisons:
            target = by_subject.get(subject)
            baseline = by_subject.get(baseline_id)
            ratio, quality, _ = compare(target, baseline) if target and baseline else (None, "unavailable", "Missing pair")
            # Only a passing output comparison belongs in the headline. Execution-only
            # timings remain visible in the detailed tables but are not speed claims.
            if ratio is not None and quality == "checked":
                observations.setdefault((machine_id, subject, baseline_id, scope), []).append(ratio)
    result = []
    for (machine_id, subject, baseline, scope), ratios in sorted(observations.items()):
        ordered = sorted(ratios)
        result.append({
            "machine_id": machine_id,
            "machine_label": machines.get(machine_id, {}).get("label", machine_id),
            "subject": subject,
            "baseline": baseline,
            "scope": scope,
            "ratios": ordered,
            "q1": quantile(ordered, .25),
            "median": quantile(ordered, .5),
            "q3": quantile(ordered, .75),
            "geomean": math.exp(statistics.mean(math.log(value) for value in ordered)),
            "faster": sum(value > 1 for value in ordered),
            "slower": sum(value < 1 for value in ordered),
            "equal": sum(value == 1 for value in ordered),
        })
    return result


def render_boxplot(groups: list[dict]) -> str:
    """Render a log-scale Tukey box plot of parity-verified workload ratios."""
    left, right = 385, 1070
    minimum_log, maximum_log = -4.0, 4.0
    row_height = 54
    top = 26
    height = max(150, top + max(1, len(groups)) * row_height + 44)
    width = 1100

    def x_for(ratio: float) -> float:
        position = max(minimum_log, min(maximum_log, math.log2(ratio)))
        return left + (position - minimum_log) / (maximum_log - minimum_log) * (right - left)

    parts = [f'<svg class="bench-boxplot-svg" viewBox="0 0 {width} {height}" role="img" aria-labelledby="bench-boxplot-title bench-boxplot-desc">',
             '<title id="bench-boxplot-title">Per-workload speedup distribution</title>',
             '<desc id="bench-boxplot-desc">Each observation is one workload whose output comparison passed. Ratios above one are faster. Boxes show the first and third quartiles with a median line; whiskers reach the furthest observed point within one and a half interquartile ranges.</desc>']
    ticks = ((1 / 16, "1/16×"), (1 / 8, "1/8×"), (1 / 4, "1/4×"), (1 / 2, "1/2×"),
             (1, "1×"), (2, "2×"), (4, "4×"), (8, "8×"), (16, "16×"))
    for tick, label in ticks:
        x = x_for(tick)
        stroke = "#52616d" if tick == 1 else "#d1d7dc"
        thickness = "2" if tick == 1 else "1"
        parts.append(f'<line x1="{x:.1f}" y1="18" x2="{x:.1f}" y2="{height - 30}" stroke="{stroke}" stroke-width="{thickness}"/>')
        parts.append(f'<text class="bench-axis-label" x="{x:.1f}" y="{height - 8}" text-anchor="middle">{label}</text>')
    for index, item in enumerate(groups):
        y = top + index * row_height + 12
        ratios = item["ratios"]
        q1, median, q3 = item["q1"], item["median"], item["q3"]
        iqr = q3 - q1
        lower_bound, upper_bound = q1 - 1.5 * iqr, q3 + 1.5 * iqr
        inliers = [value for value in ratios if lower_bound <= value <= upper_bound]
        lower, upper = min(inliers), max(inliers)
        scope = {"operations": "individual operations", "pipelines": "pipeline workflows"}.get(item.get("scope"))
        scope_label = f' · {scope}' if scope else ""
        label = f'{item["machine_label"]} · {NAMES.get(item["subject"], item["subject"])} vs {NAMES.get(item["baseline"], item["baseline"])}{scope_label}'
        summary = f'n={len(ratios)} · geomean {speed_label(item["geomean"])} · {item["faster"]} faster / {item["slower"]} slower'
        parts.append(f'<text class="bench-box-label" x="8" y="{y - 3}">{escape(label)}</text>')
        parts.append(f'<text class="bench-box-summary" x="8" y="{y + 13}">{escape(summary)}</text>')
        parts.append(f'<line x1="{x_for(lower):.1f}" y1="{y + 2}" x2="{x_for(upper):.1f}" y2="{y + 2}" class="bench-whisker"/>')
        parts.append(f'<line x1="{x_for(lower):.1f}" y1="{y - 5}" x2="{x_for(lower):.1f}" y2="{y + 9}" class="bench-whisker"/>')
        parts.append(f'<line x1="{x_for(upper):.1f}" y1="{y - 5}" x2="{x_for(upper):.1f}" y2="{y + 9}" class="bench-whisker"/>')
        box_x, box_end = x_for(q1), x_for(q3)
        parts.append(f'<rect x="{box_x:.1f}" y="{y - 9}" width="{max(2.0, box_end - box_x):.1f}" height="22" class="bench-box"/>')
        parts.append(f'<line x1="{x_for(median):.1f}" y1="{y - 10}" x2="{x_for(median):.1f}" y2="{y + 14}" class="bench-median"/>')
        for value in ratios:
            if value < lower_bound or value > upper_bound:
                parts.append(f'<circle cx="{x_for(value):.1f}" cy="{y + 2}" r="3" class="bench-outlier"><title>{value:.5g}×</title></circle>')
    if not groups:
        parts.append(f'<text class="bench-empty-plot" x="{width / 2}" y="{height / 2}" text-anchor="middle">No parity-verified workload pairs match these filters.</text>')
    parts.append('</svg>')
    return "".join(parts)


def render_dashboard(snapshot: dict, config: dict) -> str:
    kind = config["benchmark"]["kind"]
    primary_baseline = BASELINES[kind]
    machines = machine_catalog(snapshot)
    grouped: dict[tuple[str, str], list[dict]] = {}
    for row in snapshot["rows"]:
        machine_id = row.get("machine_id", next(iter(machines)))
        grouped.setdefault((machine_id, row["workload"]), []).append(row)
    groups = sorted({facets(rows[0], kind)[1] for rows in grouped.values()})
    modes = sorted({facets(rows[0], kind)[2] for rows in grouped.values()})
    machine_options = sorted({(machine_id, machines[machine_id]["label"])
                              for machine_id, _ in grouped})
    all_subjects = {row["subject"] for row in snapshot["rows"]}
    if kind == "pillow":
        targets = [subject for subject in ("python-cpu", "python-simd", "python-gpu", "python-parallel-cpu", "pillow-simd")
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
    excluded = 0
    for index, ((machine_id, workload), rows) in enumerate(grouped.items()):
        machine_label = machines.get(machine_id, {}).get("label", machine_id)
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
            simd_ratio, simd_quality = None, "unavailable"
            if subject in {"python-cpu", "python-simd"} and not is_baseline and "pillow-simd" in by_subject:
                simd_ratio, simd_quality, _ = compare(row, by_subject["pillow-simd"])
            direction = "unavailable" if ratio is None else "faster" if ratio > 1 else "slower" if ratio < 1 else "tie"
            score_direction = direction if quality == "checked" else "unavailable"
            if not is_baseline:
                counts[subject][score_direction] += 1
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
            primary_ratio = ratio if quality == "checked" else None
            pillow_simd_ratio = simd_ratio if simd_quality == "checked" else None
            if ratio is not None and quality != "checked":
                excluded += 1
            if simd_ratio is not None and simd_quality != "checked":
                excluded += 1
            cells[-1] = cells[-1].replace(
                ' data-direction="',
                f' data-ratio-primary="{primary_ratio if primary_ratio is not None else ""}"'
                f' data-observed-ratio-primary="{ratio if ratio is not None else ""}" data-quality-primary="{quality}"'
                f' data-ratio-pillow-simd="{pillow_simd_ratio if pillow_simd_ratio is not None else ""}"'
                f' data-observed-ratio-pillow-simd="{simd_ratio if simd_ratio is not None else ""}"'
                f' data-quality-pillow-simd="{simd_quality}" data-score-direction="{score_direction}" data-direction="',
                1,
            )
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
        runner_cell = f'<td data-sort-value="{escape(machine_label)}">{escape(machine_label)}</td>'
        category_cell = f'<td data-sort-value="{escape(workload_type)}">{escape(workload_type)}</td>'
        mode_cell = f'<td data-sort-value="{escape(mode)}">{escape(mode)}</td>'
        title_cell = f'<th scope="row"><span class="bench-workload-name">{escape(title)}</span><small>{escape(context)}</small></th>'
        status = f'<td class="bench-conclusion"><strong data-fastest>{escape(fastest)}</strong><small data-quality-summary>{escape("; ".join(sorted(quality_labels)) or "Comparison unavailable")}</small><button type="button" class="bench-expand" aria-expanded="false" aria-controls="bench-detail-{index}" hidden>Details</button></td>'
        search = " ".join([workload, title, workload_type, mode, context, machine_label])
        table_rows[table_kind].append(f'<tr class="bench-workload" data-kind="{table_kind}" data-workload="{escape(workload)}" data-machine="{escape(machine_id)}" data-machine-label="{escape(machine_label)}" data-group="{escape(workload_type)}" data-mode="{escape(mode)}" data-name="{escape(title + " " + context)}" data-search="{escape(search)}" data-order="{index}">{title_cell}{runner_cell}{category_cell}{mode_cell}{"".join(cells)}{status}</tr>')
        table_rows[table_kind].append(f'<tr class="bench-expanded" id="bench-detail-{index}" hidden><td colspan="{len(subjects)+5}"><p><strong>{escape(title)}</strong> · Runner: {escape(machine_label)}. Timing boundary: {escape(boundary)}. Sample unit: {escape(rows[0]["sample_unit"])}. Percentiles show spread, not confidence intervals.</p>'
                    '<div class="bench-detail-table"><table><thead><tr><th>Implementation</th><th>Median</th><th>P90</th><th>P95</th><th>Samples</th><th>Recorded result</th></tr></thead><tbody>'
                    + "".join(detail_rows) + '</tbody></table></div>'
                    + f'<p>Workload ID: <code>{escape(workload)}</code></p></td></tr>')
    summary = []
    for subject, count in counts.items():
        baseline_id = BASELINE_FOR.get(subject, primary_baseline)
        baseline_name = NAMES.get(baseline_id, baseline_id)
        summary.append(f'<div class="bench-score" data-score-subject="{escape(subject)}"><strong>{escape(NAMES.get(subject, subject))}</strong>'
                       f'<span><b data-score="faster">{count["faster"]}</b> faster · <b data-score="slower">{count["slower"]}</b> slower</span>'
                       f'<small>vs {escape(baseline_name)} · output-verified pairs only · <span data-score="tie">{count["tie"]}</span> equal · <span data-score="unavailable">{count["unavailable"]}</span> not comparable</small></div>')
    scope = {"pillow": "Python API operations and complete pipelines, including their declared setup and output steps. CPU, SIMD and GPU are separate implementations.",
             "fontdone": "Font loading, metadata, text measurement and glyph rendering. Complete multi-step text-layout pipelines are not measured in this snapshot.",
             "jpeg": "JPEG encode and decode across image sizes, quality and color settings. Other codecs and complete decode–encode pipelines are not measured in this snapshot."}[kind]
    options = ''.join(f'<option value="{escape(group)}">{escape(group)}</option>' for group in groups)
    mode_options = ''.join(f'<option value="{escape(mode)}">{escape(mode)}</option>' for mode in modes)
    machine_filter_options = ''.join(f'<option value="{escape(machine_id)}">{escape(label)}</option>'
                                     for machine_id, label in machine_options)
    subject_options = ''.join(f'<option value="{escape(subject)}">{escape(NAMES.get(subject, subject))}</option>' for subject in targets)
    headers = ''.join(f'<th scope="col" data-subject="{escape(subject)}" data-baseline-for="{escape(BASELINE_FOR.get(subject, subject))}" aria-sort="none"><button type="button" class="bench-sort" data-sort="{escape(subject)}">{escape(NAMES.get(subject, subject))}<span aria-hidden="true"> ↕</span></button><small>{"Baseline" if subject in baseline_ids else "Median · vs " + escape(NAMES.get(BASELINE_FOR.get(subject, primary_baseline), primary_baseline))}</small></th>' for subject in subjects)
    environment = snapshot["environment"]
    host = environment.get("platforms") or environment.get("runner") or environment.get("os") or environment.get("platform") or "Runner not recorded"
    table_head = ('<thead><tr>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="name">Operation / pipeline<span aria-hidden="true"> ↕</span></button></th>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="runner">Runner<span aria-hidden="true"> ↕</span></button></th>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="type">Type<span aria-hidden="true"> ↕</span></button></th>'
            '<th scope="col" aria-sort="none"><button type="button" class="bench-sort" data-sort="mode">Mode<span aria-hidden="true"> ↕</span></button></th>'
            + headers + '<th scope="col">Lowest median<small>Among comparable implementations</small></th></tr></thead>')
    pipeline_count = sum(1 for rows in grouped.values() if facets(rows[0], kind)[0] == "pipelines")
    operation_count = len(grouped) - pipeline_count
    distributions = workload_distributions(grouped, machines, targets, kind)
    boxplot = render_boxplot(distributions)
    intro = (
        "Compare each profile with its workload-matched baseline on the same runner. Parallel CPU is measured separately with the opt-in Rayon feature. Pillow-SIMD appears only for its matched x86 workload cohort. Lower time is better."
        if kind == "pillow" else "Compare only workload pairs with matching inputs and measurement conditions. Lower time is better."
    )
    return (f'<div class="benchmark-dashboard" data-baseline="{escape(primary_baseline)}" data-kind="{escape(kind)}">'
            f'<p class="bench-intro">{escape(intro)}</p>'
            '<section class="bench-overall"><h2>Overall speedup by workload</h2>'
            '<p>Each observation is one workload whose exact output comparison passed. The box shows the middle half of paired speed ratios, the center line is the median, and whiskers reach the furthest observed point within 1.5× the interquartile range. Ratios above 1× mean less time than the named baseline. Geometric means are split between individual operations and complete pipelines so catalog size does not make one dominate the other. Runner cohorts stay separate, and timing-only pairs are excluded.</p>'
            f'<div class="bench-boxplot" aria-label="Filterable per-workload speedup box plot">{boxplot}</div>'
            f'<p class="bench-boxplot-excluded">Timing-only pairs excluded from the plot: <span data-excluded-count>{excluded}</span>. They remain in the tables and are not parity-backed speed claims.</p></section>'
            f'<div class="bench-summary">{"".join(summary)}</div>'
            '<p class="bench-summary-note">The cards count observed per-workload directions. They are not a weighted score or a confidence estimate.</p>'
            '<div class="bench-toolbar" hidden>'
            '<label class="bench-search">Find a workload<input id="evidence-filter" type="search" placeholder="Search operations, pipelines, sizes…" autocomplete="off"></label>'
            f'<label>Type<select id="bench-group"><option value="">All types</option>{options}</select></label>'
            f'<label>Mode<select id="bench-mode"><option value="">All modes</option>{mode_options}</select></label>'
            f'<label>Runner<select id="bench-machine"><option value="">All runners</option>{machine_filter_options}</select></label>'
            f'<label>Compare<select id="bench-subject"><option value="">All implementations</option>{subject_options}</select></label>'
            '<button type="button" id="bench-reset">Reset</button><output id="bench-count" aria-live="polite"></output></div>'
            '<p class="bench-chart-key">Shorter bars = less time within a row. Sort by runner, type, mode, operation or pipeline, or backend latency. Scroll each table horizontally on small screens.</p>'
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

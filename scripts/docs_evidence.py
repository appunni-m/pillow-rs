#!/usr/bin/env python3
"""Project public views of benchmark observations without changing their meaning.

The snapshot is a presentation format, not a parity/coverage receipt. It retains
the original report hash, revision, measurement policy, and every measured or
failed subject. Hostnames, local paths, and verbose implementation traces are
not copied into the public view. Missing values stay null.
"""
from __future__ import annotations

import argparse
from collections import Counter
import csv
import hashlib
import html
import json
import math
from pathlib import Path
import re
import statistics

SCHEMA = "public-docs/benchmark-snapshot@1"
SHA = re.compile(r"^[0-9a-f]{40}$")


def number(value: object, scale: float = 1.0) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError("benchmark measurements must be numeric or null")
    result = float(value) * scale
    if not math.isfinite(result) or result < 0:
        raise ValueError("benchmark measurements must be finite and nonnegative")
    return result


def project_pillow(document: dict) -> dict:
    if document.get("schema") != "migration-parity/benchmark-result@1":
        raise ValueError("expected the maintained Pillow benchmark result schema")
    identity = document["identity"]
    targets = identity["targets"]
    revisions = {t["revision"] for t in targets}
    if len(revisions) != 1:
        raise ValueError("benchmark subjects do not share one source revision")
    rows = []
    for workload in document["workloads"]:
        policy = workload["measurement_policy"]
        correctness = workload["correctness"]
        for subject in workload["subjects"]:
            latency = next((m for m in subject.get("measurements", []) if m["metric"] == "latency"), None)
            if latency and latency["unit"] != "millisecond":
                raise ValueError("unknown benchmark latency unit")
            stats = latency["statistics"] if latency else {}
            execution = subject.get("execution", {})
            rows.append({
                "workload": workload["workload_id"], "subject": subject["id"],
                "status": subject["status"], "median_us": number(stats.get("median"), 1000),
                "p90_us": None, "p95_us": number(stats.get("p95"), 1000),
                "sample_count": latency.get("sample_count") if latency else None,
                "sample_unit": "timed workflow execution", "policy": policy,
                "context": workload.get("context", {}),
                "correctness": f"{correctness['gate']}: {correctness['outcome']}",
                "requested_backend": execution.get("requested_backend"),
                "actual_backend": execution.get("actual_backend"),
                "terminal_complete": execution.get("terminal_complete"),
                "fallback_reasons": execution.get("fallback_reason_counts", {}),
            })
    env = document["environment"]
    return {
        "revision": next(iter(revisions)), "measured_at": identity["finished_at"],
        "clean": all(t.get("dirty") is False for t in targets),
        "status": document["status"], "run_id": identity["run_id"],
        "environment": {k: env.get(k) for k in ("os", "architecture", "cpu", "toolchain", "power_mode")},
        "policy_status": "Timing budget acceptance is separate; no budget pass is inferred from this result.",
        "notes": ["Whole-workflow timings. successful_execution is an execution gate, not a pixel-parity claim.",
                  "Requested and actual backends are separate. Host controls and missing terminal evidence do not prove native GPU performance."],
        "rows": rows,
    }


def project_fontdone(document: dict) -> dict:
    # Committed historical ledger and fresh reports share the retained summary
    # fields only after the existing benchmark's own summary extractor runs.
    if "performance" in document and "clean_runs" in document["performance"]:
        runs = document["performance"]["clean_runs"]
        if not runs:
            raise ValueError("fontdone has no recorded clean benchmark")
        run = max(runs, key=lambda r: r["measured_utc"])
    else:
        run = document
    rows = []
    for operation in run["per_operation"]:
        for key, name in (("rust", "fontdone"), ("c", "FreeType")):
            rows.append({
                "workload": operation["id"], "subject": name, "status": "completed",
                "median_us": number(operation.get(f"{key}_ns_per_iter_median"), .001),
                "p90_us": number(operation.get(f"{key}_ns_per_iter_p90"), .001), "p95_us": None,
                "sample_count": run["sample_count"], "sample_unit": "benchmark sample",
                "policy": {"profile": run["workload_profile"], "boundary": "public operation; process memory is separate"},
                "context": {}, "correctness": operation["comparison_trust"],
                "requested_backend": "native", "actual_backend": "native", "terminal_complete": None,
                "fallback_reasons": {},
            })
    return {
        "revision": run["source_commit"], "measured_at": run["measured_utc"],
        "clean": run["clean_source"], "status": "completed", "run_id": run["report_sha256"],
        "environment": run["environment"], "policy_status": run["regression"]["status"],
        "notes": ["timing_only rows do not establish output parity. Values apply only to this operation matrix and host.",
                  "No reviewed regression thresholds are active while the policy is collecting_baseline."],
        "memory": run.get("peak_process_memory"), "binary_size": run.get("binary_size"), "rows": rows,
    }


def project_jpeg(directory: Path) -> dict:
    metadata = json.loads((directory / "metadata.json").read_text())
    if metadata.get("schema") != 1:
        raise ValueError("unknown JPEG benchmark metadata schema")
    rounds, count = metadata["rounds"], metadata["case_count"]
    if type(rounds) is not int or rounds < 3 or rounds % 2 == 0 or type(count) is not int or count < 1:
        raise ValueError("invalid JPEG measurement matrix dimensions")
    with (directory / "summary.csv").open(newline="") as stream:
        summaries = list(csv.DictReader(stream))
    keys = {(row["operation"], row["case"]) for row in summaries}
    cases = {row["case"] for row in summaries}
    if len(cases) != count or len(summaries) != 2 * count or keys != {(op, case) for op in ("encode", "decode") for case in cases}:
        raise ValueError("incomplete or duplicate JPEG summary matrix")
    records = [json.loads(line) for line in (directory / "raw.jsonl").read_text().splitlines() if line]
    expected = {(op, case, impl, round_) for op, case in keys
                for impl in ("image-slash-star", "libjpeg-turbo") for round_ in range(1, rounds + 1)}
    actual = {(r["operation"], r["case"], r["implementation"], r["round"]) for r in records}
    if actual != expected or len(records) != len(expected):
        raise ValueError("incomplete or duplicate JPEG raw observations")
    for row in summaries:
        if int(row["rounds"]) != rounds:
            raise ValueError("JPEG summary round count differs from metadata")
        for key, impl in (("rust", "image-slash-star"), ("turbo", "libjpeg-turbo")):
            reports = [r["report"] for r in records if (r["operation"], r["case"], r["implementation"]) == (row["operation"], row["case"], impl)]
            if any(int(r["iterations"]) != int(row["iterations_per_round"]) for r in reports):
                raise ValueError("JPEG raw iteration count differs from summary")
            median = statistics.median(int(r["median_ns"]) for r in reports)
            if median != int(row[f"{key}_median_ns"]):
                raise ValueError("JPEG summary median differs from raw observations")
    rows = []
    for row in summaries:
        for key, name in (("rust", "image-slash-star"), ("turbo", "libjpeg-turbo")):
            rows.append({
                "workload": f"{row['operation']}.{row['case']}", "subject": name, "status": "completed",
                "median_us": number(int(row[f"{key}_median_ns"]), .001), "p90_us": None, "p95_us": None,
                "sample_count": int(row["rounds"]), "sample_unit": "round median",
                "policy": {"boundary": "public codec operation including context/output lifetime",
                           "iterations_per_round": int(row["iterations_per_round"]), "aggregation": "median of round medians"},
                "context": {k: row[k] for k in ("width", "height", "mode", "quality", "subsampling", "progressive", "optimize", "restart_rows")},
                "correctness": f"output hash match: {row['output_hash_match']}; byte length match: {row['output_bytes_match']}",
                "requested_backend": "native", "actual_backend": "native", "terminal_complete": None,
                "fallback_reasons": {},
            })
    return {
        "revision": metadata["git_head"], "measured_at": metadata["timestamp_utc"],
        "clean": not bool(metadata["git_status"]), "status": "completed", "run_id": metadata["timestamp_utc"],
        "environment": {k: metadata.get(k) for k in ("platform", "machine", "rustc", "cc", "python")},
        "policy_status": "Observational codec comparison; no cross-machine regression budget.",
        "notes": ["JPEG only. This does not measure other image codecs or general image processing.",
                  "CMYK uses different public sample conventions in Pillow and TurboJPEG; unequal output hashes are not automatically codec errors.",
                  "Spread is not present in the summary CSV. It is unavailable here rather than inferred from the medians."],
        "rows": rows,
    }


def validate(snapshot: dict, repository: str) -> None:
    if snapshot.get("schema") != SCHEMA or snapshot.get("repository") != repository:
        raise ValueError("benchmark snapshot schema/repository mismatch")
    if not SHA.fullmatch(snapshot.get("revision", "")):
        raise ValueError("benchmark source revision is missing")
    if not snapshot.get("measured_at") or not snapshot.get("environment"):
        raise ValueError("benchmark measurement identity is incomplete")
    if not re.fullmatch(r"[0-9a-f]{64}", snapshot.get("source_sha256", "")):
        raise ValueError("benchmark source hash is missing")
    keys = set()
    for row in snapshot["rows"]:
        key = (row["workload"], row["subject"])
        if key in keys:
            raise ValueError(f"duplicate benchmark subject: {key}")
        keys.add(key)
        for metric in ("median_us", "p90_us", "p95_us"):
            number(row[metric])
        count = row["sample_count"]
        if count is not None and (isinstance(count, bool) or not isinstance(count, int) or count <= 0):
            raise ValueError("benchmark sample count must be positive or unavailable")
        if not row["status"] or not row["correctness"] or not row["policy"]:
            raise ValueError("benchmark row omitted its result or measurement boundary")
    if not keys:
        raise ValueError("an empty result is not a measured benchmark")


def cell(value: object) -> str:
    if value is None:
        return "Not measured"
    if isinstance(value, float):
        return f"{value:,.3f}"
    return html.escape(str(value)).replace("|", "&#124;").replace("\n", " ")


def controls(label: str) -> str:
    return f'<div class="evidence-controls"><label for="evidence-filter">{label}</label><input id="evidence-filter" type="search" autocomplete="off"><output aria-live="polite"></output></div>\n'


def render_benchmarks(root: Path, config: dict, output: Path) -> str:
    source = root / config["benchmark"]["source"]
    if not source.exists():
        (output / "benchmark-details.md").write_text("# Benchmark measurement details\n\nNo measurement is available. [Contributor benchmark guide](benchmarking.md).\n")
        return ("# Benchmark results\n\nNo benchmark snapshot has been published for this project yet. "
                "This is **unmeasured**, not a zero-duration result or a performance claim.\n\n"
                "See the [measurement protocol](benchmarking.md) for the maintained command and CI artifact.\n")
    snapshot = json.loads(source.read_text())
    validate(snapshot, config["repository"])
    (output / "assets" / "benchmark.json").write_text(json.dumps(snapshot, indent=2) + "\n")
    historical = snapshot["revision"] != config["release_revision"]
    status = ("**Diagnostic measurement from a modified checkout.** Not a release baseline."
              if not snapshot["clean"] else "**Historical measurement.** Not a measurement of the latest release."
              if historical else "**Measurement of the documented release.**")
    lines = ["# Benchmark results", "", status,
             f"Recorded {cell(snapshot['measured_at'][:10])}. Run status: **{cell(snapshot['status'])}**.", ""]
    details = ["# Benchmark measurement details", "", status, "",
             "[View the results](benchmarks.md) · [Run benchmarks](benchmarking.md)", "",
             "| Measurement identity | Value |", "| --- | --- |",
             f"| Source revision | `{snapshot['revision']}` |", f"| Measured at | {cell(snapshot['measured_at'])} |",
             f"| Result status | {cell(snapshot['status'])} |", f"| Source document SHA-256 | `{snapshot['source_sha256']}` |",
             f"| Policy | {cell(snapshot['policy_status'])} |"]
    for key, value in snapshot["environment"].items():
        details.append(f"| {cell(key)} | {cell(value)} |")
    details += ["", "Download the [public measurement data](assets/benchmark.json). This is a presentation snapshot, "
              "not the full source receipt. It preserves the original report hash and numerical observations; "
              "hostnames, local paths, and internal traces are omitted.", ""]
    details += [f"- {cell(note)}" for note in snapshot["notes"]]
    lines += ["Latency is in **microseconds (µs)**; lower is faster. Compare implementations "
              "within the same workload. P95 is the 95th percentile; sample counts show how many timings were collected.", "",
              "[Hardware, caveats, and full measurements](benchmark-details.md). "
              "A completed timing run does not by itself establish equal output.", "", controls("Filter results"),
              "| Workload | Implementation | Median µs | P95 µs | Samples | Result |",
              "| --- | --- | ---: | ---: | ---: | --- |"]
    details += ["", "## Full measurements", "",
                "| Workload | Subject | Median µs | P90 µs | P95 µs | Samples | Result / correctness | Requested → actual | Terminal receipt |",
                "| --- | --- | ---: | ---: | ---: | ---: | --- | --- | --- |"]
    for row in snapshot["rows"]:
        route = f"{row['requested_backend']} → {row['actual_backend']}"
        result = f"{row['status']}; {row['correctness']}"
        terminal = "Complete" if row["terminal_complete"] is True else "Not proven" if row["terminal_complete"] is False else "Not measured" if row["requested_backend"] == "gpu" else "Not applicable"
        details.append("| " + " | ".join(cell(v) for v in (row["workload"], row["subject"], row["median_us"], row["p90_us"], row["p95_us"], row["sample_count"], result, route, terminal)) + " |")
        correctness = row["correctness"]
        outcome = ("Timing only" if correctness in {"timing_only", "successful_execution: pass"}
                   else "Output matches" if correctness == "output hash match: True; byte length match: True"
                   else "Output differs" if correctness.startswith("output hash match: False;") else correctness)
        if row["status"] not in {"completed", "passed", "ok"}:
            outcome = f"{row['status']}; {outcome}"
        implementation = row["subject"]
        if row["requested_backend"] != row["actual_backend"]:
            implementation += f" ({route})"
        if row["requested_backend"] == "gpu" and row["terminal_complete"] is not True:
            outcome += "; GPU completion unproven"
        lines.append("| " + " | ".join(cell(v) for v in (row["workload"], implementation, row["median_us"],
                     row["p95_us"], row["sample_count"], outcome)) + " |")
    lines += ["", "[Measurement details and hardware](benchmark-details.md) · "
              "[Download results](assets/benchmark.json) · [Contributor benchmark guide](benchmarking.md)", ""]
    details += ["", "## Workload boundaries", "", "Repeat counts, dimensions, modes, and cache states are retained per workload:", ""]
    seen = set()
    for row in snapshot["rows"]:
        if row["workload"] in seen:
            continue
        seen.add(row["workload"])
        details += [f"### {cell(row['workload'])}", "", f"Sample unit: {cell(row['sample_unit'])}.", "", "```json",
                  json.dumps({"context": row["context"], "measurement_policy": row["policy"]}, indent=2), "```", ""]
    details += ["## Reproduce and interpret", "", "Use the [benchmark protocol](benchmarking.md). "
              "Correctness, native dispatch, timing regressions, process memory, and artifact size are separate claims.", ""]
    (output / "benchmark-details.md").write_text("\n".join(details))
    return "\n".join(lines)


def render_support(root: Path, config: dict, output: Path) -> str:
    import yaml

    source = root / config["support"]["source"]
    document = yaml.safe_load(source.read_text())
    if config["support"]["kind"] == "pillow":
        counts: Counter[tuple[str, str]] = Counter()
        paths = {}
        for relative in document["input_index"]["parity"]:
            for case in json.loads((source.parent / relative).read_text())["cases"]:
                key = (case["surface"], case["operation"])
                counts[key] += 1
                paths[key] = f"https://github.com/{config['repository']}/blob/main/{source.parent.relative_to(root)}/{relative}"
        lines = ["# API support inventory", "", "Generated from the public manifest and indexed input files. "
                 "A declared status and an input count do not prove all argument combinations. "
                 "[Maturity and release evidence](compatibility.md) identify the executed scope.", "",
                 f"Manifest SHA-256: `{hashlib.sha256(source.read_bytes()).hexdigest()}`.", ""]
        for surface in document["surfaces"]:
            lines += [f"## {surface['id']}", "", "| Public path | Declared support | Input cases | Contract |", "| --- | --- | ---: | --- |"]
            for operation in surface["operations"]:
                key = (surface["id"], operation["id"])
                path = operation["source"]["path"]
                status = ", ".join(t["support"]["status"] for t in operation["targets"])
                witness = f"[{counts[key]}]({paths[key]})" if key in paths else "0 — unmeasured"
                signature = cell(operation["source"]["signature"])
                lines.append(f"| `{path}` | {status} | {witness} | `{signature}` |")
        return "\n".join(lines) + "\n"
    raise ValueError("unknown generated support inventory")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=("pillow", "fontdone", "jpeg"))
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--repository", required=True)
    args = parser.parse_args()
    if args.kind == "jpeg":
        snapshot = project_jpeg(args.source)
        raw = b"".join((args.source / name).read_bytes() for name in ("metadata.json", "summary.csv", "raw.jsonl"))
    else:
        raw = args.source.read_bytes()
        document = json.loads(raw)
        if args.kind == "fontdone" and "metadata" in document:
            from bench_freetype import DEFAULT_MATRIX, compact_performance_baseline, load_matrix, sha256_file
            document = compact_performance_baseline(document, hashlib.sha256(raw).hexdigest(),
                                                    load_matrix(DEFAULT_MATRIX), sha256_file(DEFAULT_MATRIX))
        snapshot = (project_pillow if args.kind == "pillow" else project_fontdone)(document)
    snapshot.update(schema=SCHEMA, repository=args.repository, source_sha256=hashlib.sha256(raw).hexdigest())
    validate(snapshot, args.repository)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n")
    print(f"Published-view snapshot: {len(snapshot['rows'])} rows; source {snapshot['revision']}")


if __name__ == "__main__":
    main()

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
import os
from pathlib import Path
import re
import statistics

SCHEMA = "public-docs/benchmark-snapshot@2"
LEGACY_SCHEMA = "public-docs/benchmark-snapshot@1"
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


def attach_machine_identity(snapshot: dict, runner_id: str | None = None,
                            runner_label: str | None = None,
                            cpu_model: str | None = None,
                            image_version: str | None = None,
                            rust_toolchain: str | None = None) -> dict:
    """Label one hosted-runner cohort without publishing its ephemeral hostname."""
    environment = snapshot.get("environment", {})
    architecture = str(environment.get("architecture") or "unknown")
    os_name = str(environment.get("os") or environment.get("platform") or "unknown")
    machine_id = runner_id or f"local-{os_name.lower()}-{architecture.lower()}"
    machine_id = re.sub(r"[^a-zA-Z0-9._-]+", "-", machine_id).strip("-").lower()
    label = runner_label or f"Unpinned runner · {os_name} · {architecture}"
    machine = {
        "id": machine_id,
        "label": label,
        "os": os_name,
        "architecture": architecture,
        "cpu": cpu_model or environment.get("cpu") or "not recorded",
        "python_version": environment.get("python_version") or environment.get("toolchain") or "not recorded",
        "rust_toolchain": rust_toolchain or environment.get("rust_toolchain") or "not recorded",
        "image_version": image_version or "not recorded",
    }
    snapshot["machine"] = machine
    snapshot["environment"] = {**environment, "runner": label, "cpu_model": machine["cpu"],
                                "python_version": machine["python_version"],
                                "rust_toolchain": machine["rust_toolchain"],
                                "image_version": machine["image_version"]}
    for row in snapshot.get("rows", []):
        row["machine_id"] = machine_id
    return snapshot


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
                "comparison_group": "default",
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
    snapshot = {
        "revision": next(iter(revisions)), "measured_at": identity["finished_at"],
        "clean": all(t.get("dirty") is False for t in targets),
        "status": document["status"], "run_id": identity["run_id"],
        "environment": {"os": env.get("os"), "architecture": env.get("architecture"), "cpu": env.get("cpu"),
                        "python_version": env.get("toolchain"), "power_mode": env.get("power_mode"),
                        "rust_toolchain": os.environ.get("DOCS_BENCHMARK_RUST_TOOLCHAIN")},
        "policy_status": "Timing budget acceptance is separate; no budget pass is inferred from this result.",
        "notes": ["Whole-workflow timings. successful_execution is an execution gate, not a pixel-parity claim.",
                  "Requested and actual backends are separate. Host controls and missing terminal evidence do not prove native GPU performance."],
        "rows": rows,
    }
    return attach_machine_identity(
        snapshot,
        os.environ.get("DOCS_BENCHMARK_RUNNER_ID"),
        os.environ.get("DOCS_BENCHMARK_RUNNER_LABEL"),
        os.environ.get("DOCS_BENCHMARK_CPU_MODEL"),
        os.environ.get("DOCS_BENCHMARK_IMAGE_VERSION"),
        os.environ.get("DOCS_BENCHMARK_RUST_TOOLCHAIN"),
    )


def normalize_parallel_cpu_snapshot(snapshot: dict) -> dict:
    """Drop a legacy second Pillow timing and use the ordinary Pillow row.

    A previously published snapshot may still contain the old ``parallel-cpu``
    comparison cohort. Normalize it at the docs boundary so an in-flight or
    latest-successful artifact remains renderable during benchmark workflow
    upgrades. New benchmark exports already contain target-only Parallel CPU
    rows in the default comparison group.
    """

    rows = snapshot.get("rows", [])
    if not any(
        row.get("comparison_group", "default") == "parallel-cpu"
        or row.get("subject") == "pillow-parallel-cpu"
        for row in rows
    ):
        return snapshot

    default_rows: list[dict] = []
    parallel_by_workload: dict[str, dict[str, dict]] = {}
    for row in rows:
        cohort = row.get("comparison_group", "default")
        if cohort == "parallel-cpu":
            if row["subject"] not in {"pillow-parallel-cpu", "python-parallel-cpu"}:
                raise ValueError("legacy Parallel CPU snapshot contains an unrelated subject")
            subjects = parallel_by_workload.setdefault(row["workload"], {})
            if row["subject"] in subjects:
                raise ValueError(f"duplicate legacy Parallel CPU subject for {row['workload']}")
            subjects[row["subject"]] = row
        elif row["subject"] == "pillow-parallel-cpu":
            raise ValueError("legacy Parallel CPU Pillow row has no Parallel CPU cohort")
        else:
            default_rows.append(row)

    ordinary_by_workload: dict[str, dict[str, dict]] = {}
    for row in default_rows:
        ordinary_by_workload.setdefault(row["workload"], {})[row["subject"]] = row

    targets = []
    for workload, subjects in parallel_by_workload.items():
        if set(subjects) != {"pillow-parallel-cpu", "python-parallel-cpu"}:
            raise ValueError(f"legacy Parallel CPU workload lacks its paired baseline: {workload}")
        ordinary_pillow = ordinary_by_workload.get(workload, {}).get("pillow")
        if ordinary_pillow is None:
            raise ValueError(f"legacy Parallel CPU workload lacks ordinary Pillow timing: {workload}")
        target = subjects["python-parallel-cpu"]
        for field in ("policy", "context", "sample_unit", "sample_count"):
            if ordinary_pillow[field] != target[field]:
                raise ValueError(
                    f"legacy Parallel CPU and ordinary Pillow conditions differ for {workload}: {field}"
                )
        targets.append(dict(target, comparison_group="default"))

    normalized = dict(snapshot)
    normalized["rows"] = [*default_rows, *targets]
    normalized["notes"] = [
        *snapshot.get("notes", []),
        "Legacy snapshot normalized: Parallel CPU is compared with ordinary Pillow; the duplicate Parallel CPU Pillow timing is omitted.",
    ]
    return normalized


def merge_parallel_cpu(snapshot: dict, parallel: dict, source_hash: str, parallel_hash: str) -> dict:
    """Join target-only Rayon timings to the ordinary Pillow measurements."""
    if snapshot["revision"] != parallel["revision"]:
        raise ValueError("default and Parallel CPU benchmarks use different source revisions")
    if snapshot["environment"] != parallel["environment"]:
        raise ValueError("default and Parallel CPU benchmarks use different environments")

    expected_subjects = {"python-parallel-cpu"}
    by_workload: dict[str, dict[str, dict]] = {}
    for row in parallel["rows"]:
        if row["subject"] not in expected_subjects:
            raise ValueError(f"unexpected Parallel CPU subject: {row['subject']}")
        subjects = by_workload.setdefault(row["workload"], {})
        if row["subject"] in subjects:
            raise ValueError(f"duplicate Parallel CPU subject for {row['workload']}")
        subjects[row["subject"]] = row

    default_by_workload: dict[str, dict[str, dict]] = {}
    for row in snapshot["rows"]:
        default_by_workload.setdefault(row["workload"], {})[row["subject"]] = row
    merged = []
    for workload, subjects in by_workload.items():
        if set(subjects) != expected_subjects:
            raise ValueError(f"Parallel CPU workload must contain only its target timing: {workload}")
        if workload not in default_by_workload:
            raise ValueError(f"Parallel CPU workload is absent from the default benchmark: {workload}")
        default_subjects = default_by_workload[workload]
        pillow = default_subjects.get("pillow")
        if pillow is None:
            raise ValueError(f"Parallel CPU workload lacks ordinary Pillow timing: {workload}")
        default_cpu = default_subjects.get("python-cpu")
        if default_cpu is None:
            raise ValueError(f"Parallel CPU workload has no matching default CPU lane: {workload}")
        parallel_cpu = subjects["python-parallel-cpu"]
        for field in ("policy", "context", "sample_unit", "sample_count"):
            if pillow[field] != parallel_cpu[field]:
                raise ValueError(f"Parallel CPU and Pillow conditions differ for {workload}: {field}")
        if any(default_cpu[field] != pillow[field]
               for field in ("policy", "context", "sample_unit", "sample_count")):
            raise ValueError(f"Parallel CPU and default CPU conditions differ for {workload}")
        target = dict(parallel_cpu, comparison_group="default")
        merged.append(target)

    for row in snapshot["rows"]:
        row.setdefault("comparison_group", "default")
    snapshot["rows"].extend(merged)
    snapshot["cohorts"] = [
        {"id": "default", "machine_id": snapshot.get("machine", {}).get("id"),
         "run_id": snapshot["run_id"], "measured_at": snapshot["measured_at"],
         "source_sha256": source_hash},
        {"id": "parallel-cpu", "machine_id": snapshot.get("machine", {}).get("id"),
         "run_id": parallel["run_id"], "measured_at": parallel["measured_at"],
         "source_sha256": parallel_hash},
    ]
    snapshot["notes"].append(
        "Parallel CPU is an opt-in Rayon build measured separately. Its parity preflight uses Pillow, while its speed ratios reuse ordinary Pillow timings from the default run; no second Pillow timing is collected."
    )
    return snapshot


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
    if snapshot.get("schema") not in {SCHEMA, LEGACY_SCHEMA} or snapshot.get("repository") != repository:
        raise ValueError("benchmark snapshot schema/repository mismatch")
    if not SHA.fullmatch(snapshot.get("revision", "")):
        raise ValueError("benchmark source revision is missing")
    if not snapshot.get("measured_at") or not snapshot.get("environment"):
        raise ValueError("benchmark measurement identity is incomplete")
    if not re.fullmatch(r"[0-9a-f]{64}", snapshot.get("source_sha256", "")):
        raise ValueError("benchmark source hash is missing")
    machines = snapshot.get("machines") or ([snapshot["machine"]] if snapshot.get("machine") else [])
    machine_ids = {machine.get("id") for machine in machines}
    if len(machine_ids) != len(machines) or any(not isinstance(item, str) or not item for item in machine_ids):
        raise ValueError("benchmark runner identities are missing or duplicated")
    if snapshot.get("schema") == SCHEMA and not machine_ids:
        raise ValueError("machine-aware benchmark snapshot omitted runner identity")
    keys = set()
    rows_by_workload: dict[tuple[str, str], dict[str, dict]] = {}
    for row in snapshot["rows"]:
        cohort = row.get("comparison_group", "default")
        if cohort != "default":
            raise ValueError("unknown benchmark comparison cohort")
        if snapshot.get("schema") == SCHEMA:
            machine_id = row.get("machine_id")
            if not machine_id:
                raise ValueError("machine-aware benchmark row omitted its runner identity")
        else:
            machine_id = row.get("machine_id", next(iter(machine_ids), "legacy"))
        if machine_ids and machine_id not in machine_ids:
            raise ValueError(f"benchmark row references unknown runner: {machine_id}")
        rows_by_workload.setdefault((machine_id, row["workload"]), {})[row["subject"]] = row
        key = (machine_id, row["workload"], row["subject"])
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
    for (_, workload), subjects in rows_by_workload.items():
        parallel = subjects.get("python-parallel-cpu")
        if parallel is None:
            continue
        pillow = subjects.get("pillow")
        if pillow is None:
            raise ValueError(f"Parallel CPU row lacks ordinary Pillow baseline: {workload}")
        for field in ("policy", "context", "sample_unit", "sample_count"):
            if pillow[field] != parallel[field]:
                raise ValueError(f"Parallel CPU and Pillow conditions differ for {workload}: {field}")


def merge_pillow_snapshots(source: Path, repository: str, output: Path | None = None) -> dict:
    """Join one public benchmark snapshot per runner into a provenance-preserving view."""
    files = sorted(source.rglob("snapshot.json")) if source.is_dir() else [source]
    if not files:
        raise ValueError("no public benchmark snapshots were downloaded")
    snapshots = [json.loads(path.read_text(encoding="utf-8")) for path in files]
    for snapshot in snapshots:
        validate(snapshot, repository)
    revisions = {snapshot["revision"] for snapshot in snapshots}
    if len(revisions) != 1:
        raise ValueError("runner snapshots do not share one source revision")
    machines: list[dict] = []
    rows: list[dict] = []
    source_hashes = []
    run_ids = []
    cohorts = []
    notes = []
    for snapshot in snapshots:
        children = snapshot.get("machines") or ([snapshot["machine"]] if snapshot.get("machine") else [])
        if not children:
            raise ValueError("runner snapshot has no machine identity")
        for machine in children:
            if machine["id"] in {item["id"] for item in machines}:
                raise ValueError(f"duplicate runner snapshot: {machine['id']}")
            machines.append(machine)
        child_ids = {machine["id"] for machine in children}
        for row in snapshot["rows"]:
            machine_id = row.get("machine_id") or (next(iter(child_ids)) if len(child_ids) == 1 else None)
            if machine_id not in child_ids:
                raise ValueError("benchmark row has no unambiguous runner identity")
            rows.append({**row, "machine_id": machine_id})
        source_hashes.append(snapshot["source_sha256"])
        run_ids.append(str(snapshot["run_id"]))
        for cohort in snapshot.get("cohorts", []):
            default_machine = next(iter(child_ids)) if len(child_ids) == 1 else None
            cohorts.append({**cohort, "machine_id": cohort.get("machine_id") or default_machine})
        notes.extend(snapshot.get("notes", []))
    combined = {
        "schema": SCHEMA,
        "repository": repository,
        "revision": next(iter(revisions)),
        "measured_at": max(snapshot["measured_at"] for snapshot in snapshots),
        "clean": all(snapshot["clean"] for snapshot in snapshots),
        "status": "completed" if all(snapshot["status"] == "completed" for snapshot in snapshots) else "partial",
        "run_id": "+".join(run_ids),
        "environment": {"runner_count": len(machines),
                        "platforms": "; ".join(machine["label"] for machine in machines)},
        "machines": machines,
        "cohorts": cohorts,
        "policy_status": "Paired workload comparisons are summarized within each runner cohort; runner cohorts are not compared against one another.",
        "notes": list(dict.fromkeys(notes + [
            "Each runner cohort is a separate GitHub-hosted measurement. Ratios are formed only between matching rows from that same cohort; runner cohorts are not compared directly.",
            "The overall box plot gives each parity-verified workload one observation. It shows workload spread, not a confidence interval or a universal speed guarantee.",
        ])),
        "rows": rows,
        "source_sha256": hashlib.sha256("\n".join(sorted(source_hashes)).encode()).hexdigest(),
    }
    validate(combined, repository)
    if output is not None:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(combined, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return combined


def merge_pillow_simd_cohort(snapshot: dict, simd_snapshot: dict) -> dict:
    """Add the separately version-matched x86 cohort as its own runner cohort."""
    if snapshot["revision"] != simd_snapshot["revision"]:
        raise ValueError("Pillow-SIMD and main benchmark snapshots use different source revisions")
    source_machines = simd_snapshot.get("machines") or ([simd_snapshot["machine"]] if simd_snapshot.get("machine") else [])
    if len(source_machines) != 1:
        raise ValueError("Pillow-SIMD evidence must identify exactly one runner cohort")
    source_machine = source_machines[0]
    main_machines = snapshot.get("machines") or ([snapshot["machine"]] if snapshot.get("machine") else [])
    comparable_machine = next((machine for machine in main_machines
                               if machine["id"] == source_machine["id"]), None)
    if comparable_machine is None:
        raise ValueError("Pillow-SIMD and main benchmark snapshots use different runner classes")
    missing_identity = {"not recorded", "not-recorded", "unknown", "unknown-unknown"}
    for key in ("os", "architecture", "cpu", "python_version", "rust_toolchain", "image_version"):
        left, right = comparable_machine.get(key), source_machine.get(key)
        if (
            not left
            or not right
            or str(left).strip().lower() in missing_identity
            or str(right).strip().lower() in missing_identity
        ):
            raise ValueError(f"Pillow-SIMD and main benchmark runner {key} identity is incomplete")
        if left != right:
            raise ValueError(f"Pillow-SIMD and main benchmark runner {key} differs")
    machine = dict(source_machine)
    machine["id"] = re.sub(r"[^a-zA-Z0-9._-]+", "-", f"{machine['id']}-pillow-simd-paired").strip("-").lower()
    machine["label"] = f"{machine['label']} · Pillow-SIMD paired cohort"
    machines = snapshot.get("machines") or ([snapshot["machine"]] if snapshot.get("machine") else [])
    if machine["id"] in {item["id"] for item in machines}:
        raise ValueError("Pillow-SIMD runner cohort identity is duplicated")
    combined = dict(snapshot)
    combined.pop("machine", None)
    combined["schema"] = SCHEMA
    combined["machines"] = [*machines, machine]
    combined["cohorts"] = [
        *snapshot.get("cohorts", []),
        *[{**cohort, "machine_id": machine["id"]} for cohort in simd_snapshot.get("cohorts", [])],
    ]
    combined["rows"] = [
        *snapshot["rows"],
        *[{**row, "machine_id": machine["id"]} for row in simd_snapshot["rows"]],
    ]
    combined["measured_at"] = max(snapshot["measured_at"], simd_snapshot["measured_at"])
    combined["run_id"] = f"{snapshot['run_id']}+{simd_snapshot['run_id']}"
    combined["source_sha256"] = hashlib.sha256(
        f"{snapshot['source_sha256']}\0{simd_snapshot['source_sha256']}".encode()
    ).hexdigest()
    combined["environment"] = {
        "runner_count": len(combined["machines"]),
        "platforms": "; ".join(item["label"] for item in combined["machines"]),
    }
    combined["notes"] = list(dict.fromkeys([
        *snapshot.get("notes", []),
        *simd_snapshot.get("notes", []),
        "Pillow-SIMD rows are paired with ordinary Pillow and pillow-rs CPU/SIMD from one Linux x86 benchmark run; that narrow cohort is kept separate from the full Linux and macOS workloads.",
    ]))
    validate(combined, snapshot["repository"])
    return combined


def cell(value: object) -> str:
    if value is None:
        return "Not measured"
    if isinstance(value, float):
        return f"{value:,.3f}"
    return html.escape(str(value)).replace("|", "&#124;").replace("\n", " ")


def controls(label: str) -> str:
    return f'<div class="evidence-controls"><label for="evidence-filter">{label}</label><input id="evidence-filter" type="search" autocomplete="off"><output aria-live="polite"></output></div>\n'


def contract_code(path: str, signature: str) -> str:
    # Markdown code spans escape entities again. Use literal HTML code, escaping
    # once for HTML and protecting syntax from Markdown's inline/table parser.
    content = html.escape(signature)
    for character in "\\`*_[]|~\n\r":
        content = content.replace(character, f"&#{ord(character)};")
    return f'<pre class="api-contract" data-api-path="{html.escape(path, quote=True)}"><code>{content}</code></pre>'


def render_benchmarks(root: Path, config: dict, output: Path) -> str:
    source = root / config["benchmark"]["source"]
    if not source.exists():
        (output / "benchmark-details.md").write_text("# Benchmark measurement details\n\nNo measurement is available. [Contributor benchmark guide](benchmarking.md).\n")
        return ("# Benchmark results\n\nNo benchmark snapshot has been published for this project yet. "
                "This is **unmeasured**, not a zero-duration result or a performance claim.\n\n"
                "See the [measurement protocol](benchmarking.md) for the maintained command and CI artifact.\n")
    snapshot = json.loads(source.read_text())
    snapshot = normalize_parallel_cpu_snapshot(snapshot)
    simd_source = config.get("pillow_simd_benchmark", {}).get("snapshot")
    simd_note = None
    if simd_source:
        simd_snapshot = json.loads((root / simd_source).read_text(encoding="utf-8"))
        validate(simd_snapshot, config["repository"])
        if simd_snapshot["revision"] != snapshot["revision"]:
            simd_note = "The latest Pillow-SIMD run used a different source revision; it remains on its separate comparison page and is excluded from this overall summary."
        else:
            try:
                snapshot = merge_pillow_simd_cohort(snapshot, simd_snapshot)
            except ValueError as error:
                simd_note = (f"The latest Pillow-SIMD run is excluded from this overall summary because it is not a matched x86 runner cohort ({error}); "
                             "its measurements remain on the separate comparison page.")
    validate(snapshot, config["repository"])
    (output / "assets" / "benchmark.json").write_text(json.dumps(snapshot, indent=2) + "\n")
    historical = snapshot["revision"] != config["release_revision"]
    status = ("**Diagnostic measurement from a modified checkout.** Not a release baseline."
              if not snapshot["clean"] else "**Historical measurement.** Not a measurement of the latest release."
              if historical else "**Measurement of the documented release.**")
    lines = ["# Benchmark results", "",
             f'<p class="bench-run-note">Recorded {cell(snapshot["measured_at"][:10])}. {cell(status.replace("**", ""))} Run status: {cell(snapshot["status"])}.</p>', ""]
    if simd_note:
        lines.extend([f'<p class="bench-run-note">{cell(simd_note)}</p>', ""])
    details = ["# Benchmark measurement details", "", status, "",
             "[View the results](benchmarks.md) · [Run benchmarks](benchmarking.md)", "",
             "| Measurement identity | Value |", "| --- | --- |",
             f"| Source revision | `{snapshot['revision']}` |", f"| Measured at | {cell(snapshot['measured_at'])} |",
             f"| Result status | {cell(snapshot['status'])} |", f"| Source document SHA-256 | `{snapshot['source_sha256']}` |",
             f"| Policy | {cell(snapshot['policy_status'])} |"]
    for key, value in snapshot["environment"].items():
        details.append(f"| {cell(key)} | {cell(value)} |")
    machines = snapshot.get("machines") or ([snapshot["machine"]] if snapshot.get("machine") else [])
    if machines:
        details += ["", "## Runner cohorts and hardware", "",
                    "| Runner cohort | OS | Architecture | CPU | Python | Rust toolchain | Runner image |", "| --- | --- | --- | --- | --- | --- | --- |"]
        for machine in machines:
            details.append("| " + " | ".join(cell(machine.get(key)) for key in (
                "label", "os", "architecture", "cpu", "python_version", "rust_toolchain", "image_version"
            )) + " |")
    if snapshot.get("cohorts"):
        details += ["", "## Comparison cohorts", "", "| Runner | Cohort | Run | Measured at | Source SHA-256 |", "| --- | --- | --- | --- | --- |"]
        for cohort in snapshot["cohorts"]:
            runner = next((machine["label"] for machine in machines
                           if machine["id"] == cohort.get("machine_id")), "Runner not recorded")
            details.append("| " + " | ".join([cell(runner)] + [cell(cohort.get(key))
                            for key in ("id", "run_id", "measured_at", "source_sha256")]) + " |")
    details += ["", "Download the [public measurement data](assets/benchmark.json). This is a presentation snapshot, "
              "not the full source receipt. It preserves the original report hash and numerical observations; "
              "hostnames, local paths, and internal traces are omitted.", ""]
    details += [f"- {cell(note)}" for note in snapshot["notes"]]
    from docs_benchmark_view import render_dashboard
    lines += [render_dashboard(snapshot, config), ""]
    details += ["", "## Full measurements", "",
                "| Workload | Runner | Subject | Median µs | P90 µs | P95 µs | Samples | Result / correctness | Requested → actual | Terminal receipt |",
                "| --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- | --- |"]
    for row in snapshot["rows"]:
        machine_id = row.get("machine_id", "")
        runner = next((machine["label"] for machine in machines if machine["id"] == machine_id),
                      snapshot.get("machine", {}).get("label", "Runner not recorded"))
        route = f"{row['requested_backend']} → {row['actual_backend']}"
        result = f"{row['status']}; {row['correctness']}"
        terminal = "Complete" if row["terminal_complete"] is True else "Not proven" if row["terminal_complete"] is False else "Not measured" if row["requested_backend"] == "gpu" else "Not applicable"
        details.append("| " + " | ".join(cell(v) for v in (row["workload"], runner, row["subject"], row["median_us"], row["p90_us"], row["p95_us"], row["sample_count"], result, route, terminal)) + " |")
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


def render_pillow_simd_benchmarks(source: Path, output: Path) -> str:
    """Render the separately measured x86/Pillow-SIMD operation cohort."""
    from docs_benchmark_view import render_dashboard

    snapshot = json.loads(source.read_text(encoding="utf-8"))
    validate(snapshot, "appunni-m/pillow-rs")
    machine = snapshot.get("machine", {})
    (output / "assets" / "pillow-simd-benchmark.json").write_text(
        json.dumps(snapshot, indent=2) + "\n", encoding="utf-8"
    )
    source_versions = {
        cohort["id"]: cohort["version"] for cohort in snapshot.get("cohorts", [])
    }
    cohort_rows = [
        "| Source | Package version | Benchmark run | Parity run |",
        "| --- | --- | --- | --- |",
    ]
    cohort_rows.extend(
        "| "
        + " | ".join(
            cell(cohort.get(key))
            for key in ("id", "version", "run_id", "parity_run_id")
        )
        + " |"
        for cohort in snapshot.get("cohorts", [])
    )
    notes = "\n".join(f"- {cell(note)}" for note in snapshot["notes"])
    rows = [
        "| Workload | Mode | Implementation | Median µs | Samples | Output/backend evidence |",
        "| --- | --- | --- | ---: | ---: | --- |",
    ]
    for row in snapshot["rows"]:
        context = row["context"]
        mode = context.get("mode", "Not recorded")
        evidence = f"{row['status']}; {row['correctness']}; {row['requested_backend']} → {row['actual_backend']}"
        rows.append(
            "| "
            + " | ".join(
                cell(value)
                for value in (
                    row["workload"], mode, row["subject"], row["median_us"],
                    row["sample_count"], evidence,
                )
            )
            + " |"
        )
    dashboard = render_dashboard(snapshot, {"benchmark": {"kind": "pillow"}})
    return "\n".join(
        [
            "# Pillow-SIMD x86 operation comparison",
            "",
            f"Recorded {cell(snapshot['measured_at'][:10])} on {cell(machine.get('label') or snapshot['environment'].get('os') or 'an x86 runner')} (CPU: {cell(machine.get('cpu') or 'not recorded')}; Python: {cell(machine.get('python_version') or 'not recorded')}; Rust: {cell(machine.get('rust_toolchain') or 'not recorded')}; image: {cell(machine.get('image_version') or 'not recorded')}). This page compares only workloads run in that one benchmark run: Pillow {source_versions.get('pillow', 'version not recorded')}, Pillow-SIMD {source_versions.get('pillow-simd', 'version not recorded')}, and the pillow-rs CPU/SIMD profiles.",
            "",
            "Pillow-SIMD installs the `PIL` namespace, so the two Pillow variants are run in isolated environments. The benchmark admits timings only after each source has independently passed the same exact-output parity cases against pillow-rs. These results are separate from the Apple ARM results on the main [benchmark page](benchmarks.md).",
            "",
            *snapshot["notes"],
            "",
            dashboard,
            "",
            "## Matched source and parity runs",
            "",
            *cohort_rows,
            "",
            "## Workload-level evidence",
            "",
            *rows,
            "",
            "## Measurement notes",
            "",
            notes,
            "",
            f"Source revision: `{snapshot['revision']}` · Run: `{cell(snapshot['run_id'])}` · Policy: {cell(snapshot['policy_status'])}.",
            "",
            "Download the [public measurement data](assets/pillow-simd-benchmark.json) or follow the [benchmark protocol](benchmarking.md).",
            "",
        ]
    )


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
            lines += ["", f"## {surface['id']}", "", "| Public path | Declared support | Input cases | Contract |", "| --- | --- | ---: | --- |"]
            for operation in surface["operations"]:
                key = (surface["id"], operation["id"])
                path = operation["source"]["path"]
                status = ", ".join(t["support"]["status"] for t in operation["targets"])
                witness = f"[{counts[key]}]({paths[key]})" if key in paths else "0 — unmeasured"
                signature = contract_code(path, operation["source"]["signature"])
                lines.append(f"| `{path}` | {status} | {witness} | {signature} |")
        return "\n".join(lines) + "\n"
    raise ValueError("unknown generated support inventory")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=("pillow", "fontdone", "jpeg", "merge"))
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--parallel-source", type=Path,
                        help="optional target-only Parallel CPU result to compare with ordinary Pillow")
    args = parser.parse_args()
    if args.kind == "merge":
        snapshot = merge_pillow_snapshots(args.source, args.repository, args.output)
        print(f"Merged {len(snapshot['machines'])} runner cohorts and {len(snapshot['rows'])} rows")
        return
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
    source_hash = hashlib.sha256(raw).hexdigest()
    if args.parallel_source:
        if args.kind != "pillow":
            raise ValueError("a separate Parallel CPU source is supported only for Pillow benchmarks")
        parallel_raw = args.parallel_source.read_bytes()
        parallel_document = json.loads(parallel_raw)
        parallel = project_pillow(parallel_document)
        parallel_hash = hashlib.sha256(parallel_raw).hexdigest()
        snapshot = merge_parallel_cpu(snapshot, parallel, source_hash, parallel_hash)
        source_hash = hashlib.sha256(raw + b"\0parallel-cpu\0" + parallel_raw).hexdigest()
    snapshot.update(schema=SCHEMA, repository=args.repository, source_sha256=source_hash)
    validate(snapshot, args.repository)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n")
    print(f"Published-view snapshot: {len(snapshot['rows'])} rows; source {snapshot['revision']}")


if __name__ == "__main__":
    main()

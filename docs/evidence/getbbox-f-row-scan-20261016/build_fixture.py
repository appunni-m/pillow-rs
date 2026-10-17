#!/usr/bin/env python3
"""Build a disposable input-only GetBBox F workload bundle under /tmp.

The benchmark runner's manifest is a slice containing only Image.new and
Image.getbbox metadata. Both runners have a fixed repository fixture root, so
this script writes the same relative inputs under the temporary bundle; callers
must stage those two JSON files under pillow-rs/tests/fixtures before invoking
the runners with this temporary manifest.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import yaml

CASE_ID = "PIL.Image.Image.getbbox.nuanced.performance-f-filled-1024x768"
WORKLOAD_ID = "pil-image-image.getbbox.materialized.f-filled-1024x768"
REQUIREMENT_ID = "PIL.Image.Image.getbbox.performance.standard"
PARITY_REL = "inputs/parity/getbbox-f-filled-1024x768.json"
BENCHMARK_REL = "inputs/benchmark/getbbox-f-filled-1024x768.json"


def find_surface(manifest: dict, surface_id: str) -> dict:
    return next(surface for surface in manifest["surfaces"] if surface["id"] == surface_id)


def selected_operation(surface: dict, operation_id: str) -> dict:
    return next(operation for operation in surface["operations"] if operation["id"] == operation_id)


def build(repo: Path, out: Path) -> None:
    fixture_root = repo / "pillow-rs" / "tests" / "fixtures"
    source_manifest = yaml.safe_load((fixture_root / "manifest.yaml").read_text(encoding="utf-8"))

    image_namespace = find_surface(source_manifest, "PIL.Image")
    image_type = find_surface(source_manifest, "PIL.Image.Image")
    manifest = dict(source_manifest)
    manifest["scope"] = dict(source_manifest["scope"])
    manifest["scope"]["id"] = "pillow-rs-getbbox-f-filled-slice"
    manifest["scope"]["mode"] = "slice"
    manifest["oracles"] = [item for item in source_manifest["oracles"] if item["id"] == "pillow"]
    manifest["targets"] = [item for item in source_manifest["targets"] if item["id"] == "pillow-rs-python"]
    manifest["target_profiles"] = [item for item in source_manifest["target_profiles"] if item["id"] == "python-cpu"]

    needed_commands = {
        source_manifest["scope"]["inventory"]["command_id"],
        *(item["identity_command_id"] for item in source_manifest["oracles"] if item["id"] == "pillow"),
        *(item["identity_command_id"] for item in source_manifest["targets"] if item["id"] == "pillow-rs-python"),
        *(item["command_id"] for item in source_manifest["interfaces"].values()),
        source_manifest["documentation"]["command_id"],
    }
    manifest["commands"] = [item for item in source_manifest["commands"] if item["id"] in needed_commands]
    manifest["coverage_components"] = [
        item for item in source_manifest["coverage_components"] if item["id"] == "image-core"
    ]

    new_operation = selected_operation(image_namespace, "new")
    getbbox_operation = selected_operation(image_type, "getbbox")
    manifest["surfaces"] = [
        {**image_namespace, "operations": [new_operation]},
        {**image_type, "operations": [getbbox_operation]},
    ]
    manifest["input_index"] = {
        "parity": [PARITY_REL],
        "coverage": [],
        "benchmark": [BENCHMARK_REL],
    }

    case = {
        "case_id": CASE_ID,
        "surface": "PIL.Image.Image",
        "operation": "getbbox",
        "covers": [REQUIREMENT_ID],
        "target_profiles": ["python-cpu"],
        "assets": [],
        "steps": [
            {
                "step_id": "image",
                "surface": "PIL.Image",
                "operation": "new",
                "receiver": None,
                "arguments": {
                    "mode": {"kind": "literal", "value": "F"},
                    "size": {"kind": "literal", "value": [1024, 768]},
                    "color": {"kind": "literal", "value": 1.0},
                },
            },
            {
                "step_id": "call",
                "surface": "PIL.Image.Image",
                "operation": "getbbox",
                "receiver": {"kind": "binding", "step_id": "image"},
                "arguments": {},
            },
        ],
        "observations": ["call"],
    }
    parity = {"schema": "migration-parity/parity-input@1", "cases": [case]}

    workload = {
        "workload_id": WORKLOAD_ID,
        "covers": [REQUIREMENT_ID],
        "subjects": [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
        ],
        "input": {"kind": "parity_case", "case_id": CASE_ID},
        "measurement": {
            "boundary": "observed_steps",
            "step_ids": ["call"],
            "metrics": ["latency", "throughput"],
            "warmup_iterations": 5,
            "measurement_iterations": 20,
            "samples": 5,
            "concurrency": 1,
            "cache_state": "warm",
            "correctness_gate": "parity_pass",
        },
        "context": {
            "size": [1024, 768],
            "mode": "F",
            "chain_length": 1,
            "operation_class": "terminal",
            "cache_state": "warm",
            "build_profile": "release",
        },
    }
    benchmark = {"schema": "migration-parity/benchmark-input@1", "workloads": [workload], "suites": []}

    out.mkdir(parents=True, exist_ok=True)
    for relative, payload in ((PARITY_REL, parity), (BENCHMARK_REL, benchmark)):
        path = out / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    (out / "manifest.yaml").write_text(yaml.safe_dump(manifest, sort_keys=False), encoding="utf-8")
    print(f"bundle={out}")
    print(f"manifest={out / 'manifest.yaml'}")
    print(f"case_id={CASE_ID}")
    print(f"workload_id={WORKLOAD_ID}")
    print(f"parity_input={PARITY_REL}")
    print(f"benchmark_input={BENCHMARK_REL}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True, help="pillow-rs repository root")
    parser.add_argument("--out", type=Path, default=Path("/tmp/pillow-rs-getbbox-f-filled-1024x768"))
    args = parser.parse_args()
    build(args.repo.resolve(), args.out.resolve())


if __name__ == "__main__":
    main()

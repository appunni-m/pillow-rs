"""Recover this trial's rows from boundary-tagged child envelopes, without reruns.

The original native producer overwrote case-keyed report rows across timing
boundaries. Original reports stay intact. This script uses the retained raw
samples and existing subject aggregation to generate explicitly derived reports.
"""
from pathlib import Path
import copy
import hashlib
import json
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from run_migration_benchmark import subject_result  # noqa: E402


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def recover(label):
    original_path = HERE / f"{label}.original.json.txt"
    raw_path = HERE / f"{label}.raw.jsonl"
    original = json.loads(original_path.read_text())
    entries = [json.loads(line) for line in raw_path.read_text().splitlines()]
    assert len(entries) == 6
    timed = {}
    proofs = {}
    pending = None
    for index, entry in enumerate(entries):
        command = entry["command"]
        side = entry["identity"]["side"]
        if "--timings" in command:
            steps = tuple(command[i + 1] for i, item in enumerate(command) if item == "--timing-step")
            assert steps in {("call",), ("call", "materialize")}
            assert command[command.index("--repeat") + 1] == "105"
            assert command[command.index("--lifecycle") + 1] == "warm"
            assert command[command.index("--timing-boundary") + 1] == "observed_steps"
            assert (steps, side) not in timed
            timed[steps, side] = entry
            if side == "target":
                assert entry["identity"]["backend"] == "simd"
                pending = (steps, index, entry["identity"])
            else:
                assert entry["identity"]["implementation"] == "Pillow"
                assert entry["identity"]["version"] == "12.2.0"
        else:
            assert side == "target" and pending is not None
            steps, prior_index, identity = pending
            assert index == prior_index + 1 and entry["identity"] == identity
            assert command[command.index("--repeat") + 1] == "1"
            assert not entry["timings_ns"]
            proofs[steps] = entry
            pending = None
    assert pending is None and len(timed) == 4 and len(proofs) == 2
    recovered = copy.deepcopy(original)
    rows = []
    for workload in recovered["workloads"]:
        assert workload["correctness"]["outcome"] == "pass"
        steps = tuple(workload["measurement_policy"]["step_ids"])
        subjects = []
        for side, kind, subject_id in [("source", "oracle", "pillow"), ("target", "target_profile", "python-simd")]:
            entry = timed[steps, side]
            assert len(entry["timings_ns"]) == 1
            case_id = next(iter(entry["timings_ns"]))
            assert case_id == "PIL.Image.Image.convert.nuanced.material-cmyk-to-rgb-1024x768"
            durations = entry["timings_ns"][case_id]
            telemetry = entry["telemetry"][case_id]
            assert len(durations) == len(telemetry) == 105
            execution = proofs[steps]["execution"][case_id] if side == "target" else entry["execution"][case_id]
            subject = subject_result(kind, subject_id, durations, telemetry, execution, workload["measurement_policy"])
            assert subject["status"] == "completed"
            if side == "target":
                receipt = subject["execution"]
                assert receipt["actual_backend"] == "simd" and receipt["terminal_complete"]
                assert not receipt["fallback_reason_counts"] and not receipt["errors"]
                assert receipt["operation_telemetry"] == [{"operation": "Convert", "path": "vector", "vector_block_count": 49152, "scalar_tail_count": 0, "mode_conversion_count": 1, "handoff_count": 0}]
            subjects.append(subject)
            rows.append({"workload_id": workload["workload_id"], "subject": subject_id, "steps": list(steps), "median_ms": subject["measurements"][0]["statistics"]["median"]})
        workload["subjects"] = subjects
    output = HERE / f"{label}.recovered.json"
    output.write_text(json.dumps(recovered, indent=2) + "\n")
    return {"cohort": label, "original_report_sha256": digest(original_path), "raw_sha256": digest(raw_path), "recovered_report_sha256": digest(output), "rows": rows}


if __name__ == "__main__":
    recovery = {
        "method": "Project raw timing envelopes by explicit command boundary and side; bind each adjacent untimed target proof; discard exactly five warmups; aggregate the unchanged 100 samples. Original combined reports remain uncorrected historical artifacts.",
        "aggregation_source_sha256": digest(ROOT / "scripts/run_migration_benchmark.py"),
        "recovery_script_sha256": digest(Path(__file__)),
        "cohorts": [recover(label) for label in ["A1-selected", "B1", "B2", "A2"]],
    }
    (HERE / "recovery.json").write_text(json.dumps(recovery, indent=2) + "\n")
    print(json.dumps(recovery["cohorts"], indent=2))

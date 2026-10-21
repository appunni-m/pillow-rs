"""Check public GaussianBlur parity and AVX2 receipt on a transposed-tail shape."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path.cwd()
if not (ROOT / "scripts" / "run_migration_benchmark.py").is_file():
    raise SystemExit("run this probe from the pillow-rs checkout root")

CHILD = r"""
import hashlib, json, os
from pathlib import Path
from PIL import Image, ImageFilter

width, height = 65, 47
pixels = hashlib.shake_256(b"GaussianBlur AVX2 odd-size parity guard").digest(width * height * 4)
image = Image.frombytes("RGBA", (width, height), pixels)
core = None
if os.environ.get("GAUSSIAN_PROBE_TARGET") == "1":
    from pillow_rs import disable_backend, enable_backend
    from pillow_rs import _core as core
    for backend in ("cpu", "simd", "gpu"):
        disable_backend(backend)
    assert enable_backend("simd")
    core.set_pipeline_telemetry(True)
result = image.filter(ImageFilter.GaussianBlur(7.5))
output = result.tobytes()
receipt = core.take_pipeline_telemetry() if core is not None else None
print(json.dumps({
    "pillow_version": getattr(__import__("PIL"), "__version__", ""),
    "mode": result.mode,
    "size": result.size,
    "output_bytes": len(output),
    "output_sha256": hashlib.sha256(output).hexdigest(),
    "core_path": str(Path(core.__file__).resolve()) if core is not None else None,
    "core_sha256": hashlib.sha256(Path(core.__file__).read_bytes()).hexdigest() if core is not None else None,
    "receipt": receipt,
}, sort_keys=True))
"""


def run(target: bool) -> dict[str, object]:
    environment = os.environ.copy()
    for name in ("MIGRATION_TARGET_BACKEND", "MIGRATION_STRICT_TARGET_BACKEND", "GAUSSIAN_PROBE_TARGET"):
        environment.pop(name, None)
    environment.pop("PYTHONPATH", None)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    if target:
        environment["PYTHONPATH"] = str(ROOT / "pillow-rs-py" / "python")
        environment["MIGRATION_TARGET_BACKEND"] = "simd"
        environment["MIGRATION_STRICT_TARGET_BACKEND"] = "1"
        environment["GAUSSIAN_PROBE_TARGET"] = "1"
    completed = subprocess.run(
        [sys.executable, "-c", CHILD],
        cwd=ROOT,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout.strip().splitlines()[-1])


oracle = run(target=False)
target = run(target=True)
assert (oracle["mode"], oracle["size"], oracle["output_bytes"]) == (
    target["mode"],
    target["size"],
    target["output_bytes"],
)
assert oracle["output_sha256"] == target["output_sha256"], "Pillow output differs"
receipt = target["receipt"]
assert receipt["actual_backend"] == "simd"
assert not receipt["fallback_reason"]
operations = receipt["operation_telemetry"]
assert len(operations) == 1 and operations[0]["operation"] == "GaussianBlur"
assert operations[0]["path"] == "avx2-transpose"
assert operations[0]["scalar_tail_count"] > 0, "odd dimensions did not exercise transpose tails"

print(json.dumps({
    "shape": [65, 47],
    "mode": "RGBA",
    "radius": 7.5,
    "pillow_version": oracle["pillow_version"],
    "target_extension_sha256": target["core_sha256"],
    "output_bytes": target["output_bytes"],
    "output_sha256": target["output_sha256"],
    "exact_pillow_parity": True,
    "receipt": receipt,
}, indent=2, sort_keys=True))

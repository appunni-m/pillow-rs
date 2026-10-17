"""Capture child timing envelopes after measured work; leave producer unmodified."""
import importlib.util
import json
import os
from pathlib import Path
import sys


def capture(process, output):
    def run(*args, **kwargs):
        result = process(*args, **kwargs)
        code, stdout, stderr = result
        if code == 0:
            try:
                payload = json.loads(stdout)
            except (ValueError, TypeError):
                payload = None
            if isinstance(payload, dict) and "timings_ns" in payload:
                retained = {key: payload[key] for key in ("identity", "timings_ns", "telemetry", "execution") if key in payload}
                retained["label"] = kwargs.get("label")
                retained["command"] = args[0] if args else None
                with output.open("a") as stream:
                    stream.write(json.dumps(retained, separators=(",", ":")) + "\n")
        return result
    return run


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[3]
    sys.path.insert(0, str(root / "scripts"))
    spec = importlib.util.spec_from_file_location("retained_native_benchmark", root / "scripts/run_migration_benchmark.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    output = Path(os.environ["CMYK_RAW_SAMPLES"])
    if output.exists():
        raise SystemExit("Refusing to append to existing raw sample run")
    module.run_process = capture(module.run_process, output)
    raise SystemExit(module.main())

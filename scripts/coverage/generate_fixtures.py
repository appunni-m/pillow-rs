#!/usr/bin/env python3
"""Generate test fixtures using the shared execution engine + PIL backend."""
import json, hashlib, sys
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent
FIXTURES_DIR = ROOT / "tests" / "fixtures"

sys.path.insert(0, str(ROOT))
from scripts.coverage.pil_backend import PilBackend, get_reference, make_image
from scripts.coverage.execution_engine import execute
from scripts.coverage.ops_registry import REGISTRY

_backend = PilBackend()
_ref_rgb = None


def _get_ref():
    global _ref_rgb
    if _ref_rgb is None:
        _ref_rgb = get_reference()
    return _ref_rgb


def _make_input(mode):
    return make_image(mode)


def _type_map(reg_type):
    """Map registry type to fixture operation type."""
    m = {"image": "method", "module": "classmethod"}
    return m.get(reg_type, reg_type)


def generate_fixture(op_name, mode):
    spec = REGISTRY[op_name]
    typ = spec["type"]
    module, target = op_name.rsplit(".", 1) if "." in op_name else (op_name, op_name)
    params = dict(spec.get("params", {}))

    # Build operation
    op_def = {"type": _type_map(typ), "module": module, "target": target, "params": params}

    # Create image(s)
    img = _make_input(mode)
    img2 = None
    if typ in ("dual",) and not op_name.startswith("ImageChops."):
        img2 = _make_input(mode)
    elif op_name.startswith("ImageChops.") and op_name.rsplit(".",1)[1] not in ("invert","duplicate","constant","offset"):
        img2 = _make_input(mode)
    # For module-level blend/composite
    if op_name in ("ImageModule.blend", "ImageModule.composite"):
        img2 = _make_input(mode)

    # Execute
    result = execute(_backend, op_def, img, img2)

    # Build fixture
    ref_bytes = _get_ref().tobytes().hex()
    fixture = {
        "format_version": 1,
        "operation": op_def,
        "input": {"mode": mode, "size": list(img.size), "bytes": img.tobytes().hex()},
        "config": {"reference_bytes_rgb": ref_bytes, "targets": spec.get("targets", ["cpu"])},
    }
    if img2:
        fixture["input2"] = {"mode": mode, "size": list(img2.size), "bytes": img2.tobytes().hex()}

    # Expected
    if hasattr(result, 'tobytes'):
        fixture["expected"] = {"result_type": "hash", "value": hashlib.sha256(result.tobytes()).hexdigest()}
    elif isinstance(result, bytes):
        fixture["expected"] = {"result_type": "hash", "value": hashlib.sha256(result).hexdigest()}
    elif isinstance(result, (int, float, str, bool, list, tuple, dict, type(None))):
        fixture["expected"] = {"result_type": "value", "value": _serialize(result)}
    else:
        fixture["expected"] = {"result_type": "value", "value": str(result) if result is not None else None}

    return fixture


def _serialize(val):
    if val is None: return None
    if isinstance(val, (int, float, str, bool)): return val
    if isinstance(val, bytes): return val.hex()
    if isinstance(val, tuple): return [_serialize(v) for v in val]
    if isinstance(val, list): return [_serialize(v) for v in val]
    if isinstance(val, dict): return {str(k): _serialize(v) for k, v in val.items()}
    if hasattr(val, '__iter__') and not isinstance(val, (str, bytes)):
        try: return [_serialize(v) for v in list(val)[:1000]]
        except: pass
    return str(val)


def main():
    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    count = 0
    for op_name, spec in REGISTRY.items():
        for mode in spec.get("modes", ["L", "RGB"]):
            try:
                fixture = generate_fixture(op_name, mode)
                fname = op_name.replace(".", "_") + "_" + mode + ".json"
                with open(FIXTURES_DIR / fname, "w") as f:
                    json.dump(fixture, f, indent=2)
                count += 1
            except Exception as e:
                print(f"  SKIP {op_name} x {mode}: {e}", file=sys.stderr)
    print(f"Generated {count} fixtures in {FIXTURES_DIR}")


if __name__ == "__main__":
    main()

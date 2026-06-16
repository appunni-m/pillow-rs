"""Generic PIL-RSPIL parity test runner.

Discovers fixture pairs from fixtures/input/jsons/ and fixtures/outputs/jsons/.
Zips input cases with expected outputs by case id.
Zero per-operation logic — the engine handles everything.
"""

import json
from pathlib import Path

import pytest
import pillow_rs as rspil

from engine import CALL_STYLE, ASSERT, create_input, get_call_style

FIXTURES_DIR = Path(__file__).parent / "fixtures"
INPUT_DIR = FIXTURES_DIR / "input" / "jsons"
OUTPUT_DIR = FIXTURES_DIR / "outputs" / "jsons"


def _discover():
    """Yield every input fixture that has a corresponding output fixture."""
    for fpath in sorted(INPUT_DIR.glob("*.json")):
        if (OUTPUT_DIR / fpath.name).exists():
            yield pytest.param(fpath.name, id=fpath.stem)


@pytest.mark.parametrize("fixture_file", _discover())
def test_parity(fixture_file):
    inp = json.loads((INPUT_DIR / fixture_file).read_text())
    out = json.loads((OUTPUT_DIR / fixture_file).read_text())
    op = inp["operation"]
    call_style = get_call_style(op["module"], op["target"])

    # Index output cases by id for O(1) lookup
    out_cases = {c["id"]: c for c in out["cases"]}

    for case in inp["cases"]:
        cid = case["id"]
        mode = case.get("mode")
        img = create_input(rspil, mode, case.get("input"))
        img2 = create_input(rspil, mode, case.get("input2"))
        params = dict(case.get("params", {}))

        assertion = out_cases[cid]["assert"]

        try:
            result = CALL_STYLE[call_style](rspil, img, img2, op["target"], params)
        except Exception as e:
            if assertion["method"] == "error":
                assert ASSERT["error"](assertion, e), f"[{cid}] error mismatch"
                continue
            raise

        assert ASSERT[assertion["method"]](assertion, result), \
            f"[{cid}] {assertion['method']} mismatch"


# ── Coverage validation ──────────────────────────────────────────

def test_coverage_complete():
    """Every non-ignored implemented operation in manifest.yaml must have a fixture."""
    import yaml
    manifest_path = Path(__file__).parent.parent / "manifest.yaml"
    with open(manifest_path) as f:
        manifest = yaml.safe_load(f)

    fixtures = set(f.stem for f in INPUT_DIR.glob("*.json"))
    missing = []

    for mod_name, mod_data in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for entry in mod_data.get(section, []):
                if entry.get("status") == "implemented":
                    op = f"{mod_name}.{entry['name']}"
                    if op not in fixtures:
                        missing.append(op)

        for cls in mod_data.get("classes", []):
            if cls.get("status") == "implemented":
                op = f"{mod_name}.{cls['name']}"
                if op not in fixtures:
                    missing.append(op)
            for entry in cls.get("methods", []):
                entry_status = entry.get("status", "")
                if cls.get("status") == "implemented" and entry_status != "ignored":
                    op = f"{mod_name}.{entry['name']}"
                    if op not in fixtures:
                        missing.append(op)

    if missing:
        pytest.fail(
            f"Missing fixtures for {len(missing)} implemented operations:\n  " +
            "\n  ".join(sorted(missing))
        )

#!/usr/bin/env python3
"""Generate decode reference fixtures — matches tests/fixtures/ pattern exactly.

Reads manifest.yaml, loads each test asset via PIL (libjpeg/libpng/etc.),
extracts raw pixels via image.tobytes(), writes .bin reference files and
output fixture JSONs.

Pattern (mirrors scripts/generate_fixtures.py):
  Input:  tests/fixtures/input/jsons/Decode.{format}.json  → test cases
  Output: tests/fixtures/outputs/jsons/Decode.{format}.json → expected results
  References: tests/fixtures/outputs/raws/{name}.bin → raw pixel bytes from PIL

Reference .bin files contain raw pixel bytes matching PIL.Image.tobytes().
"""
import json, hashlib, sys, argparse
from pathlib import Path
import yaml

ROOT = Path(__file__).parent.parent
MANIFEST = ROOT / "manifest.yaml"
INPUT_JSONS = ROOT / "tests" / "fixtures" / "input" / "jsons"
OUTPUT_JSONS = ROOT / "tests" / "fixtures" / "outputs" / "jsons"
OUTPUT_RAWS = ROOT / "tests" / "fixtures" / "outputs" / "raws"
ASSETS_DIR = ROOT / "test-assets" / "input"


def generate(target_format=None):
    manifest = yaml.safe_load(MANIFEST.read_text())
    generated = 0

    for fmt_name, fmt_data in manifest["formats"].items():
        if target_format and fmt_name != target_format:
            continue

        # Read or create input fixture
        input_json = INPUT_JSONS / f"Decode.{fmt_name}.json"
        if input_json.exists():
            inp = json.loads(input_json.read_text())
        else:
            inp = {"format_version": 2, "operation": {"module": "Decode", "target": fmt_name}, "cases": []}

        out_cases = []
        asset_dir = ASSETS_DIR / fmt_name

        for case in fmt_data.get("edge_cases", []):
            for asset_name in case.get("test_assets", []):
                img_path = asset_dir / asset_name
                if not img_path.exists():
                    continue

                cid = f"Decode_{fmt_name}_{asset_name.replace('.', '_')}"
                try:
                    from PIL import Image
                    img = Image.open(img_path)
                    raw = img.tobytes()

                    ref_name = f"Decode.{fmt_name}_{asset_name.replace('.', '_')}.bin"
                    ref_path = OUTPUT_RAWS / ref_name
                    ref_path.parent.mkdir(parents=True, exist_ok=True)
                    ref_path.write_bytes(raw)

                    expect_error = case.get("expect_error", False)

                    # Ensure input case exists
                    inp_case = {"id": cid, "mode": img.mode, "asset": f"{fmt_name}/{asset_name}"}
                    if not any(c["id"] == cid for c in inp["cases"]):
                        inp["cases"].append(inp_case)

                    # Output assertion
                    out_cases.append({
                        "id": cid,
                        "assert": {"method": "error", "exception": "DecodeError", "message_contains": ""}
                        if expect_error else
                        {"method": "binary", "reference": f"raws/{ref_name}"},
                    })
                except Exception as e:
                    print(f"  FAIL {asset_name}: {e}", file=sys.stderr)

        # Write fixtures
        INPUT_JSONS.mkdir(parents=True, exist_ok=True)
        input_json.write_text(json.dumps(inp, indent=2) + "\n")

        out = {"format_version": 2, "operation": {"module": "Decode", "target": fmt_name}, "cases": out_cases}
        OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
        (OUTPUT_JSONS / f"Decode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

        print(f"  OK  Decode.{fmt_name} ({len(out_cases)} cases)")
        generated += 1

    print(f"\nGenerated {generated} format fixtures")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--format", help="Only generate for specific format")
    p.add_argument("--dry-run", action="store_true")
    args = p.parse_args()
    if not args.dry_run:
        generate(args.format)
    else:
        print("Dry run — would generate fixtures for all formats in manifest.yaml")

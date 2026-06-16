#!/usr/bin/env python3
"""Fill missing mode gaps in fixture JSONs.

For each gap (operation declares mode as supported but no fixture case exists),
copy an existing case's input spec, change the mode, and add to the fixture.
Then run generate_fixtures.py to produce the PIL reference output.
"""
import json
import sys
import yaml
from pathlib import Path

ROOT = Path(__file__).parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"
INPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "jsons"


def load_manifest():
    with open(MANIFEST_PATH) as f:
        return yaml.safe_load(f)


def get_op_modes(manifest):
    """Return {operation_name: set of supported_modes} from manifest."""
    op_modes = {}
    for mod_name, mod_data in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for entry in mod_data.get(section, []):
                if entry.get("status") == "implemented":
                    op = f"{mod_name}.{entry['name']}"
                    modes = set(entry.get("supported_modes", []))
                    if modes:
                        op_modes[op] = modes

        for cls in mod_data.get("classes", []):
            if cls.get("status") == "implemented":
                methods = cls.get("methods", [])
                if methods:
                    for entry in methods:
                        entry_status = entry.get("status", "")
                        if cls.get("status") == "implemented" and entry_status != "ignored":
                            op = f"{mod_name}.{entry['name']}"
                            modes = set(entry.get("supported_modes",
                                        cls.get("supported_modes", [])))
                            if modes:
                                op_modes[op] = modes
                else:
                    op = f"{mod_name}.{cls['name']}"
                    modes = set(cls.get("supported_modes", []))
                    if modes:
                        op_modes[op] = modes
    return op_modes


def main():
    manifest = load_manifest()
    op_modes = get_op_modes(manifest)

    added = 0
    for fpath in sorted(INPUT_DIR.glob("*.json")):
        stem = fpath.stem
        if stem not in op_modes:
            continue

        declared_modes = op_modes[stem]
        fx = json.loads(fpath.read_text())

        # Get existing modes from cases
        existing_modes = set()
        for case in fx["cases"]:
            mode = case.get("mode", "")
            if mode:
                existing_modes.add(mode)

        gaps = declared_modes - existing_modes
        if not gaps:
            continue

        # Find a template case to copy structure from
        template = fx["cases"][0]

        for mode in sorted(gaps):
            new_case = {
                "id": f"{stem}_{mode}",
                "mode": mode,
            }

            # Copy input spec from template, adapting for the new mode
            inp = template.get("input")
            if inp:
                if inp.get("source") == "reference_rgb":
                    new_case["input"] = {
                        "source": "reference_rgb",
                        "size": inp.get("size", [256, 256]),
                    }
                elif inp.get("source") == "constant":
                    # For constant source, pick sensible defaults per mode
                    color = inp.get("color", 0)
                    if isinstance(color, list):
                        color = color[:3] if mode in ("RGB", "HSV", "YCbCr") else color
                    new_case["input"] = {"source": "constant", "size": inp.get("size", [100, 100]), "color": color}
                elif inp.get("source") == "bytes":
                    new_case["input"] = {"source": "bytes", "size": inp.get("size", [256, 256]), "bytes": inp["bytes"]}
                else:
                    new_case["input"] = {"source": "reference_rgb", "size": [256, 256]}

            # Copy input2 if present
            if "input2" in template:
                new_case["input2"] = template["input2"]

            # Copy params
            if "params" in template:
                new_case["params"] = template["params"]

            fx["cases"].append(new_case)
            print(f"  + {stem}: {mode}")
            added += 1

        # Sort cases by id
        fx["cases"].sort(key=lambda c: c["id"])
        fpath.write_text(json.dumps(fx, indent=2) + "\n")

    print(f"\nAdded {added} cases across all fixtures")

    if added > 0:
        print("\nNow run: python scripts/generate_fixtures.py")
        # Auto-run fixture generation
        import subprocess
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts/generate_fixtures.py")],
            cwd=ROOT, capture_output=True, text=True
        )
        print(result.stdout[-500:] if result.stdout else "")
        if result.stderr:
            print("STDERR:", result.stderr[-500:])


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Generate Rust stub functions from manifest.yaml."""
import yaml
import sys
from pathlib import Path


def load_manifest(path: str) -> dict:
    with open(path) as f:
        return yaml.safe_load(f)


def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else "manifest.yaml"
    manifest = load_manifest(manifest_path)
    ops_dir = Path("pillow-rs-core/src/ops")
    existing = set()
    for rs_file in ops_dir.glob("*.rs"):
        for line in rs_file.read_text().split("\n"):
            if "pub fn " in line:
                name = line.split("pub fn ")[1].split("(")[0].strip()
                existing.add(name)

    missing = []
    for mod_name, mod_def in manifest.get("modules", {}).items():
        for method in mod_def.get("methods", []):
            if method["name"] not in existing:
                missing.append((mod_name, method["name"], method))

    if missing:
        print(f"Missing stubs ({len(missing)}):")
        for mod, name, _ in missing:
            print(f"  {mod}.{name}")
    else:
        print("All manifest entries have stubs.")


if __name__ == "__main__":
    main()

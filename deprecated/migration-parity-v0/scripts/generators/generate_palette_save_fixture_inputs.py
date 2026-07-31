#!/usr/bin/env python3
"""Generate independent file-like and path ImagePalette.save inputs."""

import argparse
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixtures-dir", type=Path, required=True)
    parser.add_argument("--suite", type=int, required=True)
    args = parser.parse_args()

    suffix = f"_suite{args.suite}" if args.suite else ""
    cases = [
        {
            "id": f"ImagePalette_save_{mode}{suffix}",
            "params": {},
            "mode": mode,
        }
        for mode in ("L", "P", "RGB")
    ]
    cases.append(
        {
            "id": f"ImagePalette_save_path_RGB{suffix}",
            "params": {"destination": "path"},
            "mode": "RGB",
        }
    )
    payload = {"format_version": 2}
    if args.suite:
        payload["suite"] = args.suite
    payload.update(
        {
            "operation": {"module": "ImagePalette", "target": "save"},
            "cases": cases,
        }
    )
    path = args.fixtures_dir / "input" / "jsons" / "ImagePalette.save.json"
    path.write_text(json.dumps(payload, indent=2) + "\n")


if __name__ == "__main__":
    main()

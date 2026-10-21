#!/usr/bin/env python3
"""Check the Cargo feature and CPU/GPU ownership boundaries."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_PACKAGES = {"pillow-rs", "pillow-rs-py", "pillow-rs-js"}
BACKEND_ROOTS = (
    Path("pillow-rs/src/compute/pool_gpu"),
    Path("pillow-rs/src/compute/pool_simd"),
)
RAYON_CODE = re.compile(
    r"\brayon\s*::|\buse\s+rayon\b|\.(?:into_)?par_iter(?:_mut)?\s*\(|"
    r"\bpar_chunks(?:_mut)?\s*\("
)


def active_feature_tokens(package: dict[str, object]) -> list[str]:
    features = package.get("features", {})
    if not isinstance(features, dict):
        return []
    pending = list(features.get("default", []))
    expanded: set[str] = set()
    tokens: list[str] = []
    while pending:
        token = str(pending.pop())
        if token in features:
            if token not in expanded:
                expanded.add(token)
                pending.extend(str(value) for value in features[token])
        else:
            tokens.append(token)
    return tokens


def cargo_packages() -> list[dict[str, object]]:
    cargo = os.environ.get("CARGO", "cargo")
    result = subprocess.run(
        [cargo, "metadata", "--format-version", "1", "--no-deps", "--locked"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "cargo metadata failed")
    return json.loads(result.stdout).get("packages", [])


def rayon_violations() -> list[str]:
    findings: list[str] = []
    for relative_root in BACKEND_ROOTS:
        path_root = ROOT / relative_root
        if not path_root.exists():
            continue
        for path in sorted(path_root.rglob("*.rs")):
            relative = path.relative_to(ROOT).as_posix()
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
                code = line.split("//", 1)[0]
                if RAYON_CODE.search(code):
                    findings.append(f"{relative}:{number}: Rayon-style execution in CPU SIMD/GPU code")
    return findings


def main() -> int:
    try:
        packages = [
            package
            for package in cargo_packages()
            if package.get("name") in WORKSPACE_PACKAGES
        ]
    except (OSError, RuntimeError, json.JSONDecodeError) as error:
        print(f"Backend-boundary check could not inspect Cargo metadata: {error}", file=sys.stderr)
        return 2

    errors: list[str] = []
    found = {str(package["name"]): package for package in packages}
    for name in sorted(WORKSPACE_PACKAGES - found.keys()):
        errors.append(f"workspace package is missing from Cargo metadata: {name}")

    for name, package in sorted(found.items()):
        tokens = active_feature_tokens(package)
        forbidden = [token for token in tokens if "parallel" in token.lower() or "rayon" in token.lower()]
        if forbidden:
            errors.append(
                f"{name} default features activate Parallel CPU/Rayon: {', '.join(sorted(forbidden))}"
            )

    core = found.get("pillow-rs")
    if core and "parallel" not in core.get("features", {}):
        errors.append("pillow-rs must retain its explicit opt-in parallel feature")

    errors.extend(rayon_violations())
    if errors:
        print("Backend-boundary violations:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1

    print("Backend-boundary checks passed: defaults stay serial and SIMD/GPU code has no Rayon scheduling.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

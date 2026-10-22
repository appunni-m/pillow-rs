#!/usr/bin/env python3
"""Check Cargo features, backend ownership, and the SIMD adapter facade."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_PACKAGES = {"pillow-rs", "pillow-rs-py", "pillow-rs-js"}
RUST_SOURCE_ROOTS = (
    Path("pillow-rs/src"),
    Path("pillow-rs-py/src"),
    Path("pillow-rs-js/src"),
)
CORE_LIBRARY = Path("pillow-rs/src/lib.rs")
PARALLEL_MODULE = Path("pillow-rs/src/par.rs")
SIMD_ADAPTER_FACADE = Path("pillow-rs/src/compute/pool_simd/ops/adapters.rs")
SIMD_FACADE_FUNCTIONS = frozenset(
    {
        "simd_unsupported",
        "simd_supports_for_image",
        "preserves_native_contract",
        "simd_supports_for_shape",
        "simd_in_place_supported",
        "simd_execute_in_place",
    }
)
TOP_LEVEL_FUNCTION = re.compile(
    r"(?m)^(?:(?:pub(?:\([^)]*\))?|async|unsafe|const|extern(?:[ \t]+\"[^\"]+\")?)[ \t]+)*"
    r"fn[ \t]+([A-Za-z_][A-Za-z0-9_]*)\b"
)
RAYON_CODE = re.compile(
    r"\brayon\s*::|\buse\s+rayon\b|\.(?:into_)?par_iter(?:_mut)?\s*\(|"
    r"\bpar_chunks(?:_mut)?\s*\("
)
DIRECT_FACADE_INSPECTION = re.compile(
    r"\b(?:img|shape)\s*\."
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
            if "parallel" in token.lower() or "rayon" in token.lower():
                tokens.append(token)
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


def rayon_violations(root: Path = ROOT) -> list[str]:
    findings: list[str] = []
    for relative_root in RUST_SOURCE_ROOTS:
        path_root = root / relative_root
        if not path_root.exists():
            continue
        for path in sorted(path_root.rglob("*.rs")):
            relative_path = path.relative_to(root)
            relative = relative_path.as_posix()
            lines = path.read_text(encoding="utf-8").splitlines()
            for number, line in enumerate(lines, start=1):
                code = line.split("//", 1)[0]
                if not RAYON_CODE.search(code):
                    continue
                if relative_path == PARALLEL_MODULE:
                    continue
                if relative_path == CORE_LIBRARY and code.strip() == "use rayon as _;":
                    previous = lines[number - 2].strip() if number > 1 else ""
                    if previous == '#[cfg(feature = "parallel")]':
                        continue
                findings.append(
                    f"{relative}:{number}: Rayon belongs in the opt-in Parallel CPU module "
                    "pillow-rs/src/par.rs, not serial, SIMD, GPU, or binding code"
                )
    return findings


def parallel_module_gate_violations(root: Path = ROOT) -> list[str]:
    """Require the Rayon helper module to remain behind the opt-in feature."""
    path = root / CORE_LIBRARY
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return [f"{CORE_LIBRARY}: could not inspect the Parallel CPU module gate: {error}"]

    declarations = [
        index for index, line in enumerate(lines) if re.fullmatch(r"\s*mod par;\s*", line)
    ]
    if len(declarations) != 1:
        return [f"{CORE_LIBRARY}: expected one private `mod par;` declaration"]

    index = declarations[0] - 1
    while index >= 0 and (not lines[index].strip() or lines[index].lstrip().startswith("///")):
        index -= 1
    if index < 0 or lines[index].strip() != '#[cfg(feature = "parallel")]':
        return [
            f"{CORE_LIBRARY}:{declarations[0] + 1}: `mod par;` must use "
            '#[cfg(feature = "parallel")]'
        ]
    return []


def simd_facade_violations(source: str) -> list[str]:
    """Keep operation admission and implementation in focused family modules."""
    defined = set(TOP_LEVEL_FUNCTION.findall(source))
    unexpected = sorted(defined - SIMD_FACADE_FUNCTIONS)
    missing = sorted(SIMD_FACADE_FUNCTIONS - defined)
    findings = [
        f"{SIMD_ADAPTER_FACADE}: {name} belongs in adapters/<family>.rs, not the dispatch facade"
        for name in unexpected
    ]
    findings.extend(
        f"{SIMD_ADAPTER_FACADE}: expected dispatch function is missing: {name}"
        for name in missing
    )
    for number, line in enumerate(source.splitlines(), start=1):
        code = line.split("//", 1)[0]
        if DIRECT_FACADE_INSPECTION.search(code):
            findings.append(
                f"{SIMD_ADAPTER_FACADE}:{number}: image and shape admission belongs in "
                "adapters/<family>.rs; the facade should delegate it"
            )
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
    if core:
        features = core.get("features", {})
        parallel = features.get("parallel") if isinstance(features, dict) else None
        if not isinstance(parallel, list):
            errors.append("pillow-rs must retain its explicit opt-in parallel feature")
        elif not any(str(token) in {"rayon", "dep:rayon"} for token in parallel):
            errors.append("pillow-rs parallel feature must enable its optional Rayon dependency")
        rayon_dependencies = [
            dependency
            for dependency in core.get("dependencies", [])
            if dependency.get("name") == "rayon"
        ]
        if not rayon_dependencies or any(not dependency.get("optional") for dependency in rayon_dependencies):
            errors.append("the core Rayon dependency must remain optional")

    errors.extend(rayon_violations())
    errors.extend(parallel_module_gate_violations())
    try:
        facade_source = (ROOT / SIMD_ADAPTER_FACADE).read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"could not read SIMD adapter facade: {error}")
    else:
        errors.extend(simd_facade_violations(facade_source))
    if errors:
        print("Backend-boundary violations:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1

    print(
        "Backend-boundary checks passed: defaults stay serial, SIMD/GPU code has no Rayon scheduling, "
        "Rayon stays in Parallel CPU, and the SIMD facade delegates family admission."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

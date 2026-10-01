#!/usr/bin/env python3
"""Check one declared release version across packages and normalize PyPI artifacts."""
from __future__ import annotations

import argparse
import ast
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
NUMBER = r"(?:0|[1-9][0-9]*)"
VERSION = re.compile(
    rf"(?P<base>{NUMBER}\.{NUMBER}\.{NUMBER})"
    rf"(?:-(?P<phase>alpha|beta|rc)\.(?P<serial>{NUMBER}))?"
)


def python_version(version: str) -> str:
    """Map the supported SemVer spelling to PEP 440 without another version counter."""
    match = VERSION.fullmatch(version)
    if match is None:
        raise ValueError(f"expected X.Y.Z or X.Y.Z-(alpha|beta|rc).N: {version!r}")
    phase = match["phase"]
    return match["base"] + (
        {"alpha": "a", "beta": "b", "rc": "rc"}[phase] + match["serial"]
        if phase else ""
    )


def runtime_version(root: Path = ROOT) -> str:
    """Read the thin binding's declared version without importing its extension."""
    path = root / "pillow-rs-py/python/pillow_rs/__init__.py"
    assignments = [node.value for node in ast.parse(path.read_text()).body
                   if isinstance(node, ast.Assign)
                   and any(isinstance(target, ast.Name) and target.id == "__version__"
                           for target in node.targets)]
    if len(assignments) != 1:
        raise ValueError(f"expected one __version__ declaration in {path}")
    value = ast.literal_eval(assignments[0])
    python_version(value)
    return value


def check_versions(expected: str, declarations: dict[str, str]) -> None:
    python_version(expected)
    mismatches = {name: value for name, value in declarations.items() if value != expected}
    if mismatches:
        raise ValueError(f"release versions differ: expected {expected}, got {mismatches}")


def validate_parallel_projects(expected: str, python_project: dict, companion: dict) -> None:
    """Keep the optional Python wheel and its PyPI extra tied to this release."""
    normalized = python_version(expected)
    python_extras = python_project.get("optional-dependencies", {})
    expected_extra = [f"pillow-rs-parallel=={normalized}"]
    if python_extras.get("parallel") != expected_extra:
        raise ValueError(
            "Python parallel extra must pin the matching pillow-rs-parallel "
            f"wheel: expected {expected_extra}"
        )

    package = companion.get("project", {})
    if package.get("name") != "pillow-rs-parallel":
        raise ValueError("parallel companion project must be named pillow-rs-parallel")
    if package.get("readme") != "README.md":
        raise ValueError("parallel companion project must publish its README")
    if package.get("dynamic") != ["version"]:
        raise ValueError("parallel companion version must be inherited from its Cargo manifest")
    if package.get("dependencies") != [f"pillow-rs=={normalized}"]:
        raise ValueError("parallel companion must depend on the matching pillow-rs wheel")

    maturin = companion.get("tool", {}).get("maturin", {})
    if maturin.get("manifest-path") != "../pillow-rs-py/Cargo.toml":
        raise ValueError("parallel companion must build the shared Python binding crate")
    if maturin.get("module-name") != "pillow_rs._core_parallel":
        raise ValueError("parallel companion must expose the distinct _core_parallel module")
    if "parallel-wheel" not in maturin.get("features", []):
        raise ValueError("parallel companion must enable the parallel-wheel Cargo feature")


def repository_versions(root: Path = ROOT) -> tuple[str, dict[str, str]]:
    # Release preparation uses Python 3.12. Keep conversion/import helpers
    # dependency-free for the isolated wheel consumer and publishing jobs.
    import tomllib

    def toml(relative: str) -> dict:
        return tomllib.loads((root / relative).read_text())

    workspace = toml("Cargo.toml")["workspace"]
    expected = workspace["package"]["version"]
    declarations = {"Cargo workspace": expected}
    lock = toml("Cargo.lock")["package"]
    for member in workspace["members"]:
        package = toml(f"{member}/Cargo.toml")["package"]
        if package["version"] != {"workspace": True}:
            raise ValueError(f"{member} must inherit the workspace version")
        locked = [item["version"] for item in lock
                  if item["name"] == package["name"] and "source" not in item]
        if len(locked) != 1:
            raise ValueError(f"expected one workspace lock entry for {member}")
        declarations[f"Cargo.lock {member}"] = locked[0]
    declarations["Python manifest"] = toml("pillow-rs-py/pyproject.toml")["project"]["version"]
    validate_parallel_projects(
        expected,
        toml("pillow-rs-py/pyproject.toml")["project"],
        toml("pillow-rs-parallel/pyproject.toml"),
    )
    declarations["Python runtime"] = runtime_version(root)
    declarations["npm manifest"] = json.loads((root / "pillow-rs-js/package.json").read_text())["version"]
    npm_lock = json.loads((root / "pillow-rs-js/package-lock.json").read_text())
    declarations["npm lock"] = npm_lock["version"]
    declarations["npm root package lock"] = npm_lock["packages"][""]["version"]
    declarations["documentation"] = json.loads((root / "documentation.json").read_text())["version"]
    return expected, declarations


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="expected version or v-prefixed release tag")
    args = parser.parse_args()
    expected, declarations = repository_versions()
    if args.version is not None:
        expected = args.version.removeprefix("v")
    check_versions(expected, declarations)
    print(f"All {len(declarations)} release declarations agree: {expected}")


if __name__ == "__main__":
    main()

"""Bind Maturin's materialized Python extension to a fresh Cargo build witness.

Cargo's raw cdylib and the extension Maturin places in the Python package are
separate artifacts. This receipt records both stages without pretending that
Maturin's platform-specific load-command or RPATH rewriting preserves the raw
Cargo byte hash.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

try:
    import migration_build_witness as build_witness
except ModuleNotFoundError:
    from scripts import migration_build_witness as build_witness


SCHEMA = "pillow-rs/build-witness@2"
MATERIALIZATION_SCHEMA = "pillow-rs/maturin-materialization@1"


def _fingerprint(path: Path, root: Path, *, label: str) -> dict[str, Any]:
    resolved = path.resolve(strict=True)
    if not resolved.is_file():
        raise ValueError(f"{label} is not a regular file")
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"{label} escapes the workspace") from exc
    content = resolved.read_bytes()
    return {
        "path": str(resolved),
        "size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def _log_fingerprint(path: Path, root: Path) -> dict[str, Any]:
    resolved = path.resolve(strict=True)
    if not resolved.is_file():
        raise ValueError("Maturin log is not a regular file")
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValueError("Maturin log escapes the workspace") from exc
    content = resolved.read_bytes()
    return {
        "path": str(resolved),
        "size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def _configured_features(root: Path) -> tuple[Path, list[str]]:
    path = root / "pillow-rs-py" / "pyproject.toml"
    section = ""
    features = None
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.partition("#")[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line
            continue
        if section == "[tool.maturin]" and re.match(r"^features\s*=", line):
            if features is not None:
                raise ValueError("Maturin pyproject configuration repeats its feature key")
            value = line.partition("=")[2].strip()
            try:
                features = ast.literal_eval(value)
            except (SyntaxError, ValueError) as exc:
                raise ValueError("Maturin pyproject feature list is unsupported") from exc
    if not isinstance(features, list) or any(not isinstance(item, str) for item in features):
        raise ValueError("Maturin pyproject configuration has no valid feature list")
    normalized = sorted(set(features))
    if len(normalized) != len(features):
        raise ValueError("Maturin pyproject configuration repeats features")
    if normalized != ["pyo3/extension-module"]:
        raise ValueError("Maturin pyproject features differ from the supported extension configuration")
    return path, normalized


def _run_version(
    python: Path,
    root: Path,
    environment: dict[str, str],
    maturin_executable: Path | None,
) -> str:
    command = (
        [str(maturin_executable), "--version"]
        if maturin_executable is not None
        else [str(python), "-m", "maturin", "--version"]
    )
    completed = subprocess.run(
        command,
        cwd=root,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    version = completed.stdout.strip()
    if completed.returncode != 0 or not version:
        raise ValueError("could not identify the Maturin version")
    return version


def _features(argv: list[str], option: str) -> set[str]:
    values: set[str] = set()
    index = 0
    while index < len(argv):
        argument = argv[index]
        if argument == option:
            if index + 1 >= len(argv):
                raise ValueError(f"{option} has no value")
            raw = argv[index + 1]
            index += 2
        elif argument.startswith(option + "="):
            raw = argument.partition("=")[2]
            index += 1
        else:
            index += 1
            continue
        for feature in raw.split(","):
            if not feature or feature in values:
                raise ValueError("build command features are empty or duplicated")
            values.add(feature)
    return values


def build_materialization_witness(
    *,
    root: Path,
    cargo_witness_path: Path,
    output_path: Path,
    maturin_log_path: Path,
    module_path: Path,
    maturin_python: Path,
    maturin_executable: Path | None = None,
    maturin_pip_path: Path | None = None,
    maturin_argv: list[str],
    maturin_exit_code: int,
    parallel: bool,
) -> dict[str, Any]:
    workspace = root.expanduser().resolve(strict=True)
    cargo_witness = json.loads(cargo_witness_path.read_text(encoding="utf-8"))
    if cargo_witness.get("schema") != build_witness.SCHEMA:
        raise ValueError("Cargo build witness has an unsupported schema")
    if cargo_witness.get("status") != "complete" or cargo_witness.get("errors") != []:
        raise ValueError("Cargo build witness is not complete")
    source = cargo_witness.get("source")
    if not isinstance(source, dict) or source.get("before") != source.get("after"):
        raise ValueError("Cargo build witness does not prove a stable source snapshot")
    current_source = build_witness.capture_source_snapshot(workspace)
    if current_source != source["after"]:
        raise ValueError("source changed between the Cargo and Maturin builds")

    build = cargo_witness.get("build")
    artifact = cargo_witness.get("artifact")
    if not isinstance(build, dict) or not isinstance(artifact, dict):
        raise ValueError("Cargo build witness is missing build or artifact facts")
    raw_sha = artifact.get("sha256")
    raw_path = artifact.get("path")
    if not isinstance(raw_sha, str) or not isinstance(raw_path, str):
        raise ValueError("Cargo build witness has an invalid raw artifact record")
    if build.get("features", {}).get("pillow-rs-py") is None:
        raise ValueError("Cargo build witness has no binding feature record")
    cargo_parallel = "parallel" in build["features"].get("pillow-rs-py", [])
    if cargo_parallel != parallel:
        raise ValueError("Cargo build feature profile differs from the requested Maturin profile")

    maturin_python = maturin_python.expanduser().absolute()
    if not maturin_python.is_file():
        raise ValueError("selected Maturin Python executable is missing")
    resolved_maturin_executable = None
    if maturin_executable is not None:
        resolved_maturin_executable = maturin_executable.expanduser().resolve(strict=True)
        if not resolved_maturin_executable.is_file():
            raise ValueError("selected Maturin executable is not a regular file")
        if not os.access(resolved_maturin_executable, os.X_OK):
            raise ValueError("selected Maturin executable is not executable")
    resolved_maturin_pip_path = None
    if maturin_pip_path is not None:
        try:
            resolved_maturin_pip_path = maturin_pip_path.expanduser().resolve(strict=True)
        except OSError as exc:
            raise ValueError("selected Maturin pip executable is missing") from exc
        if not resolved_maturin_pip_path.is_file():
            raise ValueError("selected Maturin pip executable is not a regular file")
        if not os.access(resolved_maturin_pip_path, os.X_OK):
            raise ValueError("selected Maturin pip executable is not executable")
    environment = os.environ.copy()
    environment["PYO3_PYTHON"] = str(maturin_python)
    captured_environment = {
        key: environment.get(key) for key in build_witness.CAPTURED_ENVIRONMENT
    }
    if captured_environment != build.get("environment"):
        raise ValueError("Cargo and Maturin build environments differ")
    version = _run_version(
        maturin_python, workspace, environment, resolved_maturin_executable
    )
    config_path, config_features = _configured_features(workspace)
    expected_cargo_features = set(config_features)
    expected_maturin_features: set[str] = set()
    if parallel:
        expected_cargo_features.add("parallel")
        expected_maturin_features.add("parallel")
    if _features(build.get("argv", []), "--features") != expected_cargo_features:
        raise ValueError("Cargo features differ from Maturin configuration and target profile")
    config_relative_path = config_path.relative_to(workspace).as_posix()
    source_config = next(
        (
            row
            for row in source["after"].get("files", [])
            if row.get("path") == config_relative_path
        ),
        None,
    )
    config_sha256 = hashlib.sha256(config_path.read_bytes()).hexdigest()
    if source_config is None or source_config.get("sha256") != config_sha256:
        raise ValueError("Maturin configuration is not bound by the Cargo source snapshot")

    expected_command = (
        [str(resolved_maturin_executable), "develop"]
        if resolved_maturin_executable is not None
        else [str(maturin_python), "-m", "maturin", "develop"]
    )
    expected_command.extend(
        [
            "--skip-install",
            "--manifest-path",
            "pillow-rs-py/Cargo.toml",
            "--release",
            "--locked",
        ]
    )
    if resolved_maturin_pip_path is not None:
        expected_command.extend(["--pip-path", str(resolved_maturin_pip_path)])
    if parallel:
        expected_command.extend(["--features", "parallel"])
    if maturin_argv != expected_command:
        raise ValueError("recorded Maturin argv differs from the invoked materialization command")
    if _features(maturin_argv, "--features") != expected_maturin_features:
        raise ValueError("Maturin CLI features differ from the target profile")

    module = module_path if module_path.is_absolute() else workspace / module_path
    cargo_after = Path(raw_path)
    materialization: dict[str, Any] = {
        "schema": MATERIALIZATION_SCHEMA,
        "status": "not_proven",
        "argv": list(maturin_argv),
        "cwd": str(workspace),
        "environment": {
            key: build.get("environment", {}).get(key)
            for key in build_witness.CAPTURED_ENVIRONMENT
        },
        "version": version,
        "exit_code": maturin_exit_code,
        "log": _log_fingerprint(maturin_log_path, workspace),
        "config": {
            "path": str(config_path.resolve()),
            "sha256": config_sha256,
            "features": config_features,
        },
        "raw_cargo_artifact_sha256": raw_sha,
        "cargo_artifact_after": None,
        "module_artifact": None,
    }
    errors: list[str] = []
    try:
        if maturin_exit_code != 0:
            raise ValueError(f"Maturin exited with status {maturin_exit_code}")
        materialization["cargo_artifact_after"] = {
            **artifact,
            **_fingerprint(cargo_after, workspace, label="post-Maturin Cargo artifact"),
        }
        materialization["module_artifact"] = _fingerprint(
            module, workspace, label="Maturin Python module"
        )
        if materialization["module_artifact"]["sha256"] == raw_sha:
            # Equality is valid; record it without requiring platform-specific
            # post-processing to produce a different digest.
            pass
        materialization["status"] = "complete"
    except (OSError, RuntimeError, ValueError) as exc:
        errors.append(str(exc))

    result = {
        "schema": SCHEMA,
        "status": "complete" if not errors else "not_proven",
        "workspace": cargo_witness["workspace"],
        "source": source,
        "build": build,
        "artifact": artifact,
        "materialization": materialization,
        "errors": errors,
    }
    if output_path.exists() or output_path.is_symlink():
        raise FileExistsError(f"materialization witness already exists: {output_path}")
    build_witness._publish(output_path, result)
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--cargo-witness", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--maturin-log", type=Path, required=True)
    parser.add_argument("--module", type=Path, required=True)
    parser.add_argument("--maturin-python", type=Path, required=True)
    parser.add_argument(
        "--maturin-executable",
        type=Path,
        help="use an existing standalone Maturin binary instead of `python -m maturin`",
    )
    parser.add_argument(
        "--maturin-pip-path",
        type=Path,
        help="bind Maturin's explicit --pip-path when the selected Python has no pip",
    )
    parser.add_argument("--maturin-exit-code", type=int, required=True)
    parser.add_argument("--parallel", action="store_true")
    parser.add_argument("maturin_command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    command = list(args.maturin_command)
    if command and command[0] == "--":
        command.pop(0)
    try:
        result = build_materialization_witness(
            root=args.root,
            cargo_witness_path=args.cargo_witness,
            output_path=args.output,
            maturin_log_path=args.maturin_log,
            module_path=args.module,
            maturin_python=args.maturin_python,
            maturin_executable=args.maturin_executable,
            maturin_pip_path=args.maturin_pip_path,
            maturin_argv=command,
            maturin_exit_code=args.maturin_exit_code,
            parallel=args.parallel,
        )
    except (OSError, RuntimeError, TypeError, ValueError) as exc:
        parser.error(str(exc))
    print(json.dumps({"status": result["status"], "output": str(args.output), "errors": result["errors"]}, sort_keys=True))
    return 0 if result["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())

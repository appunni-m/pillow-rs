"""Capture a fresh Cargo build and the on-disk source closure that produced it.

The witness binds observed files, command arguments, selected environment
values, Cargo JSON events, and the emitted cdylib. It does not prove which
compiler bytes were loaded into memory or attest a compiler's internal state.
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


SCHEMA = "pillow-rs/build-witness@1"
FEATURE_PACKAGES = ("pillow-rs", "pillow-rs-py")
CAPTURED_ENVIRONMENT = (
    "CARGO_TARGET_DIR",
    "RUSTC_WRAPPER",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "PYO3_PYTHON",
    "CARGO_BUILD_TARGET",
)
_DYNAMIC_LIBRARY_SUFFIXES = (".so", ".dylib", ".dll", ".pyd", ".bundle")
_PACKAGE_ID_NAME = re.compile(r"^([A-Za-z0-9_.-]+)(?:@|\s)")


def _canonical_sha256(value: Any) -> str:
    payload = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _normalized_logical_path(path: Path, root: Path) -> Path:
    normalized = Path(os.path.normpath(str(path.absolute())))
    if not _is_within(normalized, root):
        raise ValueError(f"source path escapes workspace: {path}")
    return normalized


def _rust_string_at(source: str, start: int) -> tuple[str, int] | None:
    """Return a literal Rust path string and its end offset, if present."""
    index = start
    if source.startswith("br", index):
        index += 1
    if source.startswith("r", index):
        index += 1
        hash_start = index
        while index < len(source) and source[index] == "#":
            index += 1
        hashes = source[hash_start:index]
        if index >= len(source) or source[index] != '"':
            return None
        content_start = index + 1
        closing = '"' + hashes
        content_end = source.find(closing, content_start)
        if content_end < 0:
            raise ValueError("unterminated raw include path literal")
        value = source[content_start:content_end]
        return value, content_end + len(closing)

    if source.startswith('b"', start):
        index = start + 1
    elif start < len(source) and source[start] == '"':
        index = start
    else:
        return None

    quote = index
    cursor = quote + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == '"':
            token = source[start:cursor + 1]
            try:
                parsed = ast.literal_eval(token)
            except (SyntaxError, ValueError) as exc:
                raise ValueError("unsupported escaped include path literal") from exc
            if isinstance(parsed, bytes):
                try:
                    parsed = parsed.decode("utf-8")
                except UnicodeDecodeError as exc:
                    raise ValueError("non-UTF-8 byte include path") from exc
            if not isinstance(parsed, str):
                raise ValueError("include path literal is not text")
            return parsed, cursor + 1
        cursor += 1
    raise ValueError("unterminated include path literal")


def _skip_rust_literal(source: str, start: int) -> int | None:
    try:
        parsed = _rust_string_at(source, start)
    except ValueError:
        # A malformed literal will also make Cargo fail. Keep scanning so the
        # eventual build result can be retained as an incomplete witness.
        return None
    return parsed[1] if parsed is not None else None


def _literal_include_paths(source: str) -> list[str]:
    """Find direct include_str!/include_bytes! string arguments in Rust code."""
    found: list[str] = []
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = length if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            index += 2
            depth = 1
            while index < length and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            continue

        literal_start = index
        if source.startswith("br", index):
            literal_start = index
        elif source.startswith("b\"", index):
            literal_start = index
        elif source.startswith("r", index) and (index + 1 == length or source[index + 1] in '#"'):
            literal_start = index
        elif source[index] == '"':
            literal_start = index
        else:
            literal_start = -1
        if literal_start >= 0:
            end = _skip_rust_literal(source, literal_start)
            index = end if end is not None else index + 1
            continue

        if source.startswith("include_str!", index) or source.startswith("include_bytes!", index):
            previous = source[index - 1] if index else " "
            after = index + (len("include_str!") if source.startswith("include_str!", index) else len("include_bytes!"))
            if (previous.isalnum() or previous == "_"):
                index += 1
                continue
            while after < length and source[after].isspace():
                after += 1
            if after >= length or source[after] != "(":
                index += 1
                continue
            argument = after + 1
            while argument < length and source[argument].isspace():
                argument += 1
            parsed = _rust_string_at(source, argument)
            if parsed is not None:
                value, end = parsed
                close = end
                while close < length and source[close].isspace():
                    close += 1
                if close < length and source[close] == ",":
                    close += 1
                    while close < length and source[close].isspace():
                        close += 1
                if close < length and source[close] == ")":
                    found.append(value)
                    index = close + 1
                    continue
        index += 1
    return found


def capture_source_snapshot(root: Path) -> dict[str, Any]:
    """Hash workspace manifests and all Rust package source/build inputs.

    Every selected path is recorded relative to ``root``. Internal symlinks
    are followed and content-hashed; a symlink resolving outside the workspace
    is rejected. Literal direct include_str!/include_bytes! paths discovered
    in Rust source and build scripts are added even when outside ``src``.
    """
    workspace = Path(root).expanduser().resolve(strict=True)
    if not workspace.is_dir():
        raise ValueError("workspace root is not a directory")

    selected: dict[str, Path] = {}
    active_directories: set[Path] = set()

    def add_file(logical_path: Path) -> Path:
        logical = _normalized_logical_path(logical_path, workspace)
        try:
            resolved = logical.resolve(strict=True)
        except OSError as exc:
            raise ValueError(f"missing source input: {logical}") from exc
        if not _is_within(resolved, workspace):
            raise ValueError(f"source symlink escapes workspace: {logical}")
        if not resolved.is_file():
            raise ValueError(f"source input is not a regular file: {logical}")
        relative = logical.relative_to(workspace).as_posix()
        selected[relative] = resolved
        return resolved

    def add_tree(logical_directory: Path) -> None:
        logical = _normalized_logical_path(logical_directory, workspace)
        try:
            resolved_directory = logical.resolve(strict=True)
        except OSError as exc:
            raise ValueError(f"missing source directory: {logical}") from exc
        if not _is_within(resolved_directory, workspace):
            raise ValueError(f"source symlink escapes workspace: {logical}")
        if not resolved_directory.is_dir():
            raise ValueError(f"source tree path is not a directory: {logical}")
        if resolved_directory in active_directories:
            raise ValueError(f"source tree contains a directory symlink cycle: {logical}")
        active_directories.add(resolved_directory)
        try:
            for entry in sorted(logical.iterdir(), key=lambda item: item.name):
                entry_resolved = entry.resolve(strict=True)
                if not _is_within(entry_resolved, workspace):
                    raise ValueError(f"source symlink escapes workspace: {entry}")
                if entry_resolved.is_dir():
                    add_tree(entry)
                elif entry_resolved.is_file():
                    add_file(entry)
                else:
                    raise ValueError(f"unsupported source tree entry: {entry}")
        finally:
            active_directories.remove(resolved_directory)

    required = [workspace / "Cargo.toml", workspace / "Cargo.lock"]
    for path in (workspace / "rust-toolchain.toml", workspace / "rust-toolchain"):
        if path.exists():
            required.append(path)
    for config in (workspace / ".cargo" / "config.toml", workspace / ".cargo" / "config"):
        if config.exists():
            required.append(config)
    for path in required:
        add_file(path)

    package_roots = ("pillow-rs", "pillow-rs-py", "pillow-rs-js")
    rust_sources: set[str] = set()
    root_build = workspace / "build.rs"
    if root_build.exists():
        add_file(root_build)
        rust_sources.add(root_build.relative_to(workspace).as_posix())
    root_source_tree = workspace / "src"
    if root_source_tree.exists():
        add_tree(root_source_tree)
        rust_sources.update(
            name for name in selected
            if name.startswith("src/") and name.endswith(".rs")
        )
    for package in package_roots:
        package_root = workspace / package
        add_file(package_root / "Cargo.toml")
        build_script = package_root / "build.rs"
        if build_script.exists():
            add_file(build_script)
            rust_sources.add(build_script.relative_to(workspace).as_posix())
        source_tree = package_root / "src"
        add_tree(source_tree)
        rust_sources.update(
            name for name in selected
            if name.startswith(source_tree.relative_to(workspace).as_posix() + "/")
            and name.endswith(".rs")
        )

    scanned: set[str] = set()
    pending = sorted(rust_sources)
    while pending:
        relative = pending.pop(0)
        if relative in scanned:
            continue
        scanned.add(relative)
        resolved = selected.get(relative)
        if resolved is None:
            resolved = add_file(workspace / relative)
        try:
            text = resolved.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as exc:
            raise ValueError(f"cannot read Rust source input: {relative}: {exc}") from exc
        for include in _literal_include_paths(text):
            include_path = Path(include)
            logical_include = include_path if include_path.is_absolute() else workspace / relative
            if not include_path.is_absolute():
                logical_include = logical_include.parent / include_path
            add_file(logical_include)
            include_relative = _normalized_logical_path(logical_include, workspace).relative_to(workspace).as_posix()
            if include_relative.endswith(".rs") and include_relative not in scanned:
                pending.append(include_relative)

    files = []
    for relative, path in sorted(selected.items()):
        content = path.read_bytes()
        files.append({"path": relative, "size": len(content), "sha256": hashlib.sha256(content).hexdigest()})
    return {"sha256": _canonical_sha256(files), "files": files}


def _capture_output(argv: list[str], *, cwd: Path, environment: dict[str, str]) -> tuple[int | None, bytes, str | None]:
    try:
        completed = subprocess.run(
            argv,
            cwd=str(cwd),
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
    except (OSError, ValueError) as exc:
        return None, b"", str(exc)
    output = completed.stdout or b""
    if isinstance(output, str):
        output = output.encode("utf-8", errors="replace")
    return completed.returncode, output, None


def _cargo_package_name(package_id: Any) -> str | None:
    if not isinstance(package_id, str):
        return None
    if "#" in package_id:
        fragment = package_id.rsplit("#", 1)[1]
        return fragment.split("@", 1)[0] or None
    match = _PACKAGE_ID_NAME.match(package_id)
    return match.group(1) if match else None


def _cargo_message_package(message: dict[str, Any], root: Path | None) -> str | None:
    """Identify a workspace package by Cargo's exact manifest path when present.

    Modern Cargo path package IDs contain a source URL and version fragment,
    for example ``path+file:///workspace/pillow-rs#12.2.1``. That fragment is
    not the package name. The manifest path is the stable identity within this
    captured workspace. Older Cargo event shapes without it retain the legacy
    package-ID parser.
    """
    if "manifest_path" in message:
        manifest_value = message.get("manifest_path")
        if root is None or not isinstance(manifest_value, str) or not manifest_value:
            return None
        manifest = Path(manifest_value)
        if not manifest.is_absolute():
            manifest = root / manifest
        try:
            resolved_manifest = manifest.resolve(strict=True)
        except OSError:
            return None
        for package in FEATURE_PACKAGES:
            try:
                expected = (root / package / "Cargo.toml").resolve(strict=True)
            except OSError:
                continue
            if resolved_manifest == expected:
                return package
        # Do not let a misleading package ID override a present but unknown
        # manifest path.
        return None
    return _cargo_package_name(message.get("package_id"))


def _target_from_argv(argv: list[str], environment: dict[str, str], rustc_output: str | None) -> str | None:
    targets: list[str] = []
    index = 0
    while index < len(argv):
        token = argv[index]
        if token == "--target":
            if index + 1 < len(argv):
                targets.append(argv[index + 1])
            index += 2
            continue
        if token.startswith("--target="):
            targets.append(token.partition("=")[2])
        index += 1
    if len(targets) == 1 and targets[0]:
        return targets[0]
    if len(targets) > 1:
        return None
    configured = environment.get("CARGO_BUILD_TARGET")
    if configured:
        return configured
    if rustc_output:
        for line in rustc_output.splitlines():
            if line.startswith("host:"):
                return line.partition(":")[2].strip() or None
    return None


def _validate_build_command(argv: list[str], root: Path) -> list[str]:
    if len(argv) < 2 or Path(argv[0]).name not in {"cargo", "cargo.exe"} or argv[1] != "build":
        raise ValueError("command must be a Cargo build")
    if "--release" not in argv or "--locked" not in argv:
        raise ValueError("command must include --release and --locked")

    manifest_values: list[str] = []
    message_formats: list[str] = []
    index = 2
    while index < len(argv):
        token = argv[index]
        if token == "--manifest-path":
            if index + 1 >= len(argv):
                raise ValueError("--manifest-path needs a value")
            manifest_values.append(argv[index + 1])
            index += 2
            continue
        if token.startswith("--manifest-path="):
            manifest_values.append(token.partition("=")[2])
        if token == "--message-format":
            if index + 1 >= len(argv):
                raise ValueError("--message-format needs a value")
            message_formats.append(argv[index + 1])
            index += 2
            continue
        if token.startswith("--message-format="):
            message_formats.append(token.partition("=")[2])
        index += 1

    if len(manifest_values) != 1:
        raise ValueError("command must bind exactly one --manifest-path")
    manifest = Path(manifest_values[0])
    if not manifest.is_absolute():
        manifest = root / manifest
    try:
        resolved_manifest = manifest.resolve(strict=True)
    except OSError as exc:
        raise ValueError("Cargo manifest path does not exist") from exc
    expected_manifest = (root / "pillow-rs-py" / "Cargo.toml").resolve(strict=True)
    if resolved_manifest != expected_manifest:
        raise ValueError("Cargo manifest must be workspace pillow-rs-py/Cargo.toml")
    if len(message_formats) != 1 or not message_formats[0].startswith("json"):
        raise ValueError("command must select exactly one JSON Cargo message format")
    if "--" in argv:
        raise ValueError("build script arguments are not accepted")
    targets: list[str] = []
    index = 2
    while index < len(argv):
        token = argv[index]
        if token == "--target":
            if index + 1 >= len(argv) or not argv[index + 1]:
                raise ValueError("--target needs a non-empty value")
            targets.append(argv[index + 1])
            index += 2
            continue
        if token.startswith("--target="):
            target = token.partition("=")[2]
            if not target:
                raise ValueError("--target needs a non-empty value")
            targets.append(target)
        index += 1
    if len(targets) > 1:
        raise ValueError("command must select at most one Cargo target")
    return message_formats


def _parse_cargo_output(output: bytes, *, errors: list[str], root: Path | None = None) -> tuple[
    dict[str, list[str]], list[dict[str, Any]], list[bool], list[str]
]:
    features: dict[str, list[str]] = {package: [] for package in FEATURE_PACKAGES}
    feature_observed: set[str] = set()
    fresh_values: dict[str, list[bool]] = {package: [] for package in FEATURE_PACKAGES}
    artifact_events: list[dict[str, Any]] = []
    build_finished: list[bool] = []

    for raw_line in output.splitlines():
        line = raw_line.strip()
        if not line.startswith(b"{"):
            continue
        try:
            message = json.loads(line.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError):
            errors.append("Cargo emitted an invalid JSON message line")
            continue
        if not isinstance(message, dict):
            continue
        reason = message.get("reason")
        if reason == "build-finished":
            success = message.get("success")
            if type(success) is not bool:
                errors.append("Cargo build-finished message has no boolean success")
            else:
                build_finished.append(success)
            continue
        if reason != "compiler-artifact":
            continue
        package = _cargo_message_package(message, root)
        target = message.get("target")
        crate_types = target.get("crate_types") if isinstance(target, dict) else None
        core_artifact = (
            package == "pillow-rs"
            and isinstance(target, dict)
            and target.get("name") == "pillow_rs"
            and isinstance(crate_types, list)
            and "lib" in crate_types
        ) or (
            package == "pillow-rs-py"
            and isinstance(target, dict)
            and target.get("name") == "_core"
            and isinstance(crate_types, list)
            and "cdylib" in crate_types
        )
        if core_artifact:
            fresh = message.get("fresh")
            if type(fresh) is not bool:
                errors.append(f"Cargo compiler-artifact lacks boolean freshness for {package}")
            else:
                fresh_values[package].append(fresh)
        if package in FEATURE_PACKAGES:
            raw_features = message.get("features")
            if (
                not isinstance(raw_features, list)
                or any(not isinstance(item, str) or not item for item in raw_features)
            ):
                errors.append(f"Cargo compiler-artifact lacks feature list for {package}")
            else:
                normalized = sorted(set(raw_features))
                if len(normalized) != len(raw_features):
                    errors.append(f"Cargo compiler-artifact repeats features for {package}")
                prior = features[package] if package in feature_observed else None
                if prior is not None and prior != normalized:
                    errors.append(f"Cargo compiler-artifact feature lists differ for {package}")
                features[package] = normalized
                feature_observed.add(package)
        artifact_events.append(message)

    for package in FEATURE_PACKAGES:
        if package not in feature_observed:
            errors.append(f"Cargo compiler-artifact did not report features for {package}")
        values = fresh_values[package]
        if not values:
            errors.append(f"Cargo compiler-artifact did not report core artifact freshness for {package}")
        elif len(values) != 1:
            errors.append(f"Cargo reported ambiguous core artifact freshness for {package}")
        elif values[0]:
            errors.append(f"Cargo reused the {package} core artifact")
    if len(build_finished) != 1 or build_finished != [True]:
        errors.append("Cargo did not report exactly one successful build-finished event")
    fresh_packages = sorted(package for package, values in fresh_values.items() if values == [False])
    return features, artifact_events, build_finished, fresh_packages


def _find_core_cdylib(
    events: list[dict[str, Any]], errors: list[str], *, root: Path | None = None
) -> dict[str, Any] | None:
    candidates: list[tuple[dict[str, Any], Path]] = []
    for event in events:
        if _cargo_message_package(event, root) != "pillow-rs-py":
            continue
        target = event.get("target")
        if not isinstance(target, dict) or target.get("name") != "_core":
            continue
        crate_types = target.get("crate_types")
        if not isinstance(crate_types, list) or "cdylib" not in crate_types:
            continue
        filenames = event.get("filenames")
        if not isinstance(filenames, list):
            errors.append("_core cdylib compiler-artifact has no filenames")
            continue
        dynamic_paths = []
        for value in filenames:
            if not isinstance(value, str):
                continue
            filename = Path(value).name
            suffix = next((item for item in _DYNAMIC_LIBRARY_SUFFIXES if filename.endswith(item)), None)
            base = filename[:-len(suffix)] if suffix else ""
            if base in {"_core", "lib_core"} or base.startswith(("_core.", "lib_core.")):
                dynamic_paths.append(Path(value))
        for path in dynamic_paths:
            candidates.append((event, path))

    if len(candidates) != 1:
        errors.append(f"expected one Cargo-reported _core cdylib, found {len(candidates)}")
        return None
    event, path = candidates[0]
    try:
        resolved = path.resolve(strict=True)
        if not resolved.is_file():
            raise OSError("artifact is not a regular file")
        content = resolved.read_bytes()
    except OSError as exc:
        errors.append(f"cannot hash Cargo-reported _core cdylib: {exc}")
        return None
    target = event["target"]
    return {
        "package_id": event["package_id"],
        "target": target["name"],
        "crate_types": sorted(set(target["crate_types"])),
        "path": str(resolved),
        "size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def _git_workspace_state(root: Path, environment: dict[str, str], errors: list[str]) -> tuple[str | None, bool | None]:
    head_code, head_output, head_error = _capture_output(
        ["git", "rev-parse", "HEAD"], cwd=root, environment=environment
    )
    status_code, status_output, status_error = _capture_output(
        ["git", "status", "--porcelain", "--untracked-files=normal"],
        cwd=root,
        environment=environment,
    )
    if head_error or head_code != 0:
        errors.append(f"cannot capture git HEAD: {head_error or head_output.decode('utf-8', errors='replace').strip()}")
        head = None
    else:
        head = head_output.decode("utf-8", errors="replace").strip() or None
    if status_error or status_code != 0:
        errors.append(f"cannot capture git dirty state: {status_error or status_output.decode('utf-8', errors='replace').strip()}")
        dirty = None
    else:
        dirty = bool(status_output.strip())
    return head, dirty


def _tool_version(command: list[str], root: Path, environment: dict[str, str], errors: list[str], label: str) -> str | None:
    code, output, error = _capture_output(command, cwd=root, environment=environment)
    if error or code != 0:
        errors.append(f"cannot capture {label} version: {error or output.decode('utf-8', errors='replace').strip()}")
        return None
    return output.decode("utf-8", errors="replace").strip() or None


def _reserve_log(path: Path) -> Any:
    path.parent.mkdir(parents=True, exist_ok=True)
    return path.open("xb")


def _publish(path: Path, document: dict[str, Any]) -> None:
    try:
        from migration_execution_attestation import _publish as publish
    except ModuleNotFoundError:
        from scripts.migration_execution_attestation import _publish as publish
    publish(path, document)


def run_build_witness(
    root: Path,
    output: Path,
    argv: list[str],
    env_overrides: dict[str, str | None],
) -> dict[str, Any]:
    """Run and record a release Cargo build for the Python ``_core`` cdylib.

    ``env_overrides`` uses string values to set variables and ``None`` to unset
    them. Only the documented build variables inherited from the caller and
    explicitly supplied overrides are included in the witness.
    """
    root_path = Path(root).expanduser().resolve(strict=True)
    if not root_path.is_dir():
        raise ValueError("workspace root is not a directory")
    output_path = Path(output).expanduser().absolute()
    if not isinstance(argv, list) or any(not isinstance(token, str) for token in argv):
        raise TypeError("argv must be a list of strings")
    if not isinstance(env_overrides, dict):
        raise TypeError("env_overrides must be a mapping")
    if output_path.exists() or output_path.is_symlink():
        raise FileExistsError(f"build witness already exists: {output_path}")
    overrides: dict[str, str | None] = {}
    for key, value in env_overrides.items():
        if not isinstance(key, str) or not key or "=" in key or "\x00" in key:
            raise ValueError("invalid environment variable name")
        if value is not None and (not isinstance(value, str) or "\x00" in value):
            raise ValueError(f"invalid environment override for {key}")
        overrides[key] = value

    command_environment = os.environ.copy()
    for key, value in overrides.items():
        if value is None:
            command_environment.pop(key, None)
        else:
            command_environment[key] = value
    captured_environment: dict[str, str | None] = {
        key: command_environment.get(key) for key in CAPTURED_ENVIRONMENT
    }
    for key, value in overrides.items():
        captured_environment[key] = value

    document: dict[str, Any] = {
        "schema": SCHEMA,
        "status": "not_proven",
        "workspace": {"root": str(root_path), "git_head": None, "dirty": None},
        "source": {"before": None, "after": None},
        "build": {
            "argv": list(argv),
            "cwd": str(root_path),
            "environment": captured_environment,
            "cargo_version": None,
            "rustc_version": None,
            "target_triple": None,
            "exit_code": None,
            "log": None,
            "features": {package: [] for package in FEATURE_PACKAGES},
            "fresh_packages": [],
        },
        "artifact": None,
        "errors": [],
    }
    errors: list[str] = document["errors"]

    git_head, dirty = _git_workspace_state(root_path, command_environment, errors)
    document["workspace"].update(git_head=git_head, dirty=dirty)

    command_valid = True
    try:
        _validate_build_command(argv, root_path)
    except (OSError, ValueError) as exc:
        errors.append(str(exc))
        command_valid = False

    rustc_executable = command_environment.get("RUSTC", "rustc")
    cargo_version = None
    rustc_version = None
    if command_valid:
        cargo_version = _tool_version([argv[0], "--version", "--verbose"], root_path, command_environment, errors, "Cargo")
        rustc_version = _tool_version([rustc_executable, "--version", "--verbose"], root_path, command_environment, errors, "rustc")
    document["build"]["cargo_version"] = cargo_version
    document["build"]["rustc_version"] = rustc_version
    document["build"]["target_triple"] = _target_from_argv(
        argv, command_environment, rustc_version
    )
    if command_valid and document["build"]["target_triple"] is None:
        errors.append("could not determine selected target triple")

    source_ready = True
    try:
        document["source"]["before"] = capture_source_snapshot(root_path)
    except (OSError, RuntimeError, ValueError, TypeError) as exc:
        errors.append(f"cannot capture source before build: {str(exc)[:1000]}")
        source_ready = False

    log_path = output_path.with_name(output_path.name + ".log")
    log_stream = None
    try:
        log_stream = _reserve_log(log_path)
        document["build"]["log"] = {"path": str(log_path.resolve()), "size": 0, "sha256": hashlib.sha256(b"").hexdigest()}
    except OSError as exc:
        errors.append(f"cannot exclusively create build log: {exc}")

    build_output = b""
    if command_valid and source_ready and log_stream is not None:
        exit_code, build_output, run_error = _capture_output(argv, cwd=root_path, environment=command_environment)
        document["build"]["exit_code"] = exit_code
        if run_error:
            errors.append(f"Cargo build could not run: {run_error}")
        if exit_code is not None:
            features, artifact_events, build_finished, fresh_packages = _parse_cargo_output(
                build_output, errors=errors, root=root_path
            )
            document["build"]["features"] = features
            document["build"]["fresh_packages"] = fresh_packages
            if exit_code == 0 and build_finished == [True]:
                document["artifact"] = _find_core_cdylib(artifact_events, errors, root=root_path)
            else:
                if exit_code != 0:
                    errors.append(f"Cargo build exited with status {exit_code}")
    elif command_valid and not source_ready:
        errors.append("Cargo build was not started because the source snapshot is incomplete")

    if log_stream is not None:
        try:
            log_stream.write(build_output)
            log_stream.flush()
            os.fsync(log_stream.fileno())
        except OSError as exc:
            errors.append(f"cannot finish build log: {exc}")
        finally:
            log_stream.close()
        try:
            log_bytes = log_path.read_bytes()
            document["build"]["log"] = {
                "path": str(log_path.resolve()),
                "size": len(log_bytes),
                "sha256": hashlib.sha256(log_bytes).hexdigest(),
            }
        except OSError as exc:
            document["build"]["log"] = None
            errors.append(f"cannot hash build log: {exc}")

    try:
        document["source"]["after"] = capture_source_snapshot(root_path)
        if document["source"]["before"] != document["source"]["after"]:
            errors.append("source snapshot changed during the build witness")
    except (OSError, RuntimeError, ValueError, TypeError) as exc:
        errors.append(f"cannot capture source after build: {str(exc)[:1000]}")

    if (
        not errors
        and document["artifact"] is not None
        and document["build"]["fresh_packages"] == sorted(FEATURE_PACKAGES)
    ):
        document["status"] = "complete"

    _publish(output_path, document)
    return document


def _serialized_json(document: dict[str, Any]) -> bytes:
    return (
        json.dumps(document, sort_keys=True, indent=2, allow_nan=False).encode("utf-8")
        + b"\n"
    )


def _is_repairable_package_parse_error(error: Any) -> bool:
    if not isinstance(error, str):
        return False
    for package in FEATURE_PACKAGES:
        if error in {
            f"Cargo compiler-artifact did not report features for {package}",
            f"Cargo compiler-artifact did not report core artifact freshness for {package}",
        }:
            return True
    return error == "expected one Cargo-reported _core cdylib, found 0"


def _sha256_shape(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def repair_build_witness(
    original_receipt: Path,
    repaired_output: Path,
    diagnostic_output: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Reparse a retained Cargo JSON log after a package-ID parser correction.

    This narrow repair path accepts only a successful, source-stable schema@1
    receipt whose sole failures are the former Cargo package-name parsing
    errors. It does not invoke Cargo or any compiler. The retained log hash and
    size must match the receipt, and the current on-disk source closure must
    still equal the original post-build snapshot. Both outputs are exclusive
    new files; the original receipt and log are never replaced.
    """
    receipt_path = Path(original_receipt).expanduser().resolve(strict=True)
    original_bytes = receipt_path.read_bytes()
    original_hash = hashlib.sha256(original_bytes).hexdigest()
    try:
        original = json.loads(original_bytes.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise ValueError("original build witness is not valid UTF-8 JSON") from exc
    top_fields = {"schema", "status", "workspace", "source", "build", "artifact", "errors"}
    build_fields = {
        "argv", "cwd", "environment", "cargo_version", "rustc_version",
        "target_triple", "exit_code", "log", "features", "fresh_packages",
    }
    if not isinstance(original, dict) or set(original) != top_fields:
        raise ValueError("original receipt does not have the build-witness@1 shape")
    if original.get("schema") != SCHEMA or original.get("status") != "not_proven":
        raise ValueError("repair requires a not_proven build-witness@1 receipt")
    if not isinstance(original.get("workspace"), dict) or not isinstance(original.get("source"), dict):
        raise ValueError("original receipt lacks workspace or source capture")
    build = original.get("build")
    errors = original.get("errors")
    if not isinstance(build, dict) or set(build) != build_fields:
        raise ValueError("original receipt does not have the current build-witness@1 build shape")
    if original.get("artifact") is not None or not isinstance(errors, list) or not errors:
        raise ValueError("repair requires an incomplete receipt with no accepted artifact")
    if not all(_is_repairable_package_parse_error(error) for error in errors):
        raise ValueError("original receipt has failures outside the known package-ID parse defect")
    if not any(error == "expected one Cargo-reported _core cdylib, found 0" for error in errors):
        raise ValueError("original receipt does not contain the expected missing-core-artifact parse failure")
    if type(build.get("exit_code")) is not int or build["exit_code"] != 0:
        raise ValueError("repair requires an observed successful Cargo exit")
    if not isinstance(build.get("argv"), list) or any(not isinstance(item, str) for item in build["argv"]):
        raise ValueError("original receipt lacks the captured Cargo argv")
    root_value = original["workspace"].get("root")
    if not isinstance(root_value, str) or not root_value:
        raise ValueError("original receipt lacks the workspace root")
    root = Path(root_value).expanduser().resolve(strict=True)
    if str(root) != root_value or not root.is_dir():
        raise ValueError("original workspace root is no longer the captured canonical directory")
    if build.get("cwd") != str(root):
        raise ValueError("captured Cargo cwd differs from the workspace root")
    try:
        _validate_build_command(build["argv"], root)
    except (OSError, ValueError) as exc:
        raise ValueError(f"captured Cargo command is not eligible for repair: {exc}") from exc

    source = original["source"]
    if not isinstance(source, dict) or set(source) != {"before", "after"}:
        raise ValueError("original receipt lacks exact source snapshots")
    if source["before"] is None or source["after"] is None or source["before"] != source["after"]:
        raise ValueError("original receipt does not prove a stable source closure")
    current_source = capture_source_snapshot(root)
    if current_source != source["after"]:
        raise ValueError("current source closure differs from the original post-build snapshot")

    log_record = build.get("log")
    if not isinstance(log_record, dict) or set(log_record) != {"path", "size", "sha256"}:
        raise ValueError("original receipt lacks the retained Cargo log fingerprint")
    if (
        not isinstance(log_record["path"], str)
        or not log_record["path"]
        or type(log_record["size"]) is not int
        or log_record["size"] < 0
        or not _sha256_shape(log_record["sha256"])
    ):
        raise ValueError("original receipt has an invalid Cargo log fingerprint")
    log_path = Path(log_record["path"]).expanduser().resolve(strict=True)
    if not log_path.is_file():
        raise ValueError("captured Cargo log is not a regular file")
    log_bytes = log_path.read_bytes()
    if len(log_bytes) != log_record["size"] or hashlib.sha256(log_bytes).hexdigest() != log_record["sha256"]:
        raise ValueError("retained Cargo log differs from the original size or SHA-256")

    repaired_path = Path(repaired_output).expanduser().resolve(strict=False)
    diagnostic_path = Path(diagnostic_output).expanduser().resolve(strict=False)
    protected_paths = {receipt_path, log_path}
    if repaired_path == diagnostic_path:
        raise ValueError("repaired receipt and diagnostic sidecar must be separate files")
    if repaired_path in protected_paths or diagnostic_path in protected_paths:
        raise ValueError("repair outputs must not replace the original receipt or Cargo log")
    for candidate in (repaired_path, diagnostic_path):
        if candidate.exists() or candidate.is_symlink():
            raise FileExistsError(f"repair output already exists: {candidate}")

    repaired = json.loads(original_bytes.decode("utf-8"))
    repaired_errors: list[str] = []
    features, artifact_events, build_finished, fresh_packages = _parse_cargo_output(
        log_bytes, errors=repaired_errors, root=root
    )
    repaired["build"]["features"] = features
    repaired["build"]["fresh_packages"] = fresh_packages
    if build_finished == [True] and build.get("exit_code") == 0:
        repaired["artifact"] = _find_core_cdylib(artifact_events, repaired_errors, root=root)
    else:
        repaired_errors.append("retained Cargo log does not prove one successful build-finished event")
    repaired["errors"] = repaired_errors
    if (
        not repaired_errors
        and repaired["artifact"] is not None
        and fresh_packages == sorted(FEATURE_PACKAGES)
    ):
        repaired["status"] = "complete"
    else:
        repaired["status"] = "not_proven"

    producer_path = Path(__file__).resolve(strict=True)
    producer_bytes = producer_path.read_bytes()
    receipt_payload = _serialized_json(repaired)
    repaired_fingerprint = {
        "path": str(repaired_path),
        "size": len(receipt_payload),
        "sha256": hashlib.sha256(receipt_payload).hexdigest(),
    }
    log_fingerprint = {
        "path": str(log_path),
        "size": len(log_bytes),
        "sha256": hashlib.sha256(log_bytes).hexdigest(),
    }
    diagnostic = {
        "schema": "pillow-rs/build-witness-repair@1",
        "status": repaired["status"],
        "method": "reparse-retained-cargo-log-no-build",
        "original_receipt": {"path": str(receipt_path), "size": len(original_bytes), "sha256": original_hash},
        "repaired_receipt": repaired_fingerprint,
        "cargo_log": log_fingerprint,
        "source_check": {
            "workspace_root": str(root),
            "original_after_sha256": source["after"]["sha256"],
            "current_sha256": current_source["sha256"],
            "matches_original_after": current_source == source["after"],
        },
        "repair_producer": {
            "script_path": str(producer_path),
            "script_sha256": hashlib.sha256(producer_bytes).hexdigest(),
            "python_executable": sys.executable,
            "python_version": sys.version,
        },
        "limitations": [
            "The retained Cargo log was reinterpreted; Cargo and rustc were not rerun.",
            "The extension hash is read from the current on-disk Cargo-reported artifact path, not from a build-time artifact snapshot.",
            "The repair producer fields identify current on-disk script and interpreter files; they are not a signature or proof of loaded code.",
        ],
        "errors": list(repaired_errors),
    }
    _publish(repaired_path, repaired)
    _publish(diagnostic_path, diagnostic)
    return repaired, diagnostic


def _parse_environment_overrides(args: argparse.Namespace) -> dict[str, str | None]:
    values: dict[str, str | None] = {}
    for item in args.set_env:
        key, separator, value = item.partition("=")
        if not separator or not key:
            raise ValueError("--set-env requires KEY=VALUE")
        if key in values:
            raise ValueError(f"environment override repeated for {key}")
        values[key] = value
    for key in args.unset_env:
        if not key or key in values:
            raise ValueError(f"environment override repeated or invalid for {key}")
        values[key] = None
    return values


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--set-env", action="append", default=[], metavar="KEY=VALUE")
    parser.add_argument("--unset-env", action="append", default=[], metavar="KEY")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    try:
        overrides = _parse_environment_overrides(args)
        document = run_build_witness(args.root, args.output, command, overrides)
    except (OSError, TypeError, ValueError) as exc:
        parser.error(str(exc))
    print(json.dumps({"status": document["status"], "output": str(args.output), "errors": document["errors"]}, sort_keys=True))
    return 0 if document["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())

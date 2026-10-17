"""Capture a frozen inventory of the Python producer and its loaded files.

These records hash files currently on disk. They bind producer declarations to
an observed file inventory, but do not prove that loaded Python code objects
were read from those bytes or prevent a hostile replacement after capture.
The caller publishes the complete bundle as its own immutable artifact.
"""

from __future__ import annotations

from collections.abc import Mapping
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
from typing import Any


BUNDLE_SCHEMA = "migration-parity/producer-bundle@1"
IDENTITY_SCHEMA = "migration-parity/producer-identity@1"
CHILD_ENTRYPOINT = "run_migration_parity.py"
_HASH_CHUNK_BYTES = 1024 * 1024


class ProducerIdentityError(RuntimeError):
    """Raised when the producer snapshot or loaded-file identity is ambiguous."""


def _canonical_json_sha256(value: Any) -> str:
    try:
        payload = json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as exc:
        raise ProducerIdentityError(f"value is not canonical JSON: {exc}") from exc
    return hashlib.sha256(payload).hexdigest()


def _exact_object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ProducerIdentityError(f"{label} has an invalid key set")
    return value


def _nonempty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ProducerIdentityError(f"{label} must be a non-empty string")
    return value


def _digest(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or value != value.lower()
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ProducerIdentityError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _size(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        raise ProducerIdentityError(f"{label} must be a non-negative integer")
    return value


def _safe_relative_path(value: Any, label: str) -> str:
    text = _nonempty_string(value, label)
    path = PurePosixPath(text)
    if (
        path.is_absolute()
        or "\\" in text
        or path.as_posix() != text
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise ProducerIdentityError(f"{label} is not a canonical safe relative path")
    return text


def _absolute_path(value: Any, label: str) -> Path:
    text = _nonempty_string(value, label)
    path = Path(text)
    if not path.is_absolute():
        raise ProducerIdentityError(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise ProducerIdentityError(f"{label} cannot be resolved: {exc}") from exc
    if str(resolved) != text:
        raise ProducerIdentityError(f"{label} must be a resolved absolute path")
    return resolved


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _stat_key(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        int(value.st_dev),
        int(value.st_ino),
        int(value.st_size),
        int(value.st_mtime_ns),
        int(value.st_ctime_ns),
        int(value.st_mode),
    )


def _fingerprint(path_value: str | os.PathLike[str], label: str) -> dict[str, Any]:
    """Hash one regular file and reject path or file changes during the read."""

    original = Path(path_value)
    try:
        resolved = original.resolve(strict=True)
        before_path = resolved.stat()
        if not stat.S_ISREG(before_path.st_mode):
            raise ProducerIdentityError(f"{label} is not a regular file: {resolved}")
        digest = hashlib.sha256()
        length = 0
        with resolved.open("rb") as stream:
            before_fd = os.fstat(stream.fileno())
            if _stat_key(before_path) != _stat_key(before_fd):
                raise ProducerIdentityError(f"{label} changed before hashing: {resolved}")
            while True:
                chunk = stream.read(_HASH_CHUNK_BYTES)
                if not chunk:
                    break
                digest.update(chunk)
                length += len(chunk)
            after_fd = os.fstat(stream.fileno())
        after_path = resolved.stat()
        after_original = original.resolve(strict=True)
    except ProducerIdentityError:
        raise
    except (OSError, RuntimeError, ValueError) as exc:
        raise ProducerIdentityError(f"cannot fingerprint {label}: {exc}") from exc

    before_key = _stat_key(before_path)
    if (
        _stat_key(before_fd) != before_key
        or _stat_key(after_fd) != before_key
        or _stat_key(after_path) != before_key
        or after_original != resolved
        or length != before_key[2]
    ):
        raise ProducerIdentityError(f"{label} changed while hashing: {resolved}")
    return {"path": str(resolved), "sha256": digest.hexdigest(), "size": length}


def _scan_script_files(root: Path) -> list[tuple[str, str]]:
    """Return all owned Python source paths, rejecting unprovable symlink dirs."""

    found: list[tuple[str, str]] = []

    def on_walk_error(error: OSError) -> None:
        raise ProducerIdentityError(f"cannot enumerate producer tree: {error}") from error

    for directory, directory_names, file_names in os.walk(
        root, onerror=on_walk_error, followlinks=False
    ):
        directory_path = Path(directory)
        for name in directory_names:
            child = directory_path / name
            if child.is_symlink():
                try:
                    target = child.resolve(strict=True)
                except (OSError, RuntimeError, ValueError) as exc:
                    raise ProducerIdentityError(
                        f"producer directory symlink cannot be resolved: {child}"
                    ) from exc
                if not _is_within(target, root):
                    raise ProducerIdentityError(
                        f"producer directory symlink escapes scripts root: {child}"
                    )
                raise ProducerIdentityError(
                    f"producer directory symlink prevents complete inventory: {child}"
                )
        directory_names[:] = sorted(directory_names)
        for name in sorted(file_names):
            if not name.endswith(".py") or name.startswith("test_"):
                continue
            path = directory_path / name
            try:
                relative = path.relative_to(root).as_posix()
                safe_relative = _safe_relative_path(relative, "producer file path")
                resolved = path.resolve(strict=True)
            except (OSError, RuntimeError, ValueError) as exc:
                raise ProducerIdentityError(
                    f"producer file cannot be resolved: {path}"
                ) from exc
            if not _is_within(resolved, root):
                raise ProducerIdentityError(
                    f"producer file symlink escapes scripts root: {path}"
                )
            if not resolved.is_file():
                raise ProducerIdentityError(f"producer file is not regular: {path}")
            found.append((safe_relative, str(resolved)))
    found.sort(key=lambda item: item[0])
    if len({relative for relative, _path in found}) != len(found):
        raise ProducerIdentityError("producer inventory has duplicate relative paths")
    return found


def _module_mapping(modules: Mapping[str, Any] | None) -> Mapping[str, Any]:
    selected = sys.modules if modules is None else modules
    if not isinstance(selected, Mapping):
        raise ProducerIdentityError("modules must be a mapping like sys.modules")
    return selected


def _snapshot_modules(modules: Mapping[str, Any]) -> dict[str, Any]:
    try:
        snapshot = dict(modules.items())
    except (RuntimeError, TypeError, ValueError) as exc:
        raise ProducerIdentityError(f"cannot snapshot loaded modules: {exc}") from exc
    if any(not isinstance(name, str) for name in snapshot):
        raise ProducerIdentityError("loaded module names must be strings")
    return snapshot


def _main_file(modules: Mapping[str, Any]) -> tuple[Any, Path]:
    module = modules.get("__main__")
    if module is None:
        raise ProducerIdentityError("already-loaded __main__ module is missing")
    raw_path = getattr(module, "__file__", None)
    if not isinstance(raw_path, str) or not raw_path:
        raise ProducerIdentityError("__main__.__file__ is missing or invalid")
    fingerprint = _fingerprint(raw_path, "__main__ entrypoint")
    path = Path(fingerprint["path"])
    spec_origin = getattr(getattr(module, "__spec__", None), "origin", None)
    if isinstance(spec_origin, str) and spec_origin not in {"built-in", "frozen"}:
        origin = _fingerprint(spec_origin, "__main__ spec origin")
        if origin["path"] != fingerprint["path"]:
            raise ProducerIdentityError("__main__ spec origin differs from __file__")
    return module, path


def _entrypoint_record(path: str | os.PathLike[str], label: str) -> dict[str, Any]:
    fingerprint = _fingerprint(path, label)
    return {key: fingerprint[key] for key in ("path", "sha256", "size")}


def _extra_record(module_name: str, module: Any) -> dict[str, Any]:
    if module_name != "sitecustomize":
        raise ProducerIdentityError(f"unsupported producer extra module: {module_name}")
    raw_path = getattr(module, "__file__", None)
    if not isinstance(raw_path, str) or not raw_path:
        raise ProducerIdentityError("loaded sitecustomize has no file")
    fingerprint = _fingerprint(raw_path, "sitecustomize")
    origin_raw = getattr(getattr(module, "__spec__", None), "origin", None)
    if not isinstance(origin_raw, str) or not origin_raw or origin_raw in {"built-in", "frozen"}:
        raise ProducerIdentityError("loaded sitecustomize has no file origin")
    origin = _fingerprint(origin_raw, "sitecustomize spec origin")
    if origin["path"] != fingerprint["path"]:
        raise ProducerIdentityError("sitecustomize spec origin differs from __file__")
    return {
        "module": module_name,
        "path": fingerprint["path"],
        "sha256": fingerprint["sha256"],
        "size": fingerprint["size"],
    }


def capture_producer_bundle(
    script_root: Path,
    parent_entrypoint: Path,
    *,
    modules: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Freeze scripts-tree files, both entrypoints, and loaded sitecustomize."""

    selected_modules = _snapshot_modules(_module_mapping(modules))
    try:
        root = Path(script_root).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise ProducerIdentityError(f"scripts root cannot be resolved: {exc}") from exc
    if not root.is_dir():
        raise ProducerIdentityError("scripts root is not a directory")

    _main, observed_parent_path = _main_file(selected_modules)
    try:
        requested_parent_path = Path(parent_entrypoint).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise ProducerIdentityError(f"parent entrypoint cannot be resolved: {exc}") from exc
    if observed_parent_path != requested_parent_path:
        raise ProducerIdentityError("parent entrypoint differs from loaded __main__.__file__")

    before = _scan_script_files(root)
    child_path = root / CHILD_ENTRYPOINT
    child_resolved = child_path.resolve(strict=True)
    if not _is_within(child_resolved, root):
        raise ProducerIdentityError("fixed child entrypoint escapes scripts root")
    if (CHILD_ENTRYPOINT, str(child_resolved)) not in before:
        raise ProducerIdentityError("fixed child entrypoint is absent from producer inventory")

    files = []
    for relative, resolved in before:
        fingerprint = _fingerprint(root / relative, f"producer file {relative}")
        if fingerprint["path"] != resolved:
            raise ProducerIdentityError(f"producer file path changed during capture: {relative}")
        files.append({
            "relative_path": relative,
            "resolved_path": resolved,
            "sha256": fingerprint["sha256"],
            "size": fingerprint["size"],
        })

    parent_record = _entrypoint_record(requested_parent_path, "parent entrypoint")
    child_record = _entrypoint_record(child_resolved, "child entrypoint")
    extras = []
    sitecustomize = selected_modules.get("sitecustomize")
    if sitecustomize is not None:
        extras.append(_extra_record("sitecustomize", sitecustomize))

    after = _scan_script_files(root)
    if after != before:
        raise ProducerIdentityError("producer file set changed while capturing bundle")
    bundle = {
        "schema": BUNDLE_SCHEMA,
        "root": str(root),
        "files": files,
        "entrypoints": {"parent": parent_record, "child": child_record},
        "extras": sorted(extras, key=lambda record: record["module"]),
    }
    validate_producer_bundle(bundle)
    return bundle


def validate_producer_bundle(bundle: Any) -> None:
    """Validate bundle structure and recheck its complete current file snapshot."""

    value = _exact_object(bundle, {"schema", "root", "files", "entrypoints", "extras"}, "producer bundle")
    if value["schema"] != BUNDLE_SCHEMA:
        raise ProducerIdentityError("producer bundle schema is unsupported")
    root = _absolute_path(value["root"], "producer bundle root")
    if not root.is_dir():
        raise ProducerIdentityError("producer bundle root is not a directory")

    files = value["files"]
    if not isinstance(files, list) or not files:
        raise ProducerIdentityError("producer bundle files must be a non-empty list")
    relative_paths: list[str] = []
    file_by_path: dict[str, dict[str, Any]] = {}
    previous = ""
    for index, item in enumerate(files):
        label = f"producer bundle files[{index}]"
        record = _exact_object(item, {"relative_path", "resolved_path", "sha256", "size"}, label)
        relative = _safe_relative_path(record["relative_path"], f"{label}.relative_path")
        if not relative.endswith(".py") or PurePosixPath(relative).name.startswith("test_"):
            raise ProducerIdentityError(f"{label} is not an inventoried producer source file")
        if relative <= previous:
            raise ProducerIdentityError("producer bundle files are duplicated or unsorted")
        previous = relative
        resolved = _absolute_path(record["resolved_path"], f"{label}.resolved_path")
        if not _is_within(resolved, root):
            raise ProducerIdentityError(f"{label} escapes scripts root")
        digest = _digest(record["sha256"], f"{label}.sha256")
        size = _size(record["size"], f"{label}.size")
        current = _fingerprint(root / PurePosixPath(relative), label)
        if (current["path"], current["sha256"], current["size"]) != (
            str(resolved), digest, size
        ):
            raise ProducerIdentityError(f"{label} differs from the current source file")
        relative_paths.append(relative)
        file_by_path[str(resolved)] = record

    current_inventory = _scan_script_files(root)
    expected_inventory = [
        (record["relative_path"], record["resolved_path"]) for record in files
    ]
    if current_inventory != expected_inventory:
        raise ProducerIdentityError("producer scripts file set differs from frozen bundle")
    child_relative = CHILD_ENTRYPOINT
    if child_relative not in relative_paths:
        raise ProducerIdentityError("fixed child entrypoint is absent from producer bundle")

    entrypoints = _exact_object(value["entrypoints"], {"parent", "child"}, "producer bundle entrypoints")
    parent = _validate_entrypoint(entrypoints["parent"], "parent entrypoint")
    child = _validate_entrypoint(entrypoints["child"], "child entrypoint")
    expected_child = _fingerprint(root / CHILD_ENTRYPOINT, "fixed child entrypoint")
    if child != {key: expected_child[key] for key in ("path", "sha256", "size")}:
        raise ProducerIdentityError("child entrypoint differs from fixed runner source")
    if child["path"] not in file_by_path:
        raise ProducerIdentityError("child entrypoint is not in the producer file inventory")
    if file_by_path[child["path"]]["sha256"] != child["sha256"]:
        raise ProducerIdentityError("child entrypoint and inventory hashes differ")
    if _is_within(Path(parent["path"]), root):
        parent_file = file_by_path.get(parent["path"])
        if parent_file is None:
            raise ProducerIdentityError(
                "parent entrypoint under scripts root is absent from producer inventory"
            )
        if (
            parent_file["sha256"] != parent["sha256"]
            or parent_file["size"] != parent["size"]
        ):
            raise ProducerIdentityError("parent entrypoint differs from producer inventory")

    extras = value["extras"]
    if not isinstance(extras, list):
        raise ProducerIdentityError("producer bundle extras must be a list")
    previous_extra = ""
    seen_extras: set[str] = set()
    for index, item in enumerate(extras):
        label = f"producer bundle extras[{index}]"
        record = _exact_object(item, {"module", "path", "sha256", "size"}, label)
        module = _nonempty_string(record["module"], f"{label}.module")
        if module != "sitecustomize":
            raise ProducerIdentityError(f"{label} names an unsupported extra module")
        if module in seen_extras or module <= previous_extra:
            raise ProducerIdentityError("producer bundle extras are duplicated or unsorted")
        seen_extras.add(module)
        previous_extra = module
        path = _absolute_path(record["path"], f"{label}.path")
        digest = _digest(record["sha256"], f"{label}.sha256")
        size = _size(record["size"], f"{label}.size")
        current = _fingerprint(path, label)
        if (current["path"], current["sha256"], current["size"]) != (
            str(path), digest, size
        ):
            raise ProducerIdentityError(f"{label} differs from current extra module")


def _validate_entrypoint(value: Any, label: str) -> dict[str, Any]:
    record = _exact_object(value, {"path", "sha256", "size"}, label)
    path = _absolute_path(record["path"], f"{label}.path")
    digest = _digest(record["sha256"], f"{label}.sha256")
    size = _size(record["size"], f"{label}.size")
    current = _fingerprint(path, label)
    if (current["path"], current["sha256"], current["size"]) != (
        str(path), digest, size
    ):
        raise ProducerIdentityError(f"{label} differs from its captured file")
    return record


def _module_origin_and_path(module: Any, name: str, root: Path) -> tuple[str | None, Path | None]:
    raw_file = getattr(module, "__file__", None)
    spec = getattr(module, "__spec__", None)
    raw_origin = getattr(spec, "origin", None)
    file_path: Path | None = None
    origin_path: Path | None = None
    def resolve_if_owned(raw: Any) -> Path | None:
        if not isinstance(raw, str) or not raw or raw in {"built-in", "frozen"}:
            return None
        candidate = Path(raw)
        try:
            lexical = candidate.absolute()
            resolved_hint = candidate.resolve(strict=False)
        except (OSError, RuntimeError, ValueError):
            return None
        if not _is_within(lexical, root) and not _is_within(resolved_hint, root):
            return None
        try:
            return candidate.resolve(strict=True)
        except (OSError, RuntimeError, ValueError) as exc:
            raise ProducerIdentityError(
                f"loaded producer module {name} has a missing owned file: {exc}"
            ) from exc

    file_path = resolve_if_owned(raw_file)
    origin_path = resolve_if_owned(raw_origin)
    file_inside = file_path is not None and _is_within(file_path, root)
    origin_inside = origin_path is not None and _is_within(origin_path, root)
    if not file_inside and not origin_inside:
        return None, None
    if not origin_inside or origin_path is None:
        raise ProducerIdentityError(f"loaded producer module {name} lacks a file origin")

    # With PYTHONPYCACHEPREFIX, __file__ can point at an external .pyc while
    # spec.origin still names the owned source. Resolve that mapping explicitly
    # and attest the source file in the frozen inventory.
    if not isinstance(raw_file, str) or not raw_file:
        raise ProducerIdentityError(f"loaded producer module {name} lacks a file path")
    try:
        observed_file_path = Path(raw_file).resolve(strict=True)
        observed_file_mode = observed_file_path.stat().st_mode
    except (OSError, RuntimeError, ValueError) as exc:
        raise ProducerIdentityError(
            f"loaded producer module {name} file path cannot be resolved: {exc}"
        ) from exc
    if not stat.S_ISREG(observed_file_mode):
        raise ProducerIdentityError(f"loaded producer module {name} file is not regular")
    if observed_file_path != origin_path:
        try:
            source_from_cache = Path(
                importlib.util.source_from_cache(str(observed_file_path))
            ).resolve(strict=True)
        except (NotImplementedError, OSError, ValueError):
            source_from_cache = None
        if source_from_cache != origin_path:
            raise ProducerIdentityError(f"loaded producer module {name} file and origin differ")
    return str(origin_path), origin_path


def _module_alias_names(files: list[dict[str, Any]]) -> set[str]:
    aliases: set[str] = set()
    for record in files:
        parts = list(PurePosixPath(record["relative_path"]).with_suffix("").parts)
        if parts and parts[-1] == "__init__":
            parts.pop()
        if not parts:
            continue
        relative_name = ".".join(parts)
        for name in (relative_name, f"scripts.{relative_name}"):
            aliases.add(name)
            pieces = name.split(".")
            aliases.update(".".join(pieces[:index]) for index in range(1, len(pieces)))
    return aliases


def _namespace_paths(module: Any) -> list[Path]:
    raw_paths = getattr(module, "__path__", None)
    if raw_paths is None:
        return []
    if isinstance(raw_paths, (str, os.PathLike)):
        raw_paths = [raw_paths]
    result = []
    for raw_path in raw_paths:
        try:
            result.append(Path(raw_path).resolve(strict=True))
        except (OSError, RuntimeError, TypeError, ValueError) as exc:
            raise ProducerIdentityError(f"namespace package path is invalid: {exc}") from exc
    return result


def _capture_loaded_modules(
    selected: Mapping[str, Any], bundle: dict[str, Any], entrypoint: dict[str, Any]
) -> list[dict[str, Any]]:
    root = Path(bundle["root"])
    files = bundle["files"]
    owned_files = {record["resolved_path"]: record for record in files}
    aliases = _module_alias_names(files)
    expected_extras = {record["module"]: record for record in bundle["extras"]}

    main = selected.get("__main__")
    if main is None:
        raise ProducerIdentityError("already-loaded __main__ module is missing")
    _main_module, main_path = _main_file(selected)
    if str(main_path) != entrypoint["path"]:
        raise ProducerIdentityError("loaded __main__ file differs from selected entrypoint")
    main_origin_raw = getattr(getattr(main, "__spec__", None), "origin", None)
    main_origin: str | None = None
    if isinstance(main_origin_raw, str) and main_origin_raw not in {"built-in", "frozen"}:
        main_origin_path = Path(main_origin_raw).resolve(strict=True)
        if str(main_origin_path) != entrypoint["path"]:
            raise ProducerIdentityError("loaded __main__ spec origin differs from entrypoint")
        main_origin = str(main_origin_path)
    records = [{
        "module": "__main__",
        "origin": main_origin,
        "path": entrypoint["path"],
        "sha256": entrypoint["sha256"],
        "size": entrypoint["size"],
    }]

    for name, module in sorted(selected.items()):
        if name == "__main__" or module is None:
            continue
        if name == "sitecustomize":
            expected = expected_extras.get(name)
            if expected is None:
                raise ProducerIdentityError("sitecustomize loaded but absent from frozen extras")
            current = _extra_record(name, module)
            if current != expected:
                raise ProducerIdentityError("loaded sitecustomize differs from frozen extra")
            records.append({
                "module": name,
                "origin": current["path"],
                "path": current["path"],
                "sha256": current["sha256"],
                "size": current["size"],
            })
            continue

        origin, path = _module_origin_and_path(module, name, root)
        if path is None:
            namespace_paths = _namespace_paths(module)
            if any(_is_within(item, root) for item in namespace_paths):
                continue
            if name in aliases:
                raise ProducerIdentityError(f"expected producer module {name} is not file-backed")
            continue

        if origin is None or path is None:
            raise ProducerIdentityError(f"loaded producer module {name} has no source origin")
        if not _is_within(path, root):
            if name in aliases:
                raise ProducerIdentityError(f"producer module {name} resolves outside scripts root")
            continue
        current = _fingerprint(path, f"loaded producer module {name}")
        owned = owned_files.get(current["path"])
        if owned is None:
            raise ProducerIdentityError(f"loaded producer module {name} is absent from bundle files")
        if current["sha256"] != owned["sha256"] or current["size"] != owned["size"]:
            raise ProducerIdentityError(f"loaded producer module {name} differs from bundle snapshot")
        records.append({
            "module": name,
            "origin": origin,
            "path": current["path"],
            "sha256": current["sha256"],
            "size": current["size"],
        })

    if set(expected_extras) != {record["module"] for record in records if record["module"] == "sitecustomize"}:
        raise ProducerIdentityError("loaded producer extras differ from frozen bundle")
    records.sort(key=lambda record: record["module"])
    if len({record["module"] for record in records}) != len(records):
        raise ProducerIdentityError("loaded producer module names are ambiguous")
    return records


def capture_producer_identity(
    bundle: dict[str, Any],
    *,
    role: str,
    modules: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Capture the selected entrypoint and already-loaded producer modules."""

    if not isinstance(role, str) or role not in {"parent", "child"}:
        raise ProducerIdentityError("producer role must be 'parent' or 'child'")
    validate_producer_bundle(bundle)
    selected = _snapshot_modules(_module_mapping(modules))
    entrypoint = bundle["entrypoints"][role]
    _main, main_path = _main_file(selected)
    current = _entrypoint_record(main_path, f"{role} entrypoint")
    if current != entrypoint:
        raise ProducerIdentityError(f"loaded {role} entrypoint differs from frozen bundle")
    identity = {
        "schema": IDENTITY_SCHEMA,
        "role": role,
        "entrypoint": current,
        "loaded_modules": _capture_loaded_modules(selected, bundle, entrypoint),
        "files_sha256": _canonical_json_sha256(bundle["files"]),
    }
    validate_producer_identity(identity, bundle)
    return identity


def validate_producer_identity(identity: Any, bundle: Any) -> None:
    """Validate identity records against the complete frozen producer bundle."""

    validate_producer_bundle(bundle)
    value = _exact_object(
        identity,
        {"schema", "role", "entrypoint", "loaded_modules", "files_sha256"},
        "producer identity",
    )
    if value["schema"] != IDENTITY_SCHEMA:
        raise ProducerIdentityError("producer identity schema is unsupported")
    role = value["role"]
    if not isinstance(role, str) or role not in {"parent", "child"}:
        raise ProducerIdentityError("producer identity role is invalid")
    entrypoint = _validate_entrypoint(value["entrypoint"], "producer identity entrypoint")
    if entrypoint != bundle["entrypoints"][role]:
        raise ProducerIdentityError("producer identity entrypoint differs from bundle")
    files_sha = _digest(value["files_sha256"], "producer identity files_sha256")
    if files_sha != _canonical_json_sha256(bundle["files"]):
        raise ProducerIdentityError("producer identity files_sha256 differs from bundle")

    files = bundle["files"]
    owned_files = {record["resolved_path"]: record for record in files}
    expected_extras = {record["module"]: record for record in bundle["extras"]}
    modules = value["loaded_modules"]
    if not isinstance(modules, list) or not modules:
        raise ProducerIdentityError("producer identity loaded_modules must be non-empty")
    names: list[str] = []
    main_records = 0
    extra_records: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(modules):
        label = f"producer identity loaded_modules[{index}]"
        record = _exact_object(item, {"module", "origin", "path", "sha256", "size"}, label)
        name = _nonempty_string(record["module"], f"{label}.module")
        names.append(name)
        origin_value = record["origin"]
        if origin_value is not None:
            origin = _absolute_path(origin_value, f"{label}.origin")
        else:
            origin = None
        path = _absolute_path(record["path"], f"{label}.path")
        digest = _digest(record["sha256"], f"{label}.sha256")
        size = _size(record["size"], f"{label}.size")
        current = _fingerprint(path, label)
        if (current["sha256"], current["size"]) != (digest, size):
            raise ProducerIdentityError(f"{label} differs from its on-disk file")

        if name == "__main__":
            main_records += 1
            if origin is not None and str(origin) != str(path):
                raise ProducerIdentityError("producer __main__ origin differs from its entrypoint")
            if record != {
                "module": "__main__",
                "origin": str(origin) if origin is not None else None,
                "path": entrypoint["path"],
                "sha256": entrypoint["sha256"],
                "size": entrypoint["size"],
            }:
                raise ProducerIdentityError("producer __main__ does not match selected entrypoint")
            continue

        if name == "sitecustomize":
            if name not in expected_extras:
                raise ProducerIdentityError("producer identity contains an unbound sitecustomize")
            if name in extra_records:
                raise ProducerIdentityError("producer identity duplicates an extra module")
            expected = expected_extras[name]
            if str(path) != expected["path"] or digest != expected["sha256"] or size != expected["size"]:
                raise ProducerIdentityError(f"producer extra module {name} differs from bundle")
            if origin is None or str(origin) != expected["path"]:
                raise ProducerIdentityError(f"producer extra module {name} origin differs from bundle")
            extra_records[name] = record
            continue

        if not _is_within(path, Path(bundle["root"])) or origin is None or str(origin) != str(path):
            raise ProducerIdentityError(f"producer module {name} is not an owned source file")
        owned = owned_files.get(str(path))
        if owned is None or (owned["sha256"], owned["size"]) != (digest, size):
            raise ProducerIdentityError(f"producer module {name} differs from bundle inventory")

    if names != sorted(names) or len(names) != len(set(names)):
        raise ProducerIdentityError("producer identity modules are duplicated or unsorted")
    if main_records != 1:
        raise ProducerIdentityError("producer identity must contain exactly one __main__ record")
    if set(extra_records) != set(expected_extras):
        raise ProducerIdentityError("producer identity omits or adds frozen extra modules")

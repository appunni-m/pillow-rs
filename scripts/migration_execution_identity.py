"""Capture bounded, in-process identity facts for migration child processes.

This module deliberately does not import Pillow or ``pillow_rs``. Callers must
invoke it only after the selected side has already been imported and used.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import platform
import sys
import sysconfig
from typing import Any


SCHEMA = "migration-parity/runtime-identity@1"
_HASH_CHUNK_BYTES = 1024 * 1024
_PACKAGE_SUFFIXES = (".py", ".so", ".dylib", ".pyd", ".dll", ".bundle")
_PARALLEL_FEATURES = frozenset(
    {"pillow-rs-py/parallel", "pillow-rs/parallel"}
)
_DEFAULT_FEATURES = frozenset({"pillow-rs-py/default", "pillow-rs/default"})
_TARGET_PROFILE_BACKENDS = {
    "python-cpu": "cpu",
    "python-simd": "simd",
    "python-gpu": "gpu",
    "python-parallel-cpu": "cpu",
}
_SOURCE_PROFILE_IMPLEMENTATIONS = {
    "pillow": "Pillow",
    "pillow-simd": "Pillow-SIMD",
}
_BACKENDS = frozenset({"cpu", "simd", "gpu"})
_TARGET_FEATURE_NAMES = _DEFAULT_FEATURES | _PARALLEL_FEATURES


class RuntimeIdentityError(RuntimeError):
    """Raised when a child process identity cannot be attested consistently."""


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
        raise RuntimeIdentityError(f"identity is not canonical JSON: {exc}") from exc
    return hashlib.sha256(payload).hexdigest()


def _exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise RuntimeIdentityError(f"{label} has an invalid key set")
    return value


def _nonempty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise RuntimeIdentityError(f"{label} must be a non-empty string")
    return value


def _sha256_string(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or value != value.lower()
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise RuntimeIdentityError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _nonnegative_int(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        raise RuntimeIdentityError(f"{label} must be a non-negative integer")
    return value


def _validate_stat_identity(value: Any, label: str) -> dict[str, int]:
    record = _exact_keys(
        value,
        {"device", "inode", "size", "mtime_ns", "ctime_ns", "mode"},
        label,
    )
    for key in record:
        _nonnegative_int(record[key], f"{label}.{key}")
    return record


def _validate_file_record(
    value: Any, label: str, *, require_stat: bool
) -> dict[str, Any]:
    keys = {"path", "sha256", "size", "stat"} if require_stat else {
        "path", "sha256", "size"
    }
    record = _exact_keys(value, keys, label)
    path = _nonempty_string(record["path"], f"{label}.path")
    if not Path(path).is_absolute():
        raise RuntimeIdentityError(f"{label}.path must be absolute")
    _sha256_string(record["sha256"], f"{label}.sha256")
    size = _nonnegative_int(record["size"], f"{label}.size")
    if require_stat:
        stat_record = _validate_stat_identity(record["stat"], f"{label}.stat")
        if stat_record["size"] != size:
            raise RuntimeIdentityError(f"{label}.size differs from stat size")
    return record


def _validate_installation_fingerprint(
    value: Any, label: str, expected_package: str
) -> dict[str, Any]:
    record = _exact_keys(
        value, {"package", "roots", "files", "sha256"}, label
    )
    if record["package"] != expected_package:
        raise RuntimeIdentityError(f"{label}.package is invalid")
    roots = record["roots"]
    if not isinstance(roots, list) or not roots:
        raise RuntimeIdentityError(f"{label}.roots must be a non-empty list")
    for index, root in enumerate(roots):
        root_value = _nonempty_string(root, f"{label}.roots[{index}]")
        if not Path(root_value).is_absolute():
            raise RuntimeIdentityError(f"{label}.roots[{index}] must be absolute")
    if roots != sorted(set(roots)):
        raise RuntimeIdentityError(f"{label}.roots must be unique and sorted")
    files = record["files"]
    if not isinstance(files, list) or not files:
        raise RuntimeIdentityError(f"{label}.files must be a non-empty list")
    previous: tuple[int, str] | None = None
    seen: set[tuple[int, str]] = set()
    for index, item in enumerate(files):
        file_record = _exact_keys(
            item,
            {"root_index", "relative_path", "resolved_path", "size", "sha256"},
            f"{label}.files[{index}]",
        )
        root_index = _nonnegative_int(
            file_record["root_index"], f"{label}.files[{index}].root_index"
        )
        if root_index >= len(roots):
            raise RuntimeIdentityError(f"{label}.files[{index}] references an unknown root")
        relative_path = _nonempty_string(
            file_record["relative_path"], f"{label}.files[{index}].relative_path"
        )
        if Path(relative_path).is_absolute() or ".." in Path(relative_path).parts:
            raise RuntimeIdentityError(f"{label}.files[{index}] path is not package-relative")
        resolved_path = _nonempty_string(
            file_record["resolved_path"], f"{label}.files[{index}].resolved_path"
        )
        if not Path(resolved_path).is_absolute():
            raise RuntimeIdentityError(
                f"{label}.files[{index}].resolved_path must be absolute"
            )
        key = (root_index, relative_path)
        if key in seen or (previous is not None and key < previous):
            raise RuntimeIdentityError(f"{label}.files are duplicated or unsorted")
        seen.add(key)
        previous = key
        _nonnegative_int(file_record["size"], f"{label}.files[{index}].size")
        _sha256_string(file_record["sha256"], f"{label}.files[{index}].sha256")
    expected_sha = _canonical_json_sha256(
        {"package": expected_package, "roots": roots, "files": files}
    )
    if _sha256_string(record["sha256"], f"{label}.sha256") != expected_sha:
        raise RuntimeIdentityError(f"{label}.sha256 does not match its file list")
    return record


def _package_file_matches(
    package: dict[str, Any], file_path: str, file_sha256: str, file_size: int
) -> bool:
    return any(
        item["resolved_path"] == file_path
        and item["sha256"] == file_sha256
        and item["size"] == file_size
        for item in package["files"]
    )


def _stat_identity(value: os.stat_result) -> dict[str, int]:
    return {
        "device": int(value.st_dev),
        "inode": int(value.st_ino),
        "size": int(value.st_size),
        "mtime_ns": int(value.st_mtime_ns),
        "ctime_ns": int(value.st_ctime_ns),
        "mode": int(value.st_mode),
    }


def _stream_sha256(stream: Any) -> tuple[str, int]:
    digest = hashlib.sha256()
    length = 0
    while True:
        chunk = stream.read(_HASH_CHUNK_BYTES)
        if not chunk:
            break
        digest.update(chunk)
        length += len(chunk)
    return digest.hexdigest(), length


def _stable_file_identity(path_value: str | os.PathLike[str], label: str) -> dict[str, Any]:
    """Hash one regular file and reject changes observed during the read."""

    try:
        path = Path(path_value).resolve(strict=True)
        before_path = path.stat()
        if not path.is_file():
            raise RuntimeIdentityError(f"{label} is not a regular file: {path}")
        with path.open("rb") as stream:
            before_fd = os.fstat(stream.fileno())
            if _stat_identity(before_path) != _stat_identity(before_fd):
                raise RuntimeIdentityError(f"{label} changed before hashing: {path}")
            digest, byte_length = _stream_sha256(stream)
            after_fd = os.fstat(stream.fileno())
        after_path = path.stat()
    except RuntimeIdentityError:
        raise
    except (OSError, RuntimeError, ValueError) as exc:
        raise RuntimeIdentityError(f"cannot read {label}: {exc}") from exc

    identity = _stat_identity(before_path)
    if (
        _stat_identity(before_fd) != identity
        or _stat_identity(after_fd) != identity
        or _stat_identity(after_path) != identity
        or byte_length != identity["size"]
    ):
        raise RuntimeIdentityError(f"{label} changed while hashing: {path}")
    return {
        "path": str(path),
        "sha256": digest,
        "size": byte_length,
        "stat": identity,
    }


def _package_roots(module: Any, package_name: str) -> list[Path]:
    raw_paths = getattr(module, "__path__", None)
    if raw_paths is None:
        raw_file = getattr(module, "__file__", None)
        if not isinstance(raw_file, str) or not raw_file:
            raise RuntimeIdentityError(f"{package_name} has no package path")
        raw_paths = [str(Path(raw_file).parent)]
    elif isinstance(raw_paths, (str, os.PathLike)):
        raw_paths = [raw_paths]
    try:
        roots = sorted({Path(item).resolve(strict=True) for item in raw_paths}, key=str)
    except (OSError, TypeError, ValueError) as exc:
        raise RuntimeIdentityError(f"cannot resolve {package_name} package path: {exc}") from exc
    if not roots or any(not root.is_dir() for root in roots):
        raise RuntimeIdentityError(f"{package_name} package path is missing or not a directory")
    return roots


def _package_files(roots: list[Path]) -> list[tuple[int, Path, str]]:
    found: list[tuple[int, Path, str]] = []
    for root_index, root in enumerate(roots):
        def raise_walk_error(error: OSError) -> None:
            raise RuntimeIdentityError(
                f"cannot enumerate package files under {root}: {error}"
            ) from error

        for directory, directory_names, file_names in os.walk(
            root, onerror=raise_walk_error, followlinks=False
        ):
            for name in directory_names:
                child_directory = Path(directory) / name
                if child_directory.is_symlink() and name != "__pycache__":
                    raise RuntimeIdentityError(
                        f"package contains an untracked symlink directory: {child_directory}"
                    )
            directory_names[:] = sorted(
                name for name in directory_names if name != "__pycache__"
            )
            for file_name in sorted(file_names):
                path = Path(directory) / file_name
                try:
                    is_regular = path.is_file()
                except OSError as exc:
                    raise RuntimeIdentityError(
                        f"cannot inspect package file {path}: {exc}"
                    ) from exc
                if not is_regular or path.name.endswith((".pyc", ".pyo")):
                    continue
                if not path.name.endswith(_PACKAGE_SUFFIXES):
                    continue
                try:
                    relative = path.relative_to(root).as_posix()
                except ValueError as exc:
                    raise RuntimeIdentityError(f"package file escaped its root: {path}") from exc
                found.append((root_index, path, relative))
    found.sort(key=lambda item: (item[0], item[2]))
    return found


def _installation_fingerprint(module: Any, package_name: str) -> dict[str, Any]:
    roots = _package_roots(module, package_name)
    before_files = _package_files(roots)
    file_records: list[dict[str, Any]] = []
    for root_index, path, relative in before_files:
        identity = _stable_file_identity(path, f"{package_name} package file")
        file_records.append(
            {
                "root_index": root_index,
                "relative_path": relative,
                "resolved_path": identity["path"],
                "size": identity["size"],
                "sha256": identity["sha256"],
            }
        )
    after_files = _package_files(roots)
    before_names = [(index, relative) for index, _path, relative in before_files]
    after_names = [(index, relative) for index, _path, relative in after_files]
    if before_names != after_names:
        raise RuntimeIdentityError(f"{package_name} package file set changed while hashing")
    fingerprint_payload = {
        "package": package_name,
        "roots": [str(root) for root in roots],
        "files": file_records,
    }
    return {
        **fingerprint_payload,
        "sha256": _canonical_json_sha256(fingerprint_payload),
    }


def _module_file_path(module: Any, module_name: str) -> Path | None:
    raw_path = getattr(module, "__file__", None)
    if raw_path is None:
        return None
    if not isinstance(raw_path, str) or not raw_path:
        raise RuntimeIdentityError(f"{module_name} has an invalid __file__")
    try:
        return Path(raw_path).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise RuntimeIdentityError(f"{module_name} file is missing: {exc}") from exc


def _loaded_modules(side: str) -> list[dict[str, Any]]:
    prefixes = ("PIL",) if side == "source" else ("PIL", "pillow_rs")
    records: list[dict[str, Any]] = []
    for name, module in sorted(sys.modules.items()):
        if not any(name == prefix or name.startswith(prefix + ".") for prefix in prefixes):
            continue
        if module is None:
            continue
        path = _module_file_path(module, name)
        origin_value = getattr(getattr(module, "__spec__", None), "origin", None)
        origin = str(origin_value) if isinstance(origin_value, str) else None
        if path is None:
            records.append(
                {
                    "module": name,
                    "path": None,
                    "sha256": None,
                    "size": None,
                    "origin": origin,
                }
            )
            continue
        file_identity = _stable_file_identity(path, f"loaded module {name}")
        records.append(
            {
                "module": name,
                "path": file_identity["path"],
                "sha256": file_identity["sha256"],
                "size": file_identity["size"],
                "origin": origin,
            }
        )
    return records


def _loaded_mapping_identity(path: Path, stat_identity: dict[str, int]) -> dict[str, Any]:
    """Match the selected extension to its Linux mapping by device and inode."""

    if not sys.platform.startswith("linux"):
        return {
            "status": "unavailable",
            "reason": "unsupported_platform",
            "device": None,
            "inode": None,
        }
    maps_path = Path("/proc/self/maps")
    try:
        lines = maps_path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as exc:
        raise RuntimeIdentityError(f"cannot inspect selected core mapping: {exc}") from exc

    expected_device = stat_identity["device"]
    expected_inode = stat_identity["inode"]
    for line in lines:
        fields = line.split(maxsplit=5)
        if len(fields) < 5:
            continue
        try:
            major_text, minor_text = fields[3].split(":", 1)
            device = os.makedev(int(major_text, 16), int(minor_text, 16))
            inode = int(fields[4])
        except (ValueError, OverflowError):
            continue
        if device == expected_device and inode == expected_inode:
            return {
                "status": "verified",
                "reason": None,
                "device": device,
                "inode": inode,
            }
    raise RuntimeIdentityError(
        f"selected core is not mapped from its current on-disk file: {path}"
    )


def _python_snapshot() -> dict[str, Any]:
    executable = _stable_file_identity(sys.executable, "Python executable")
    implementation = sys.implementation
    try:
        soabi = sysconfig.get_config_var("SOABI")
    except (AttributeError, OSError, ValueError) as exc:
        raise RuntimeIdentityError(f"cannot read Python SOABI: {exc}") from exc
    version_info = list(sys.version_info[:5])
    return {
        "implementation": str(implementation.name),
        "version": platform.python_version(),
        "version_info": version_info,
        "build": list(platform.python_build()),
        "full_version": sys.version,
        "cache_tag": implementation.cache_tag,
        "soabi": soabi if isinstance(soabi, str) else None,
        "platform": sys.platform,
        "executable": executable,
    }


def _host_snapshot() -> dict[str, Any]:
    uname = platform.uname()
    runner_id = os.environ.get("MIGRATION_BENCHMARK_RUNNER_ID", "").strip()
    return {
        "node": platform.node() or "unknown",
        "system": uname.system or "unknown",
        "release": uname.release or "unknown",
        "version": uname.version or "unknown",
        "machine": uname.machine or "unknown",
        "processor": platform.processor() or "unknown",
        "platform": platform.platform() or "unknown",
        "runner_id": runner_id or None,
    }


def _require_module(name: str) -> Any:
    module = sys.modules.get(name)
    if module is None:
        raise RuntimeIdentityError(f"required loaded module is missing: {name}")
    actual_name = getattr(module, "__name__", None)
    spec_name = getattr(getattr(module, "__spec__", None), "name", None)
    if actual_name != name or spec_name != name:
        raise RuntimeIdentityError(
            f"required module {name} has unexpected name/spec: {actual_name!r}/{spec_name!r}"
        )
    return module


def _validate_handshake_module(
    module: Any,
    module_name: str,
    handshake_path: Any,
    handshake_version: Any,
) -> Path:
    if not isinstance(handshake_path, str) or not handshake_path:
        raise RuntimeIdentityError(f"handshake has no path for {module_name}")
    module_path = _module_file_path(module, module_name)
    if module_path is None:
        raise RuntimeIdentityError(f"required module {module_name} has no file")
    try:
        declared_path = Path(handshake_path).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise RuntimeIdentityError(f"handshake path for {module_name} is invalid: {exc}") from exc
    if module_path != declared_path:
        raise RuntimeIdentityError(
            f"loaded {module_name} path {module_path} differs from handshake {declared_path}"
        )
    spec_origin = getattr(getattr(module, "__spec__", None), "origin", None)
    if not isinstance(spec_origin, str) or not spec_origin:
        raise RuntimeIdentityError(f"loaded {module_name} spec has no file origin")
    try:
        resolved_origin = Path(spec_origin).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise RuntimeIdentityError(f"loaded {module_name} spec origin is invalid: {exc}") from exc
    if resolved_origin != module_path:
        raise RuntimeIdentityError(f"loaded {module_name} spec origin differs from __file__")
    version = str(getattr(module, "__version__", ""))
    if not isinstance(handshake_version, str) or version != handshake_version:
        raise RuntimeIdentityError(
            f"loaded {module_name} version {version!r} differs from handshake {handshake_version!r}"
        )
    return module_path


def _target_backend_identity(
    handshake: dict[str, Any], profile: str, declared_features: list[str]
) -> dict[str, Any]:
    expected_backend = _TARGET_PROFILE_BACKENDS.get(profile)
    if expected_backend is None:
        raise RuntimeIdentityError(f"unsupported target profile: {profile!r}")
    backend = handshake.get("backend")
    backend_state = handshake.get("backend_state")
    if not isinstance(backend, str) or backend.lower() != expected_backend:
        raise RuntimeIdentityError(
            f"profile {profile!r} requires backend {expected_backend!r}, got {backend!r}"
        )
    if not isinstance(backend_state, dict):
        raise RuntimeIdentityError("target handshake has no backend state")
    if str(backend_state.get("requested", "")).lower() != expected_backend:
        raise RuntimeIdentityError("target handshake requested backend does not match profile")

    target = _require_module("pillow_rs")
    core = getattr(target, "_core", None)
    if core is None:
        raise RuntimeIdentityError("pillow_rs has no already-loaded selected _core module")
    core_name = getattr(core, "__name__", None)
    core_spec = getattr(core, "__spec__", None)
    spec_name = getattr(core_spec, "name", None)
    allowed_names = {"pillow_rs._core", "pillow_rs._core_parallel"}
    if core_name not in allowed_names or spec_name != core_name:
        raise RuntimeIdentityError(
            f"selected core name/spec is not canonical: {core_name!r}/{spec_name!r}"
        )
    if sys.modules.get(core_name) is not core:
        raise RuntimeIdentityError("selected core module is not the loaded sys.modules object")

    try:
        active_raw = target.active_backends()
        available_raw = target.available_backends()
        parallel_raw = core.parallel_feature_enabled()
        wrapper_parallel_raw = target.parallel_feature_enabled()
    except Exception as exc:  # identity capture must fail closed on binding errors
        raise RuntimeIdentityError(f"cannot query selected target backend: {exc}") from exc
    if not isinstance(parallel_raw, bool) or not isinstance(wrapper_parallel_raw, bool):
        raise RuntimeIdentityError("parallel feature query did not return bool")
    if wrapper_parallel_raw != parallel_raw:
        raise RuntimeIdentityError("pillow_rs and selected core disagree on parallel feature state")
    parallel_enabled = parallel_raw

    if not isinstance(active_raw, (list, tuple)) or not isinstance(available_raw, (list, tuple)):
        raise RuntimeIdentityError("target backend queries did not return lists")
    active = [str(item).lower() for item in active_raw]
    available = sorted(str(item).lower() for item in available_raw)
    if active != [expected_backend]:
        raise RuntimeIdentityError(
            f"active target backends {active!r} do not equal requested {[expected_backend]!r}"
        )
    if expected_backend not in available:
        raise RuntimeIdentityError(f"requested backend {expected_backend!r} is unavailable")

    handshake_active = backend_state.get("active")
    handshake_available = backend_state.get("available")
    if not isinstance(handshake_active, (list, tuple)) or [
        str(item).lower() for item in handshake_active
    ] != active:
        raise RuntimeIdentityError("actual active backends differ from target handshake")
    if not isinstance(handshake_available, (list, tuple)) or sorted(
        str(item).lower() for item in handshake_available
    ) != available:
        raise RuntimeIdentityError("actual available backends differ from target handshake")

    parallel_profile = profile == "python-parallel-cpu"
    if parallel_enabled != parallel_profile:
        raise RuntimeIdentityError(
            f"target profile {profile!r} and runtime parallel feature disagree"
        )
    if core_name == "pillow_rs._core_parallel" and not parallel_enabled:
        raise RuntimeIdentityError("_core_parallel is selected while parallel feature is disabled")
    if core_name == "pillow_rs._core" and parallel_enabled and not parallel_profile:
        raise RuntimeIdentityError(
            "parallel-enabled canonical _core is valid only for python-parallel-cpu"
        )
    if core_name == "pillow_rs._core_parallel" and not parallel_profile:
        raise RuntimeIdentityError("_core_parallel is valid only for python-parallel-cpu")

    feature_set = set(declared_features)
    if not _DEFAULT_FEATURES.issubset(feature_set):
        raise RuntimeIdentityError("declared target features omit the default binding/core features")
    declared_parallel = feature_set & _PARALLEL_FEATURES
    if parallel_enabled:
        if declared_parallel != _PARALLEL_FEATURES:
            raise RuntimeIdentityError("declared target features do not include both parallel features")
    elif declared_parallel:
        raise RuntimeIdentityError("serial runtime declares parallel target features")

    core_path = _module_file_path(core, core_name)
    if core_path is None:
        raise RuntimeIdentityError("selected native core has no file path")
    spec_origin = getattr(core_spec, "origin", None)
    if not isinstance(spec_origin, str) or not spec_origin:
        raise RuntimeIdentityError("selected native core spec has no file origin")
    try:
        resolved_spec_origin = Path(spec_origin).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as exc:
        raise RuntimeIdentityError(f"selected native core spec origin is invalid: {exc}") from exc
    if resolved_spec_origin != core_path:
        raise RuntimeIdentityError("selected native core spec origin differs from __file__")
    core_file = _stable_file_identity(core_path, "selected native core")
    mapping = _loaded_mapping_identity(core_path, core_file["stat"])
    if mapping["status"] == "verified" and (
        mapping["device"] != core_file["stat"]["device"]
        or mapping["inode"] != core_file["stat"]["inode"]
    ):
        raise RuntimeIdentityError("selected core mapping does not match its on-disk file")
    try:
        final_path_identity = _stat_identity(core_path.stat())
    except OSError as exc:
        raise RuntimeIdentityError(f"selected native core disappeared after mapping check: {exc}") from exc
    if final_path_identity != core_file["stat"]:
        raise RuntimeIdentityError("selected native core changed during mapping inspection")
    return {
        "backend": expected_backend,
        "available_backends": available,
        "active_backends": active,
        "parallel_feature_enabled": parallel_enabled,
        "selected_core": {
            "module_name": core_name,
            "spec_name": spec_name,
            "file": core_file,
            "mapping": mapping,
        },
    }


def capture_runtime_identity(
    side: str,
    handshake: dict[str, Any],
    profile: str,
    declared_features: list[str] | tuple[str, ...],
) -> dict[str, Any]:
    """Capture stable runtime/install identity and local loaded-module facts.

    This function only inspects modules that the runner already loaded. It
    never imports ``PIL``, ``pillow_rs``, image modules, or optional plugins.
    The selected native extension hash identifies its on-disk file as observed
    in this process; it is not a claim about bytes already mapped in memory.
    """

    if side not in {"source", "target"}:
        raise RuntimeIdentityError(f"unsupported identity side: {side!r}")
    if not isinstance(profile, str) or not profile:
        raise RuntimeIdentityError("profile must be a non-empty string")
    if not isinstance(handshake, dict) or handshake.get("side") != side:
        raise RuntimeIdentityError("side handshake is missing or belongs to another side")
    if not isinstance(declared_features, (list, tuple)) or any(
        not isinstance(item, str) or not item for item in declared_features
    ):
        raise RuntimeIdentityError("declared_features must be a list of non-empty strings")
    if len(set(declared_features)) != len(declared_features):
        raise RuntimeIdentityError("declared_features contains duplicates")
    feature_list = list(declared_features)

    implementation = handshake.get("implementation")
    version = handshake.get("version")
    public_module_name = handshake.get("public_module")
    if not isinstance(implementation, str) or not implementation:
        raise RuntimeIdentityError("handshake has no implementation name")
    if not isinstance(version, str) or not version:
        raise RuntimeIdentityError("handshake has no implementation version")
    if public_module_name != "PIL":
        raise RuntimeIdentityError("handshake public module must be PIL")

    pil = _require_module("PIL")
    pil_path = _validate_handshake_module(
        pil, "PIL", handshake.get("public_path", handshake.get("path")), version
    )
    pil_installation = _installation_fingerprint(pil, "PIL")
    installation: dict[str, Any] = {
        "PIL": pil_installation,
        "pillow_rs": None,
        "target_PIL_facade": None,
    }
    target_execution: dict[str, Any] | None = None

    if side == "source":
        expected_implementation = _SOURCE_PROFILE_IMPLEMENTATIONS.get(profile)
        if expected_implementation is None or implementation != expected_implementation:
            raise RuntimeIdentityError(
                f"source profile {profile!r} does not match implementation "
                f"{implementation!r}"
            )
        target_checkout_path = getattr(pil, "__file__", None)
        if not isinstance(target_checkout_path, str):
            raise RuntimeIdentityError("source PIL package has no file path")
        checkout_python = Path(__file__).resolve().parents[1] / "pillow-rs-py" / "python"
        try:
            if checkout_python.resolve() in Path(target_checkout_path).resolve().parents:
                raise RuntimeIdentityError("source PIL was imported from the target checkout")
        except (OSError, RuntimeError, ValueError) as exc:
            raise RuntimeIdentityError(f"cannot validate source package path: {exc}") from exc
        pillow_rs_loaded = sys.modules.get("pillow_rs")
        if pillow_rs_loaded is not None:
            target_path = getattr(pillow_rs_loaded, "__file__", None)
            if isinstance(target_path, str):
                try:
                    if Path(target_path).resolve() == pil_path:
                        raise RuntimeIdentityError("source PIL resolves to target facade path")
                except (OSError, RuntimeError, ValueError) as exc:
                    raise RuntimeIdentityError(f"cannot validate source package path: {exc}") from exc
    else:
        if implementation != "pillow-rs":
            raise RuntimeIdentityError(f"unexpected target implementation: {implementation!r}")
        target = _require_module("pillow_rs")
        if str(getattr(target, "__version__", "")) != version:
            raise RuntimeIdentityError("loaded pillow_rs version differs from handshake")
        target_path = _validate_handshake_module(
            target, "pillow_rs", handshake.get("path"), version
        )
        public_path_value = handshake.get("public_path")
        if not isinstance(public_path_value, str) or not public_path_value:
            raise RuntimeIdentityError("target handshake has no public PIL path")
        try:
            declared_public_path = Path(public_path_value).resolve(strict=True)
        except (OSError, RuntimeError, ValueError) as exc:
            raise RuntimeIdentityError(f"target public PIL path is invalid: {exc}") from exc
        if pil_path != declared_public_path:
            raise RuntimeIdentityError("loaded target PIL path differs from handshake")
        if str(getattr(pil, "__version__", "")) != version:
            raise RuntimeIdentityError("loaded target PIL version differs from handshake")
        if target_path == pil_path:
            raise RuntimeIdentityError("target PIL facade and pillow_rs paths unexpectedly alias")
        installation["pillow_rs"] = _installation_fingerprint(target, "pillow_rs")
        installation["target_PIL_facade"] = pil_installation
        target_execution = _target_backend_identity(handshake, profile, feature_list)

    stable = {
        "schema": SCHEMA,
        "side": side,
        "profile": profile,
        "implementation": implementation,
        "version": version,
        "declared_features": feature_list,
        "python": _python_snapshot(),
        "host": _host_snapshot(),
        "installation": installation,
        "target_execution": target_execution,
    }
    snapshot = {
        "stable": stable,
        "loaded_modules": _loaded_modules(side),
        "stable_sha256": _canonical_json_sha256(stable),
    }
    validate_runtime_identity(snapshot)
    return snapshot


def validate_runtime_identity(snapshot: dict[str, Any]) -> None:
    """Validate the exact fixed snapshot schema and its internal digests.

    This validates one invocation in isolation. It intentionally does not
    compare loaded-module membership with another invocation; that list is
    case-local observation, while ``stable`` is the cross-invocation identity.
    """

    top = _exact_keys(
        snapshot,
        {"stable", "loaded_modules", "stable_sha256"},
        "runtime identity snapshot",
    )
    stable = _exact_keys(
        top["stable"],
        {
            "schema",
            "side",
            "profile",
            "implementation",
            "version",
            "declared_features",
            "python",
            "host",
            "installation",
            "target_execution",
        },
        "runtime identity stable projection",
    )
    if stable["schema"] != SCHEMA:
        raise RuntimeIdentityError("runtime identity schema is unsupported")
    side = stable["side"]
    if side not in {"source", "target"}:
        raise RuntimeIdentityError("runtime identity side is invalid")
    profile = _nonempty_string(stable["profile"], "stable.profile")
    implementation = _nonempty_string(stable["implementation"], "stable.implementation")
    _nonempty_string(stable["version"], "stable.version")

    features = stable["declared_features"]
    if not isinstance(features, list) or any(
        not isinstance(feature, str) or not feature for feature in features
    ):
        raise RuntimeIdentityError("stable.declared_features must be a string list")
    if len(set(features)) != len(features) or any(
        feature not in _TARGET_FEATURE_NAMES for feature in features
    ):
        raise RuntimeIdentityError("stable.declared_features contains unknown or duplicate values")
    if side == "source":
        expected_implementation = _SOURCE_PROFILE_IMPLEMENTATIONS.get(profile)
        if expected_implementation is None or implementation != expected_implementation:
            raise RuntimeIdentityError("source profile does not match implementation")
        if features:
            raise RuntimeIdentityError("source identity cannot declare target build features")
    elif implementation != "pillow-rs":
        raise RuntimeIdentityError("target implementation is invalid")

    python = _exact_keys(
        stable["python"],
        {
            "implementation",
            "version",
            "version_info",
            "build",
            "full_version",
            "cache_tag",
            "soabi",
            "platform",
            "executable",
        },
        "stable.python",
    )
    _nonempty_string(python["implementation"], "stable.python.implementation")
    _nonempty_string(python["version"], "stable.python.version")
    version_info = python["version_info"]
    if (
        not isinstance(version_info, list)
        or len(version_info) != 5
        or any(type(value) is not int for value in version_info[:3])
        or not isinstance(version_info[3], str)
        or type(version_info[4]) is not int
    ):
        raise RuntimeIdentityError("stable.python.version_info has an invalid shape")
    build = python["build"]
    if not isinstance(build, list) or len(build) != 2 or any(
        not isinstance(item, str) for item in build
    ):
        raise RuntimeIdentityError("stable.python.build has an invalid shape")
    _nonempty_string(python["full_version"], "stable.python.full_version")
    if python["cache_tag"] is not None and not isinstance(python["cache_tag"], str):
        raise RuntimeIdentityError("stable.python.cache_tag must be a string or null")
    if python["soabi"] is not None and not isinstance(python["soabi"], str):
        raise RuntimeIdentityError("stable.python.soabi must be a string or null")
    _nonempty_string(python["platform"], "stable.python.platform")
    _validate_file_record(python["executable"], "stable.python.executable", require_stat=True)

    host = _exact_keys(
        stable["host"],
        {
            "node",
            "system",
            "release",
            "version",
            "machine",
            "processor",
            "platform",
            "runner_id",
        },
        "stable.host",
    )
    for key in ("node", "system", "release", "version", "machine", "processor", "platform"):
        _nonempty_string(host[key], f"stable.host.{key}")
    if host["runner_id"] is not None and not isinstance(host["runner_id"], str):
        raise RuntimeIdentityError("stable.host.runner_id must be a string or null")

    installation = _exact_keys(
        stable["installation"],
        {"PIL", "pillow_rs", "target_PIL_facade"},
        "stable.installation",
    )
    pil_installation = _validate_installation_fingerprint(
        installation["PIL"], "stable.installation.PIL", "PIL"
    )
    target_execution = stable["target_execution"]
    if side == "source":
        if installation["pillow_rs"] is not None or installation["target_PIL_facade"] is not None:
            raise RuntimeIdentityError("source identity contains target installation fingerprints")
        if target_execution is not None:
            raise RuntimeIdentityError("source identity contains target execution state")
    else:
        _validate_installation_fingerprint(
            installation["pillow_rs"], "stable.installation.pillow_rs", "pillow_rs"
        )
        facade = _validate_installation_fingerprint(
            installation["target_PIL_facade"],
            "stable.installation.target_PIL_facade",
            "PIL",
        )
        if facade != pil_installation:
            raise RuntimeIdentityError("target PIL installation fingerprints disagree")
        target = _exact_keys(
            target_execution,
            {
                "backend",
                "available_backends",
                "active_backends",
                "parallel_feature_enabled",
                "selected_core",
            },
            "stable.target_execution",
        )
        backend = target["backend"]
        expected_backend = _TARGET_PROFILE_BACKENDS.get(profile)
        if expected_backend is None or backend != expected_backend or backend not in _BACKENDS:
            raise RuntimeIdentityError("stable target backend does not match profile")
        available = target["available_backends"]
        active = target["active_backends"]
        if (
            not isinstance(available, list)
            or any(item not in _BACKENDS for item in available)
            or available != sorted(set(available))
            or backend not in available
        ):
            raise RuntimeIdentityError("stable target available_backends is invalid")
        if active != [backend]:
            raise RuntimeIdentityError("stable target active_backends is invalid")
        parallel_enabled = target["parallel_feature_enabled"]
        parallel_profile = profile == "python-parallel-cpu"
        if type(parallel_enabled) is not bool or parallel_enabled != parallel_profile:
            raise RuntimeIdentityError("stable target parallel feature does not match profile")
        if not _DEFAULT_FEATURES.issubset(set(features)):
            raise RuntimeIdentityError("stable target features omit default features")
        declared_parallel = set(features) & _PARALLEL_FEATURES
        if (parallel_enabled and declared_parallel != _PARALLEL_FEATURES) or (
            not parallel_enabled and declared_parallel
        ):
            raise RuntimeIdentityError("stable target features disagree with parallel state")
        core = _exact_keys(
            target["selected_core"],
            {"module_name", "spec_name", "file", "mapping"},
            "stable.target_execution.selected_core",
        )
        module_name = core["module_name"]
        if module_name not in {"pillow_rs._core", "pillow_rs._core_parallel"}:
            raise RuntimeIdentityError("stable selected core module name is invalid")
        if core["spec_name"] != module_name:
            raise RuntimeIdentityError("stable selected core spec name is invalid")
        if module_name == "pillow_rs._core_parallel" and not parallel_enabled:
            raise RuntimeIdentityError("stable _core_parallel selected for serial profile")
        if module_name == "pillow_rs._core" and parallel_enabled and not parallel_profile:
            raise RuntimeIdentityError("stable parallel _core selected for serial profile")
        core_file = _validate_file_record(
            core["file"], "stable.target_execution.selected_core.file", require_stat=True
        )
        if not core_file["path"].endswith((".so", ".dylib", ".pyd", ".dll", ".bundle")):
            raise RuntimeIdentityError("stable selected core file is not a native module")
        mapping = _exact_keys(
            core["mapping"],
            {"status", "reason", "device", "inode"},
            "stable.target_execution.selected_core.mapping",
        )
        if mapping["status"] == "verified":
            if mapping["reason"] is not None:
                raise RuntimeIdentityError("verified mapping cannot have an unavailable reason")
            if (
                mapping["device"] != core_file["stat"]["device"]
                or mapping["inode"] != core_file["stat"]["inode"]
            ):
                raise RuntimeIdentityError("verified mapping identity differs from core file")
        elif mapping["status"] == "unavailable":
            if mapping["reason"] != "unsupported_platform":
                raise RuntimeIdentityError("unavailable mapping reason is invalid")
            if mapping["device"] is not None or mapping["inode"] is not None:
                raise RuntimeIdentityError("unavailable mapping cannot declare device/inode")
            if python["platform"].startswith("linux"):
                raise RuntimeIdentityError("Linux target identity must verify its loaded core mapping")
        else:
            raise RuntimeIdentityError("selected core mapping status is invalid")

    loaded = top["loaded_modules"]
    if not isinstance(loaded, list):
        raise RuntimeIdentityError("loaded_modules must be a list")
    allowed_prefixes = ("PIL",) if side == "source" else ("PIL", "pillow_rs")
    names: list[str] = []
    modules_by_name: dict[str, dict[str, Any]] = {}
    for index, value in enumerate(loaded):
        item = _exact_keys(
            value,
            {"module", "path", "sha256", "size", "origin"},
            f"loaded_modules[{index}]",
        )
        name = _nonempty_string(item["module"], f"loaded_modules[{index}].module")
        if not any(name == prefix or name.startswith(prefix + ".") for prefix in allowed_prefixes):
            raise RuntimeIdentityError(f"loaded module {name!r} is outside the selected implementation")
        names.append(name)
        modules_by_name[name] = item
        if item["origin"] is not None and not isinstance(item["origin"], str):
            raise RuntimeIdentityError(f"loaded_modules[{index}].origin must be a string or null")
        if item["path"] is None:
            if item["sha256"] is not None or item["size"] is not None:
                raise RuntimeIdentityError(f"loaded_modules[{index}] has a partial file identity")
        else:
            path = _nonempty_string(item["path"], f"loaded_modules[{index}].path")
            if not Path(path).is_absolute():
                raise RuntimeIdentityError(f"loaded_modules[{index}].path must be absolute")
            _sha256_string(item["sha256"], f"loaded_modules[{index}].sha256")
            _nonnegative_int(item["size"], f"loaded_modules[{index}].size")
            if name == "PIL" or name.startswith("PIL."):
                packages = [pil_installation]
                if side == "target":
                    packages.append(installation["pillow_rs"])
            else:
                packages = [installation["pillow_rs"]]
            if not any(
                _package_file_matches(
                    package,
                    path,
                    item["sha256"],
                    item["size"],
                )
                for package in packages
            ):
                raise RuntimeIdentityError(
                    f"loaded module {name!r} does not match its installation fingerprint"
                )
    if names != sorted(set(names)):
        raise RuntimeIdentityError("loaded_modules must be unique and sorted by module name")
    if "PIL" not in names:
        raise RuntimeIdentityError("loaded module facts omit handshake-required PIL")
    pil_module = modules_by_name["PIL"]
    if pil_module["path"] is None or not _package_file_matches(
        pil_installation,
        pil_module["path"],
        pil_module["sha256"],
        pil_module["size"],
    ):
        raise RuntimeIdentityError("loaded PIL module does not match its installation fingerprint")
    if side == "target":
        if "pillow_rs" not in names:
            raise RuntimeIdentityError("loaded module facts omit handshake-required pillow_rs")
        pillow_rs_module = modules_by_name["pillow_rs"]
        if pillow_rs_module["path"] is None or not _package_file_matches(
            installation["pillow_rs"],
            pillow_rs_module["path"],
            pillow_rs_module["sha256"],
            pillow_rs_module["size"],
        ):
            raise RuntimeIdentityError(
                "loaded pillow_rs module does not match its installation fingerprint"
            )
        selected_core_name = target_execution["selected_core"]["module_name"]
        if selected_core_name not in names:
            raise RuntimeIdentityError("loaded module facts omit the selected core module")
        selected_core = modules_by_name[selected_core_name]
        selected_file = target_execution["selected_core"]["file"]
        if (
            selected_core["path"] != selected_file["path"]
            or selected_core["sha256"] != selected_file["sha256"]
            or selected_core["size"] != selected_file["size"]
            or not _package_file_matches(
                installation["pillow_rs"],
                selected_file["path"],
                selected_file["sha256"],
                selected_file["size"],
            )
        ):
            raise RuntimeIdentityError(
                "selected core module facts do not match the target installation fingerprint"
            )

    expected_stable_sha = _canonical_json_sha256(stable)
    if _sha256_string(top["stable_sha256"], "stable_sha256") != expected_stable_sha:
        raise RuntimeIdentityError("stable_sha256 does not match the stable projection")

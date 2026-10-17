"""Synthetic contract tests for migration execution identity capture."""

from __future__ import annotations

import copy
import importlib.machinery
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

try:
    import migration_execution_identity as identity
except ModuleNotFoundError:  # imported as scripts.test_migration_execution_identity
    from scripts import migration_execution_identity as identity


DEFAULT_FEATURES = ["pillow-rs-py/default", "pillow-rs/default"]
PARALLEL_FEATURES = DEFAULT_FEATURES + [
    "pillow-rs-py/parallel",
    "pillow-rs/parallel",
]


def _package_module(name: str, root: Path, version: str) -> types.ModuleType:
    root.mkdir(parents=True, exist_ok=True)
    init_path = root / "__init__.py"
    init_path.write_text("# synthetic package\n", encoding="utf-8")
    module = types.ModuleType(name)
    module.__file__ = str(init_path)
    module.__path__ = [str(root)]
    module.__spec__ = importlib.machinery.ModuleSpec(
        name,
        loader=None,
        origin=str(init_path),
        is_package=True,
    )
    module.__version__ = version
    return module


def _core_module(name: str, path: Path, parallel: bool) -> types.ModuleType:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"synthetic native extension\n")
    module = types.ModuleType(name)
    module.__file__ = str(path)
    module.__spec__ = importlib.machinery.ModuleSpec(
        name,
        loader=None,
        origin=str(path),
    )
    module.parallel_feature_enabled = lambda: parallel
    return module


class RuntimeIdentityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="execution-identity-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self._saved_modules = {
            name: module
            for name, module in list(sys.modules.items())
            if name == "PIL"
            or name.startswith("PIL.")
            or name == "pillow_rs"
            or name.startswith("pillow_rs.")
        }
        for name in list(self._saved_modules):
            sys.modules.pop(name, None)
        self.addCleanup(self._restore_modules)

    def _restore_modules(self) -> None:
        for name in list(sys.modules):
            if (
                name == "PIL"
                or name.startswith("PIL.")
                or name == "pillow_rs"
                or name.startswith("pillow_rs.")
            ):
                sys.modules.pop(name, None)
        sys.modules.update(self._saved_modules)

    def _source(self) -> tuple[types.ModuleType, dict[str, str]]:
        pil_root = self.root / "source" / "PIL"
        pil = _package_module("PIL", pil_root, "12.2.0")
        sys.modules["PIL"] = pil
        handshake = {
            "side": "source",
            "implementation": "Pillow",
            "version": "12.2.0",
            "path": str(Path(pil.__file__).resolve()),
            "public_module": "PIL",
        }
        return pil, handshake

    def _target(
        self,
        *,
        core_name: str = "pillow_rs._core",
        parallel: bool = False,
        active: list[str] | None = None,
        backend: str = "cpu",
    ) -> tuple[types.ModuleType, types.ModuleType, types.ModuleType, dict[str, object]]:
        target_root = self.root / "target" / "pillow_rs"
        pil_root = self.root / "target" / "PIL"
        target = _package_module("pillow_rs", target_root, "12.2.1")
        pil = _package_module("PIL", pil_root, "12.2.1")
        core_path = target_root / ("_core_parallel.abi3.so" if core_name.endswith("_core_parallel") else "_core.abi3.so")
        core = _core_module(core_name, core_path, parallel)
        target._core = core
        target.active_backends = lambda: list(active if active is not None else [backend])
        target.available_backends = lambda: ["cpu", "gpu", "simd"]
        target.parallel_feature_enabled = lambda: parallel
        sys.modules["pillow_rs"] = target
        sys.modules[core_name] = core
        if core_name.endswith("_core_parallel"):
            # Companion wheels expose the parallel extension under _core as
            # well, while the module's actual name/spec stay _core_parallel.
            sys.modules["pillow_rs._core"] = core
        sys.modules["PIL"] = pil
        handshake: dict[str, object] = {
            "side": "target",
            "implementation": "pillow-rs",
            "version": "12.2.1",
            "path": str(Path(target.__file__).resolve()),
            "public_module": "PIL",
            "public_path": str(Path(pil.__file__).resolve()),
            "backend": backend,
            "backend_state": {
                "requested": backend,
                "available": ["cpu", "gpu", "simd"],
                "active": list(active if active is not None else [backend]),
            },
        }
        return target, pil, core, handshake

    def _capture_target(
        self,
        handshake: dict[str, object],
        *,
        profile: str = "python-cpu",
        features: list[str] | None = None,
    ) -> dict[str, object]:
        def matched_mapping(_path: Path, stat_record: dict[str, int]) -> dict[str, object]:
            return {
                "status": "verified",
                "reason": None,
                "device": stat_record["device"],
                "inode": stat_record["inode"],
            }

        with mock.patch.object(
            identity, "_loaded_mapping_identity", side_effect=matched_mapping
        ):
            return identity.capture_runtime_identity(
                "target",
                handshake,
                profile,
                DEFAULT_FEATURES if features is None else features,
            )

    def test_target_accepts_loaded_serial_core_and_emits_fixed_identity(self) -> None:
        _target, _pil, _core, handshake = self._target()
        snapshot = self._capture_target(handshake)

        identity.validate_runtime_identity(snapshot)
        self.assertEqual(
            set(snapshot), {"stable", "loaded_modules", "stable_sha256"}
        )
        self.assertEqual(snapshot["stable"]["target_execution"]["active_backends"], ["cpu"])
        self.assertFalse(
            snapshot["stable"]["target_execution"]["parallel_feature_enabled"]
        )
        self.assertIn("pillow_rs._core", [item["module"] for item in snapshot["loaded_modules"]])

    def test_parallel_core_alias_and_parallel_profile_are_accepted(self) -> None:
        _target, _pil, _core, handshake = self._target(
            core_name="pillow_rs._core_parallel",
            parallel=True,
        )
        snapshot = self._capture_target(
            handshake,
            profile="python-parallel-cpu",
            features=PARALLEL_FEATURES,
        )

        self.assertEqual(
            snapshot["stable"]["target_execution"]["selected_core"]["module_name"],
            "pillow_rs._core_parallel",
        )
        self.assertIn("pillow_rs._core", [item["module"] for item in snapshot["loaded_modules"]])
        self.assertIn(
            "pillow_rs._core_parallel",
            [item["module"] for item in snapshot["loaded_modules"]],
        )

    def test_canonical_core_accepts_parallel_source_build_only_for_parallel_cpu(self) -> None:
        _target, _pil, _core, handshake = self._target(parallel=True)
        snapshot = self._capture_target(
            handshake,
            profile="python-parallel-cpu",
            features=PARALLEL_FEATURES,
        )
        self.assertEqual(
            snapshot["stable"]["target_execution"]["selected_core"]["module_name"],
            "pillow_rs._core",
        )

    def test_target_rejects_feature_backend_and_core_profile_mismatches(self) -> None:
        _target, _pil, _core, handshake = self._target(parallel=True)
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake, profile="python-cpu", features=PARALLEL_FEATURES)

        _target, _pil, _core, handshake = self._target(active=["simd"])
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake)

        _target, _pil, _core, handshake = self._target(
            core_name="pillow_rs._core_parallel",
            parallel=False,
        )
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake)

        _target, _pil, _core, handshake = self._target(parallel=False)
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake, features=PARALLEL_FEATURES)

    def test_selected_core_requires_exact_loaded_spec_and_regular_file(self) -> None:
        _target, _pil, core, handshake = self._target()
        core.__spec__.name = "pillow_rs.unexpected"
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake)

        core.__spec__.name = "pillow_rs._core"
        core.__file__ = str(self.root / "target" / "pillow_rs")
        core.__spec__.origin = core.__file__
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake)

    def test_missing_or_unreadable_required_modules_fail_closed(self) -> None:
        _target, _pil, core, handshake = self._target()
        del sys.modules["pillow_rs._core"]
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake)

        sys.modules["pillow_rs._core"] = core
        core.__file__ = str(self.root / "target" / "pillow_rs")
        core.__spec__.origin = core.__file__
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(handshake)

        sys.modules.pop("PIL", None)
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.capture_runtime_identity(
                "source",
                {
                    "side": "source",
                    "implementation": "Pillow",
                    "version": "12.2.0",
                    "path": "/missing/PIL/__init__.py",
                    "public_module": "PIL",
                },
                "pillow",
                [],
            )

    def test_file_replacement_during_hash_is_rejected(self) -> None:
        _target, _pil, core, handshake = self._target()
        core_path = Path(core.__file__).resolve()
        original_hash = identity._stream_sha256
        replaced = False

        def replace_after_read(stream: object) -> tuple[str, int]:
            nonlocal replaced
            result = original_hash(stream)
            if not replaced and Path(stream.name).resolve() == core_path:
                replacement = core_path.with_name(core_path.name + ".replacement")
                replacement.write_bytes(b"replacement native bytes\n")
                replacement.replace(core_path)
                replaced = True
            return result

        with mock.patch.object(identity, "_stream_sha256", side_effect=replace_after_read):
            with mock.patch.object(
                identity,
                "_loaded_mapping_identity",
                return_value={
                    "status": "unavailable",
                    "reason": "unsupported_platform",
                    "device": None,
                    "inode": None,
                },
            ):
                with self.assertRaises(identity.RuntimeIdentityError):
                    identity.capture_runtime_identity(
                        "target", handshake, "python-cpu", DEFAULT_FEATURES
                    )
        self.assertTrue(replaced)

    def test_linux_mapping_inode_mismatch_is_rejected(self) -> None:
        file_path = self.root / "mapped-core.so"
        file_path.write_bytes(b"mapped bytes")
        stat_record = identity._stat_identity(file_path.stat())
        device = f"{os.major(stat_record['device']):x}:{os.minor(stat_record['device']):x}"
        matching_line = (
            f"1000-2000 r--p 00000000 {device} {stat_record['inode']} {file_path}\n"
        )
        mismatching_line = (
            f"1000-2000 r--p 00000000 {device} {stat_record['inode'] + 1} {file_path}\n"
        )
        with mock.patch.object(identity.sys, "platform", "linux"):
            with mock.patch.object(Path, "read_text", return_value=matching_line):
                mapping = identity._loaded_mapping_identity(file_path, stat_record)
            self.assertEqual(mapping["status"], "verified")
            with mock.patch.object(Path, "read_text", return_value=mismatching_line):
                with self.assertRaises(identity.RuntimeIdentityError):
                    identity._loaded_mapping_identity(file_path, stat_record)

    def test_installation_fingerprint_excludes_cache_and_ignores_loaded_subset(self) -> None:
        pil, handshake = self._source()
        image_path = Path(pil.__path__[0]) / "Image.py"
        image_path.write_text("# optional image module\n", encoding="utf-8")
        cache_path = Path(pil.__path__[0]) / "__pycache__" / "Image.cpython-test.pyc"
        cache_path.parent.mkdir()
        cache_path.write_bytes(b"cache one")

        first = identity.capture_runtime_identity("source", handshake, "pillow", [])
        image = types.ModuleType("PIL.Image")
        image.__file__ = str(image_path)
        image.__spec__ = importlib.machinery.ModuleSpec(
            "PIL.Image", loader=None, origin=str(image_path)
        )
        sys.modules["PIL.Image"] = image
        cache_path.write_bytes(b"cache two")
        second = identity.capture_runtime_identity("source", handshake, "pillow", [])

        self.assertEqual(first["stable_sha256"], second["stable_sha256"])
        self.assertNotEqual(first["loaded_modules"], second["loaded_modules"])
        files = second["stable"]["installation"]["PIL"]["files"]
        self.assertIn("Image.py", [item["relative_path"] for item in files])
        self.assertFalse(any("__pycache__" in item["relative_path"] for item in files))

    def test_source_capture_does_not_import_optional_image_modules(self) -> None:
        _pil, handshake = self._source()
        self.assertNotIn("PIL.Image", sys.modules)
        self.assertNotIn("PIL._imaging", sys.modules)

        snapshot = identity.capture_runtime_identity("source", handshake, "pillow", [])

        self.assertIn("PIL", [item["module"] for item in snapshot["loaded_modules"]])
        self.assertNotIn("PIL.Image", sys.modules)
        self.assertNotIn("PIL._imaging", sys.modules)

    def test_source_profile_must_match_source_implementation(self) -> None:
        _pil, handshake = self._source()
        pillow_snapshot = identity.capture_runtime_identity(
            "source", handshake, "pillow", []
        )
        identity.validate_runtime_identity(pillow_snapshot)

        with self.assertRaises(identity.RuntimeIdentityError):
            identity.capture_runtime_identity("source", handshake, "pillow-simd", [])
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.capture_runtime_identity("source", handshake, "Pillow", [])

        pillow_simd_handshake = dict(handshake, implementation="Pillow-SIMD")
        pillow_simd_snapshot = identity.capture_runtime_identity(
            "source", pillow_simd_handshake, "pillow-simd", []
        )
        identity.validate_runtime_identity(pillow_simd_snapshot)
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.capture_runtime_identity(
                "source", pillow_simd_handshake, "pillow", []
            )

        forged_snapshot = copy.deepcopy(pillow_snapshot)
        forged_snapshot["stable"]["implementation"] = "Pillow-SIMD"
        forged_snapshot["stable_sha256"] = identity._canonical_json_sha256(
            forged_snapshot["stable"]
        )
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.validate_runtime_identity(forged_snapshot)

    def test_filebacked_loaded_modules_match_installation_fingerprints(self) -> None:
        _pil, handshake = self._source()
        outside = self.root / "outside" / "Image.py"
        outside.parent.mkdir()
        outside.write_text("# not part of the installed package\n", encoding="utf-8")
        image = types.ModuleType("PIL.Image")
        image.__file__ = str(outside)
        image.__spec__ = importlib.machinery.ModuleSpec(
            "PIL.Image", loader=None, origin=str(outside)
        )
        sys.modules["PIL.Image"] = image
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.capture_runtime_identity("source", handshake, "pillow", [])
        sys.modules.pop("PIL.Image", None)

        _target, _target_pil, _core, target_handshake = self._target()
        outside.write_text("# not part of the installed package\n", encoding="utf-8")
        target_image = types.ModuleType("PIL.Image")
        target_image.__file__ = str(outside)
        target_image.__spec__ = importlib.machinery.ModuleSpec(
            "PIL.Image", loader=None, origin=str(outside)
        )
        sys.modules["PIL.Image"] = target_image
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(target_handshake)
        sys.modules.pop("PIL.Image", None)

        extra_path = self.root / "outside" / "pillow_rs_extra.py"
        extra_path.write_text("# not part of the installed package\n", encoding="utf-8")
        extra = types.ModuleType("pillow_rs.extra")
        extra.__file__ = str(extra_path)
        extra.__spec__ = importlib.machinery.ModuleSpec(
            "pillow_rs.extra", loader=None, origin=str(extra_path)
        )
        sys.modules["pillow_rs.extra"] = extra
        with self.assertRaises(identity.RuntimeIdentityError):
            self._capture_target(target_handshake)
        sys.modules.pop("pillow_rs.extra", None)

    def test_target_pil_alias_may_resolve_within_pillow_rs_installation(self) -> None:
        target, _pil, _core, handshake = self._target()
        alias_path = Path(target.__path__[0]) / "ImageChops.py"
        alias_path.write_text("# implementation-backed PIL alias\n", encoding="utf-8")
        alias = types.ModuleType("PIL.ImageChops")
        alias.__file__ = str(alias_path)
        alias.__spec__ = importlib.machinery.ModuleSpec(
            "PIL.ImageChops", loader=None, origin=str(alias_path)
        )
        sys.modules["PIL.ImageChops"] = alias

        snapshot = self._capture_target(handshake)

        self.assertIn(
            "PIL.ImageChops",
            [item["module"] for item in snapshot["loaded_modules"]],
        )

    def test_snapshot_validator_rejects_unknown_fields_and_bad_hashes(self) -> None:
        _pil, handshake = self._source()
        snapshot = identity.capture_runtime_identity("source", handshake, "pillow", [])
        with_unknown_field = copy.deepcopy(snapshot)
        with_unknown_field["stable"]["extra"] = "not permitted"
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.validate_runtime_identity(with_unknown_field)

        with_bad_hash = copy.deepcopy(snapshot)
        with_bad_hash["stable_sha256"] = "0" * 64
        with self.assertRaises(identity.RuntimeIdentityError):
            identity.validate_runtime_identity(with_bad_hash)


if __name__ == "__main__":
    unittest.main()

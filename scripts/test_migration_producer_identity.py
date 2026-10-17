"""Synthetic contracts for producer bundle and loaded-module identity."""

from __future__ import annotations

import importlib.machinery
import importlib.util
from pathlib import Path
import sys
import tempfile
import types
import unittest

try:
    import migration_producer_identity as producer
except ModuleNotFoundError:  # imported as scripts.test_migration_producer_identity
    from scripts import migration_producer_identity as producer


class ProducerIdentityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="producer-identity-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "scripts"
        self.root.mkdir()
        (self.root / "run_migration_parity.py").write_text(
            "# frozen child runner\n", encoding="utf-8"
        )
        (self.root / "producer_helper.py").write_text(
            "VALUE = 1\n", encoding="utf-8"
        )
        (self.root / "test_excluded.py").write_text(
            "# excluded test source\n", encoding="utf-8"
        )
        self.parent = self.base / "native_driver.py"
        self.parent.write_text("# parent driver\n", encoding="utf-8")

    @staticmethod
    def _module(name: str, path: Path | None = None) -> types.ModuleType:
        module = types.ModuleType(name)
        if path is not None:
            resolved = path.resolve()
            module.__file__ = str(resolved)
            module.__spec__ = importlib.machinery.ModuleSpec(
                name, loader=None, origin=str(resolved)
            )
        return module

    def _bundle(self, *, extras: bool = False) -> dict[str, object]:
        main = self._module("__main__", self.parent)
        modules: dict[str, object] = {"__main__": main}
        if extras:
            sitecustomize_path = self.base / "sitecustomize.py"
            sitecustomize_path.write_text("# synthetic site customizer\n", encoding="utf-8")
            modules["sitecustomize"] = self._module("sitecustomize", sitecustomize_path)
        return producer.capture_producer_bundle(
            self.root, self.parent, modules=modules
        )

    def _child_modules(self, bundle: dict[str, object]) -> dict[str, object]:
        files = bundle["files"]
        helper_record = next(
            item for item in files if item["relative_path"] == "producer_helper.py"
        )
        child_record = bundle["entrypoints"]["child"]
        child = self._module("__main__", Path(child_record["path"]))
        helper = self._module("producer_helper", Path(helper_record["resolved_path"]))
        namespace = types.ModuleType("scripts")
        namespace.__path__ = [str(self.root)]
        namespace.__spec__ = importlib.machinery.ModuleSpec(
            "scripts", loader=None, origin=None, is_package=True
        )
        return {
            "__main__": child,
            "producer_helper": helper,
            "scripts.producer_helper": helper,
            "scripts": namespace,
        }

    def test_bundle_captures_sorted_inventory_and_parent_child_entrypoints(self) -> None:
        (self.root / "z_last.py").write_text("Z = 1\n", encoding="utf-8")
        (self.root / "a_first.py").write_text("A = 1\n", encoding="utf-8")
        bundle = self._bundle()

        self.assertEqual(bundle["schema"], producer.BUNDLE_SCHEMA)
        self.assertEqual(bundle["root"], str(self.root.resolve()))
        paths = [item["relative_path"] for item in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        self.assertNotIn("test_excluded.py", paths)
        self.assertEqual(
            Path(bundle["entrypoints"]["child"]["path"]),
            (self.root / producer.CHILD_ENTRYPOINT).resolve(),
        )
        self.assertEqual(bundle["entrypoints"]["parent"]["path"], str(self.parent.resolve()))

    def test_bundle_rejects_unsafe_paths_and_symlink_escapes(self) -> None:
        bundle = self._bundle()
        changed = dict(bundle)
        changed["files"] = [dict(record) for record in bundle["files"]]
        changed["files"][0]["relative_path"] = "../outside.py"
        with self.assertRaises(producer.ProducerIdentityError):
            producer.validate_producer_bundle(changed)

        outside = self.base / "outside.py"
        outside.write_text("SECRET = True\n", encoding="utf-8")
        (self.root / "escaped.py").symlink_to(outside)
        with self.assertRaises(producer.ProducerIdentityError):
            self._bundle()

    def test_bundle_rejects_changed_files_and_membership(self) -> None:
        bundle = self._bundle()
        (self.root / "producer_helper.py").write_text("VALUE = 2\n", encoding="utf-8")
        with self.assertRaises(producer.ProducerIdentityError):
            producer.validate_producer_bundle(bundle)

        bundle = self._bundle()
        (self.root / "late_module.py").write_text("LATE = True\n", encoding="utf-8")
        with self.assertRaises(producer.ProducerIdentityError):
            producer.validate_producer_bundle(bundle)

    def test_bundle_requires_in_root_parent_entrypoint_in_inventory(self) -> None:
        parent = self.root / "test_native_driver.py"
        parent.write_text("# excluded parent entrypoint\n", encoding="utf-8")
        modules = {"__main__": self._module("__main__", parent)}

        with self.assertRaises(producer.ProducerIdentityError):
            producer.capture_producer_bundle(self.root, parent, modules=modules)

    def test_bundle_captures_sitecustomize_without_importing_it(self) -> None:
        before = sys.modules.get("sitecustomize")
        bundle = self._bundle(extras=True)

        self.assertEqual([item["module"] for item in bundle["extras"]], ["sitecustomize"])
        self.assertIs(sys.modules.get("sitecustomize"), before)

    def test_identity_records_loaded_producer_modules_and_allows_aliases(self) -> None:
        bundle = self._bundle()
        identity = producer.capture_producer_identity(
            bundle, role="child", modules=self._child_modules(bundle)
        )
        producer.validate_producer_identity(identity, bundle)

        names = [item["module"] for item in identity["loaded_modules"]]
        self.assertEqual(names, sorted(names))
        self.assertIn("__main__", names)
        self.assertIn("producer_helper", names)
        self.assertIn("scripts.producer_helper", names)
        self.assertEqual(
            next(item for item in identity["loaded_modules"] if item["module"] == "producer_helper")["path"],
            next(item for item in identity["loaded_modules"] if item["module"] == "scripts.producer_helper")["path"],
        )

    def test_identity_accepts_cached_bytecode_for_owned_source(self) -> None:
        bundle = self._bundle()
        modules = self._child_modules(bundle)
        source = self.root / "producer_helper.py"
        cached = Path(importlib.util.cache_from_source(str(source)))
        cached.parent.mkdir(parents=True, exist_ok=True)
        cached.write_bytes(b"synthetic bytecode payload")
        helper = types.ModuleType("producer_helper")
        helper.__file__ = str(cached)
        helper.__spec__ = importlib.machinery.ModuleSpec(
            "producer_helper", loader=None, origin=str(source.resolve())
        )
        modules["producer_helper"] = helper
        modules["scripts.producer_helper"] = helper
        identity = producer.capture_producer_identity(
            bundle, role="child", modules=modules
        )

        recorded = next(
            item for item in identity["loaded_modules"]
            if item["module"] == "producer_helper"
        )
        self.assertEqual(recorded["path"], str(source.resolve()))

    def test_identity_rejects_non_string_role(self) -> None:
        bundle = self._bundle()
        with self.assertRaises(producer.ProducerIdentityError):
            producer.capture_producer_identity(
                bundle, role=[], modules=self._child_modules(bundle)
            )
        identity = producer.capture_producer_identity(
            bundle, role="child", modules=self._child_modules(bundle)
        )
        identity["role"] = []
        with self.assertRaises(producer.ProducerIdentityError):
            producer.validate_producer_identity(identity, bundle)

    def test_identity_rejects_missing_or_changed_expected_extra(self) -> None:
        bundle = self._bundle(extras=True)
        modules = self._child_modules(bundle)
        with self.assertRaises(producer.ProducerIdentityError):
            producer.capture_producer_identity(bundle, role="child", modules=modules)

        modules["sitecustomize"] = self._module(
            "sitecustomize", Path(bundle["extras"][0]["path"])
        )
        changed_path = Path(bundle["extras"][0]["path"])
        changed_path.write_text("# changed\n", encoding="utf-8")
        with self.assertRaises(producer.ProducerIdentityError):
            producer.capture_producer_identity(bundle, role="child", modules=modules)

    def test_identity_rejects_unowned_producer_module(self) -> None:
        bundle = self._bundle()
        modules = self._child_modules(bundle)
        outside = self.base / "producer_helper.py"
        outside.write_text("VALUE = 99\n", encoding="utf-8")
        modules["producer_helper"] = self._module("producer_helper", outside)
        with self.assertRaises(producer.ProducerIdentityError):
            producer.capture_producer_identity(bundle, role="child", modules=modules)

        modules = self._child_modules(bundle)
        excluded = self.root / "test_runtime.py"
        excluded.write_text("# not inventoried\n", encoding="utf-8")
        modules["test_runtime"] = self._module("test_runtime", excluded)
        with self.assertRaises(producer.ProducerIdentityError):
            producer.capture_producer_identity(bundle, role="child", modules=modules)

    def test_validation_rejects_modified_identity_or_bundle(self) -> None:
        bundle = self._bundle()
        identity = producer.capture_producer_identity(
            bundle, role="child", modules=self._child_modules(bundle)
        )
        changed_identity = dict(identity)
        changed_identity["files_sha256"] = "0" * 64
        with self.assertRaises(producer.ProducerIdentityError):
            producer.validate_producer_identity(changed_identity, bundle)

        changed_bundle = dict(bundle)
        changed_bundle["files"] = [dict(record) for record in bundle["files"]]
        changed_bundle["files"][0]["sha256"] = "0" * 64
        with self.assertRaises(producer.ProducerIdentityError):
            producer.validate_producer_identity(identity, changed_bundle)


if __name__ == "__main__":
    unittest.main()

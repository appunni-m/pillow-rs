"""Bounded fixture contracts for the fresh-build witness helper."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

try:
    import migration_build_witness as witness
except ModuleNotFoundError:
    from scripts import migration_build_witness as witness


def _json_bytes(document: dict) -> bytes:
    return json.dumps(document, sort_keys=True, indent=2, allow_nan=False).encode("utf-8") + b"\n"


class MigrationBuildWitnessTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "workspace"
        self.root.mkdir()
        self._write("Cargo.toml", "[workspace]\nmembers=['pillow-rs','pillow-rs-py','pillow-rs-js']\n")
        self._write("Cargo.lock", "version = 4\n")
        self._write("rust-toolchain.toml", "[toolchain]\nchannel='stable'\n")
        self._write(".cargo/config.toml", "[build]\n")
        for package in ("pillow-rs", "pillow-rs-py", "pillow-rs-js"):
            self._write(f"{package}/Cargo.toml", f"[package]\nname='{package}'\n")
            self._write(f"{package}/src/lib.rs", "pub fn fixture() {}\n")

    def _write(self, relative: str, content: str | bytes) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        mode = "wb" if isinstance(content, bytes) else "w"
        with path.open(mode) as stream:
            stream.write(content)
        return path

    def test_snapshot_hashes_sorted_files_and_is_stable(self):
        first = witness.capture_source_snapshot(self.root)
        second = witness.capture_source_snapshot(self.root)
        self.assertEqual(first, second)
        paths = [record["path"] for record in first["files"]]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(
            first["sha256"],
            hashlib.sha256(
                json.dumps(first["files"], sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode("utf-8")
            ).hexdigest(),
        )
        self._write("pillow-rs/src/lib.rs", "pub fn changed() {}\n")
        self.assertNotEqual(first["sha256"], witness.capture_source_snapshot(self.root)["sha256"])

    def test_snapshot_adds_literal_include_outside_src(self):
        self._write("assets/payload.bin", b"embedded payload")
        self._write(
            "pillow-rs/src/lib.rs",
            'pub const PAYLOAD: &[u8] = include_bytes!("../../assets/payload.bin");\n',
        )
        snapshot = witness.capture_source_snapshot(self.root)
        record = next(item for item in snapshot["files"] if item["path"] == "assets/payload.bin")
        self.assertEqual(record["size"], len(b"embedded payload"))
        self.assertEqual(record["sha256"], hashlib.sha256(b"embedded payload").hexdigest())

    def test_snapshot_rejects_symlink_escape(self):
        outside = Path(self.temporary.name) / "outside.rs"
        outside.write_text("pub fn outside() {}\n", encoding="utf-8")
        escape = self.root / "pillow-rs" / "src" / "outside.rs"
        escape.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "escapes workspace"):
            witness.capture_source_snapshot(self.root)

    def _build_command(self, manifest: Path | None = None) -> list[str]:
        selected_manifest = manifest or (self.root / "pillow-rs-py" / "Cargo.toml")
        return [
            "cargo", "build", "--release", "--locked", "--message-format=json",
            "--manifest-path", str(selected_manifest),
        ]

    def _cargo_events(
        self,
        artifact: Path,
        *,
        finished: bool = True,
        fresh_by_package: dict[str, bool] | None = None,
    ) -> bytes:
        fresh_by_package = fresh_by_package or {}
        core_manifest = self.root / "pillow-rs" / "Cargo.toml"
        binding_manifest = self.root / "pillow-rs-py" / "Cargo.toml"
        messages = [
            {
                "reason": "compiler-artifact",
                "package_id": f"path+{core_manifest.parent.as_uri()}#12.2.1",
                "manifest_path": str(core_manifest),
                "target": {"name": "pillow_rs", "crate_types": ["lib"]},
                "features": ["default", "gpu"],
                "fresh": fresh_by_package.get("pillow-rs", False),
                "filenames": [],
            },
            {
                "reason": "compiler-artifact",
                "package_id": f"path+{binding_manifest.parent.as_uri()}#12.2.1",
                "manifest_path": str(binding_manifest),
                "target": {"name": "_core", "crate_types": ["cdylib"]},
                "features": ["parallel"],
                "fresh": fresh_by_package.get("pillow-rs-py", False),
                "filenames": [str(artifact)],
            },
            {"reason": "build-finished", "success": finished},
        ]
        return b"".join(json.dumps(message).encode("utf-8") + b"\n" for message in messages)

    def _old_parser_receipt(self, events: bytes) -> tuple[Path, Path, bytes]:
        output = self.root / "evidence" / "original.json"
        log_path = output.with_name(output.name + ".log")
        snapshot = witness.capture_source_snapshot(self.root)
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_bytes(events)
        log_record = {
            "path": str(log_path.resolve()),
            "size": len(events),
            "sha256": hashlib.sha256(events).hexdigest(),
        }
        receipt = {
            "schema": witness.SCHEMA,
            "status": "not_proven",
            "workspace": {"root": str(self.root.resolve()), "git_head": "fixture", "dirty": True},
            "source": {"before": snapshot, "after": snapshot},
            "build": {
                "argv": self._build_command(),
                "cwd": str(self.root.resolve()),
                "environment": {name: None for name in witness.CAPTURED_ENVIRONMENT},
                "cargo_version": "cargo fixture",
                "rustc_version": "rustc fixture",
                "target_triple": "x86_64-unknown-linux-gnu",
                "exit_code": 0,
                "log": log_record,
                "features": {package: [] for package in witness.FEATURE_PACKAGES},
                "fresh_packages": [],
            },
            "artifact": None,
            "errors": [
                "Cargo compiler-artifact did not report features for pillow-rs",
                "Cargo compiler-artifact did not report core artifact freshness for pillow-rs",
                "Cargo compiler-artifact did not report features for pillow-rs-py",
                "Cargo compiler-artifact did not report core artifact freshness for pillow-rs-py",
                "expected one Cargo-reported _core cdylib, found 0",
            ],
        }
        original_bytes = _json_bytes(receipt)
        output.write_bytes(original_bytes)
        return output, log_path, original_bytes

    def _mock_capture(self, build_output: bytes):
        artifact_command = self._build_command()

        def capture(command, *, cwd, environment):
            if command == ["git", "rev-parse", "HEAD"]:
                return 0, b"0123456789abcdef\n", None
            if command == ["git", "status", "--porcelain", "--untracked-files=normal"]:
                return 0, b"", None
            if len(command) >= 2 and command[-2:] == ["--version", "--verbose"]:
                if Path(command[0]).name == "cargo":
                    return 0, b"cargo 1.96.1\n", None
                return 0, b"rustc 1.96.1\nhost: x86_64-unknown-linux-gnu\n", None
            if command == artifact_command:
                return 0, build_output, None
            raise AssertionError(f"unexpected subprocess command: {command}")

        return capture

    def test_witness_records_complete_build_and_actual_features(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        output = self.root / "evidence" / "build.json"
        events = self._cargo_events(artifact)
        overrides = {
            "CARGO_BUILD_TARGET": "x86_64-unknown-linux-gnu",
            "RUSTFLAGS": "",
            "RUSTC_WRAPPER": None,
        }
        with mock.patch.object(witness, "_capture_output", side_effect=self._mock_capture(events)):
            document = witness.run_build_witness(self.root, output, self._build_command(), overrides)

        self.assertEqual(set(document), {"schema", "status", "workspace", "source", "build", "artifact", "errors"})
        self.assertEqual(set(document["workspace"]), {"root", "git_head", "dirty"})
        self.assertEqual(set(document["source"]), {"before", "after"})
        self.assertEqual(set(document["source"]["before"]), {"sha256", "files"})
        self.assertEqual(set(document["source"]["before"]["files"][0]), {"path", "size", "sha256"})
        self.assertEqual(
            set(document["build"]),
            {"argv", "cwd", "environment", "cargo_version", "rustc_version", "target_triple", "exit_code", "log", "features", "fresh_packages"},
        )
        self.assertEqual(set(document["build"]["log"]), {"path", "size", "sha256"})
        self.assertEqual(set(document["artifact"]), {"package_id", "target", "crate_types", "path", "size", "sha256"})
        self.assertEqual(document["schema"], "pillow-rs/build-witness@1")
        self.assertEqual(document["status"], "complete")
        self.assertEqual(document["source"]["before"], document["source"]["after"])
        self.assertEqual(document["build"]["target_triple"], "x86_64-unknown-linux-gnu")
        self.assertEqual(document["build"]["environment"]["RUSTFLAGS"], "")
        self.assertEqual(document["build"]["environment"]["CARGO_BUILD_TARGET"], "x86_64-unknown-linux-gnu")
        self.assertIsNone(document["build"]["environment"]["RUSTC_WRAPPER"])
        self.assertEqual(document["build"]["features"], {"pillow-rs": ["default", "gpu"], "pillow-rs-py": ["parallel"]})
        self.assertEqual(document["build"]["fresh_packages"], ["pillow-rs", "pillow-rs-py"])
        self.assertEqual(document["artifact"]["target"], "_core")
        self.assertEqual(document["artifact"]["crate_types"], ["cdylib"])
        self.assertEqual(document["artifact"]["sha256"], hashlib.sha256(b"synthetic cdylib bytes").hexdigest())
        self.assertEqual(output.with_name(output.name + ".log").read_bytes(), events)
        self.assertEqual(json.loads(output.read_text(encoding="utf-8")), document)

    def test_modern_path_package_ids_are_resolved_by_exact_manifest_path(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        errors = []
        events = self._cargo_events(artifact)
        features, artifacts, finished, fresh = witness._parse_cargo_output(
            events, errors=errors, root=self.root.resolve()
        )
        core_artifact = witness._find_core_cdylib(artifacts, errors, root=self.root.resolve())
        self.assertEqual(errors, [])
        self.assertEqual(finished, [True])
        self.assertEqual(features, {"pillow-rs": ["default", "gpu"], "pillow-rs-py": ["parallel"]})
        self.assertEqual(fresh, ["pillow-rs", "pillow-rs-py"])
        self.assertEqual(core_artifact["package_id"], f"path+{(self.root / 'pillow-rs-py').as_uri()}#12.2.1")

        mismatched_root = Path(self.temporary.name) / "other-workspace"
        mismatched_manifests = {}
        for package in witness.FEATURE_PACKAGES:
            manifest = mismatched_root / package / "Cargo.toml"
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text(f"[package]\nname='{package}'\n", encoding="utf-8")
            mismatched_manifests[package] = manifest
        mismatched_messages = [
            json.loads(line) for line in events.decode("utf-8").splitlines()
        ]
        for message in mismatched_messages:
            if message.get("reason") != "compiler-artifact":
                continue
            package = "pillow-rs" if message["target"]["name"] == "pillow_rs" else "pillow-rs-py"
            self.assertIn("#12.2.1", message["package_id"])
            message["manifest_path"] = str(mismatched_manifests[package])
        mismatched_events = b"".join(
            json.dumps(message).encode("utf-8") + b"\n"
            for message in mismatched_messages
        )
        mismatch_errors = []
        mismatch_features, mismatch_artifacts, mismatch_finished, mismatch_fresh = (
            witness._parse_cargo_output(
                mismatched_events, errors=mismatch_errors, root=self.root.resolve()
            )
        )
        self.assertIn("Cargo compiler-artifact did not report features for pillow-rs", mismatch_errors)
        self.assertIn("Cargo compiler-artifact did not report core artifact freshness for pillow-rs", mismatch_errors)
        self.assertIn("Cargo compiler-artifact did not report features for pillow-rs-py", mismatch_errors)
        self.assertEqual(mismatch_features["pillow-rs"], [])
        self.assertEqual(mismatch_features["pillow-rs-py"], [])
        self.assertEqual(mismatch_finished, [True])
        self.assertEqual(mismatch_fresh, [])
        self.assertIsNone(witness._find_core_cdylib(mismatch_artifacts, mismatch_errors, root=self.root.resolve()))
        self.assertIn("expected one Cargo-reported _core cdylib, found 0", mismatch_errors)

        legacy_messages = [json.loads(line) for line in events.decode("utf-8").splitlines()]
        for message in legacy_messages:
            if message.get("reason") != "compiler-artifact":
                continue
            package = "pillow-rs" if message["target"]["name"] == "pillow_rs" else "pillow-rs-py"
            message.pop("manifest_path")
            message["package_id"] = f"path+file:///legacy/{package}#{package}@12.2.1"
        legacy_events = b"".join(
            json.dumps(message).encode("utf-8") + b"\n"
            for message in legacy_messages
        )
        legacy_errors = []
        legacy_features, legacy_artifacts, legacy_finished, legacy_fresh = (
            witness._parse_cargo_output(
                legacy_events, errors=legacy_errors, root=self.root.resolve()
            )
        )
        legacy_core = witness._find_core_cdylib(
            legacy_artifacts, legacy_errors, root=self.root.resolve()
        )
        self.assertEqual(legacy_errors, [])
        self.assertEqual(legacy_finished, [True])
        self.assertEqual(legacy_features, {"pillow-rs": ["default", "gpu"], "pillow-rs-py": ["parallel"]})
        self.assertEqual(legacy_fresh, ["pillow-rs", "pillow-rs-py"])
        self.assertEqual(legacy_core["target"], "_core")

    def test_witness_preserves_missing_build_finished_as_not_proven(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        output = self.root / "incomplete.json"
        events = self._cargo_events(artifact, finished=False)
        with mock.patch.object(witness, "_capture_output", side_effect=self._mock_capture(events)):
            document = witness.run_build_witness(self.root, output, self._build_command(), {})
        self.assertEqual(document["status"], "not_proven")
        self.assertIsNone(document["artifact"])
        self.assertTrue(any("build-finished" in error for error in document["errors"]))
        self.assertEqual(output.with_name(output.name + ".log").read_bytes(), events)

    def test_witness_rejects_reused_core_or_binding_artifact(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        index = 0
        for package in ("pillow-rs", "pillow-rs-py"):
            for freshness in (True, "missing", "false"):
                output = self.root / f"freshness-{index}.json"
                index += 1
                messages = [
                    json.loads(line)
                    for line in self._cargo_events(artifact).decode("utf-8").splitlines()
                ]
                event = next(
                    message for message in messages
                    if message.get("manifest_path") == str(self.root / package / "Cargo.toml")
                )
                if freshness == "missing":
                    event.pop("fresh")
                else:
                    event["fresh"] = freshness
                events = b"".join(json.dumps(message).encode("utf-8") + b"\n" for message in messages)
                with mock.patch.object(witness, "_capture_output", side_effect=self._mock_capture(events)):
                    document = witness.run_build_witness(self.root, output, self._build_command(), {})
                with self.subTest(package=package, freshness=freshness):
                    self.assertEqual(document["status"], "not_proven")
                    self.assertNotIn(package, document["build"]["fresh_packages"])
                    if freshness is True:
                        self.assertTrue(any(f"reused the {package}" in error for error in document["errors"]))
                    else:
                        self.assertTrue(any("lacks boolean freshness" in error for error in document["errors"]))

    def test_repair_reparses_immutable_log_without_running_cargo(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        events = self._cargo_events(artifact)
        original_path, log_path, original_bytes = self._old_parser_receipt(events)
        log_before = log_path.read_bytes()
        repaired_path = self.root / "repairs" / "repaired.json"
        diagnostic_path = self.root / "repairs" / "repair.json"

        with mock.patch.object(witness, "_capture_output", side_effect=AssertionError("repair must not run commands")):
            repaired, diagnostic = witness.repair_build_witness(
                original_path, repaired_path, diagnostic_path
            )

        self.assertEqual(repaired["status"], "complete")
        self.assertEqual(repaired["build"]["fresh_packages"], ["pillow-rs", "pillow-rs-py"])
        self.assertEqual(repaired["build"]["features"], {"pillow-rs": ["default", "gpu"], "pillow-rs-py": ["parallel"]})
        self.assertEqual(repaired["artifact"]["sha256"], hashlib.sha256(b"synthetic cdylib bytes").hexdigest())
        self.assertEqual(original_path.read_bytes(), original_bytes)
        self.assertEqual(log_path.read_bytes(), log_before)
        self.assertEqual(diagnostic["method"], "reparse-retained-cargo-log-no-build")
        self.assertEqual(diagnostic["original_receipt"]["sha256"], hashlib.sha256(original_bytes).hexdigest())
        self.assertEqual(diagnostic["repaired_receipt"]["sha256"], hashlib.sha256(repaired_path.read_bytes()).hexdigest())
        self.assertTrue(diagnostic["source_check"]["matches_original_after"])
        self.assertEqual(json.loads(repaired_path.read_text(encoding="utf-8")), repaired)
        self.assertEqual(json.loads(diagnostic_path.read_text(encoding="utf-8")), diagnostic)

    def test_repair_rejects_changed_retained_log(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        original_path, log_path, original_bytes = self._old_parser_receipt(self._cargo_events(artifact))
        log_path.write_bytes(log_path.read_bytes() + b"changed\n")
        changed_log = log_path.read_bytes()
        repaired_path = self.root / "repairs" / "repaired.json"
        diagnostic_path = self.root / "repairs" / "repair.json"
        with self.assertRaisesRegex(ValueError, "retained Cargo log differs"):
            witness.repair_build_witness(original_path, repaired_path, diagnostic_path)
        self.assertEqual(original_path.read_bytes(), original_bytes)
        self.assertEqual(log_path.read_bytes(), changed_log)
        self.assertFalse(repaired_path.exists())
        self.assertFalse(diagnostic_path.exists())

    def test_repair_rejects_changed_source_closure(self):
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"synthetic cdylib bytes")
        original_path, log_path, original_bytes = self._old_parser_receipt(self._cargo_events(artifact))
        self._write("pillow-rs/src/lib.rs", "pub fn changed_after_build() {}\n")
        repaired_path = self.root / "repairs" / "repaired.json"
        diagnostic_path = self.root / "repairs" / "repair.json"
        with self.assertRaisesRegex(ValueError, "current source closure differs"):
            witness.repair_build_witness(original_path, repaired_path, diagnostic_path)
        self.assertEqual(original_path.read_bytes(), original_bytes)
        self.assertTrue(log_path.is_file())
        self.assertFalse(repaired_path.exists())
        self.assertFalse(diagnostic_path.exists())

    def test_command_rejects_unbound_manifest_without_running_build(self):
        output = self.root / "invalid.json"
        outside_manifest = self.root / "pillow-rs" / "Cargo.toml"
        events = self._cargo_events(self.root / "unused.so")
        capture = self._mock_capture(events)
        observed = []

        def track(command, *, cwd, environment):
            observed.append(list(command))
            return capture(command, cwd=cwd, environment=environment)

        with mock.patch.object(witness, "_capture_output", side_effect=track):
            document = witness.run_build_witness(self.root, output, self._build_command(outside_manifest), {})
        self.assertEqual(document["status"], "not_proven")
        self.assertTrue(any("manifest" in error for error in document["errors"]))
        self.assertFalse(any(command[1:2] == ["build"] for command in observed))
        self.assertIsNone(document["build"]["exit_code"])


if __name__ == "__main__":
    unittest.main()

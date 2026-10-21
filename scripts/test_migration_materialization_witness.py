from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

try:
    import migration_build_witness as build_witness
    import migration_materialization_witness as materialization
except ModuleNotFoundError:
    from scripts import migration_build_witness as build_witness
    from scripts import migration_materialization_witness as materialization


class MigrationMaterializationWitnessTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / "workspace"
        self.root.mkdir()
        (self.root / "Cargo.toml").write_text(
            "[workspace]\nmembers=['pillow-rs','pillow-rs-py','pillow-rs-js']\n",
            encoding="utf-8",
        )
        (self.root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        for package in ("pillow-rs", "pillow-rs-py", "pillow-rs-js"):
            (self.root / package / "src").mkdir(parents=True)
            (self.root / package / "Cargo.toml").write_text(
                f"[package]\nname='{package}'\n", encoding="utf-8"
            )
            (self.root / package / "src" / "lib.rs").write_text(
                "pub fn fixture() {}\n", encoding="utf-8"
            )
        (self.root / "pillow-rs-py" / "pyproject.toml").write_text(
            "[tool.maturin]\nfeatures=['pyo3/extension-module']\n",
            encoding="utf-8",
        )

    def _cargo_events(self, artifact: Path) -> bytes:
        core_manifest = self.root / "pillow-rs" / "Cargo.toml"
        binding_manifest = self.root / "pillow-rs-py" / "Cargo.toml"
        rows = [
            {
                "reason": "compiler-artifact",
                "package_id": f"path+{core_manifest.parent.as_uri()}#12.2.1",
                "manifest_path": str(core_manifest),
                "target": {"name": "pillow_rs", "crate_types": ["lib"]},
                "features": ["default", "gpu"],
                "fresh": False,
                "filenames": [],
            },
            {
                "reason": "compiler-artifact",
                "package_id": f"path+{binding_manifest.parent.as_uri()}#12.2.1",
                "manifest_path": str(binding_manifest),
                "target": {"name": "_core", "crate_types": ["cdylib"]},
                "features": ["default"],
                "fresh": False,
                "filenames": [str(artifact)],
            },
            {"reason": "build-finished", "success": True},
        ]
        return b"".join(json.dumps(row).encode() + b"\n" for row in rows)

    def _raw_build_witness(self) -> tuple[Path, Path]:
        artifact = self.root / "target" / "release" / "lib_core.so"
        artifact.parent.mkdir(parents=True)
        artifact.write_bytes(b"raw cargo cdylib")
        output = self.root / "evidence" / "cargo-witness.json"
        events = self._cargo_events(artifact)
        with (
            mock.patch.object(build_witness, "_git_workspace_state", return_value=("a" * 40, True)),
            mock.patch.object(
                build_witness,
                "_tool_version",
                side_effect=["cargo 1.96.1", "rustc 1.96.1\nhost: x86_64-unknown-linux-gnu"],
            ),
            mock.patch.object(
                build_witness, "_capture_output", return_value=(0, events, None)
            ),
        ):
            document = build_witness.run_build_witness(
                self.root,
                output,
                [
                    "cargo", "build", "--manifest-path", "pillow-rs-py/Cargo.toml",
                    "--release", "--locked", "--message-format=json",
                    "--features", "pyo3/extension-module",
                ],
                {"PYO3_PYTHON": sys.executable},
            )
        self.assertEqual(document["status"], "complete")
        return output, artifact

    def _materialized_witness(
        self, *, exit_code: int = 0, pip_path: Path | None = None
    ) -> dict:
        raw_path, cargo_artifact = self._raw_build_witness()
        module = self.root / "pillow-rs-py" / "python" / "pillow_rs" / "_core.abi3.so"
        module.parent.mkdir(parents=True)
        module.write_bytes(b"Maturin materialized cdylib")
        log = self.root / "evidence" / "maturin.log"
        log.write_text("Maturin build completed\n", encoding="utf-8")
        command = [
            sys.executable, "-m", "maturin", "develop", "--skip-install",
            "--manifest-path", "pillow-rs-py/Cargo.toml", "--release", "--locked",
        ]
        if pip_path is not None:
            command.extend(["--pip-path", str(pip_path.resolve())])
        with mock.patch.object(
            materialization.subprocess,
            "run",
            return_value=subprocess.CompletedProcess(
                ["python", "-m", "maturin", "--version"], 0, "maturin 1.14.1\n", ""
            ),
        ):
            return materialization.build_materialization_witness(
                root=self.root,
                cargo_witness_path=raw_path,
                output_path=self.root / "evidence" / "materialized-witness.json",
                maturin_log_path=log,
                module_path=module,
                maturin_python=Path(sys.executable),
                maturin_pip_path=pip_path,
                maturin_argv=command,
                maturin_exit_code=exit_code,
                parallel=False,
            )

    def test_witness_binds_raw_cargo_and_final_maturin_artifacts_separately(self):
        witness = self._materialized_witness()
        self.assertEqual(witness["status"], "complete")
        self.assertEqual(witness["schema"], "pillow-rs/build-witness@2")
        materialized = witness["materialization"]
        self.assertEqual(materialized["status"], "complete")
        self.assertEqual(
            materialized["raw_cargo_artifact_sha256"],
            witness["artifact"]["sha256"],
        )
        self.assertNotEqual(
            materialized["module_artifact"]["sha256"],
            witness["artifact"]["sha256"],
        )
        self.assertEqual(
            materialized["cargo_artifact_after"]["path"],
            witness["artifact"]["path"],
        )
        self.assertEqual(
            materialized["module_artifact"]["sha256"],
            hashlib.sha256(b"Maturin materialized cdylib").hexdigest(),
        )

    def test_witness_binds_an_explicit_pip_path_for_pipless_venvs(self):
        pip_path = self.root.parent / "tools" / "pip"
        pip_path.parent.mkdir()
        pip_path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        pip_path.chmod(0o755)

        witness = self._materialized_witness(pip_path=pip_path)

        self.assertEqual(witness["status"], "complete")
        self.assertEqual(
            witness["materialization"]["argv"][-2:],
            ["--pip-path", str(pip_path.resolve())],
        )

    def test_witness_rejects_a_missing_explicit_pip_path(self):
        pip_path = self.root.parent / "tools" / "missing-pip"
        with self.assertRaisesRegex(ValueError, "selected Maturin pip executable"):
            self._materialized_witness(pip_path=pip_path)

    def test_witness_accepts_existing_standalone_maturin_executable(self):
        raw_path, _cargo_artifact = self._raw_build_witness()
        module = self.root / "pillow-rs-py" / "python" / "pillow_rs" / "_core.abi3.so"
        module.parent.mkdir(parents=True)
        module.write_bytes(b"Maturin materialized cdylib")
        log = self.root / "evidence" / "maturin.log"
        log.write_text("Maturin build completed\n", encoding="utf-8")
        executable = self.root.parent / "tools" / "maturin"
        executable.parent.mkdir()
        executable.write_bytes(b"standalone Maturin fixture")
        executable.chmod(0o755)
        argv = [
            str(executable.resolve()), "develop", "--skip-install", "--manifest-path",
            "pillow-rs-py/Cargo.toml", "--release", "--locked",
        ]
        with mock.patch.object(
            materialization.subprocess,
            "run",
            return_value=subprocess.CompletedProcess(
                [str(executable.resolve()), "--version"], 0, "maturin 1.14.1\n", ""
            ),
        ) as run:
            witness = materialization.build_materialization_witness(
                root=self.root,
                cargo_witness_path=raw_path,
                output_path=self.root / "evidence" / "materialized-witness.json",
                maturin_log_path=log,
                module_path=module,
                maturin_python=Path(sys.executable),
                maturin_executable=executable,
                maturin_argv=argv,
                maturin_exit_code=0,
                parallel=False,
            )

        self.assertEqual(witness["status"], "complete")
        self.assertEqual(witness["materialization"]["version"], "maturin 1.14.1")
        self.assertEqual(witness["materialization"]["argv"], argv)
        run.assert_called_once()
        self.assertEqual(
            run.call_args.args[0], [str(executable.resolve()), "--version"]
        )

    def test_witness_rejects_source_drift_after_cargo_build(self):
        raw_path, _artifact = self._raw_build_witness()
        pyproject = self.root / "pillow-rs-py" / "pyproject.toml"
        pyproject.write_text("[tool.maturin]\nfeatures=[]\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "source changed between"):
            self._capture_materialization(raw_path)

    def test_nonzero_maturin_exit_is_not_proven(self):
        witness = self._materialized_witness(exit_code=1)
        self.assertEqual(witness["status"], "not_proven")
        self.assertEqual(witness["materialization"]["status"], "not_proven")
        self.assertTrue(any("Maturin exited" in error for error in witness["errors"]))

    def _capture_materialization(self, raw_path: Path) -> dict:
        module = self.root / "pillow-rs-py" / "python" / "pillow_rs" / "_core.abi3.so"
        module.parent.mkdir(parents=True, exist_ok=True)
        module.write_bytes(b"Maturin materialized cdylib")
        log = self.root / "evidence" / "maturin.log"
        log.write_text("Maturin build completed\n", encoding="utf-8")
        with mock.patch.object(
            materialization.subprocess,
            "run",
            return_value=subprocess.CompletedProcess(
                ["python", "-m", "maturin", "--version"], 0, "maturin 1.14.1\n", ""
            ),
        ):
            return materialization.build_materialization_witness(
                root=self.root,
                cargo_witness_path=raw_path,
                output_path=self.root / "evidence" / "materialized-witness.json",
                maturin_log_path=log,
                module_path=module,
                maturin_python=Path(sys.executable),
                maturin_argv=[
                    sys.executable, "-m", "maturin", "develop", "--skip-install",
                    "--manifest-path", "pillow-rs-py/Cargo.toml", "--release", "--locked",
                ],
                maturin_exit_code=0,
                parallel=False,
            )


if __name__ == "__main__":
    unittest.main()

"""Regression tests for release configuration and immutable publication."""
from __future__ import annotations
import copy
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile
import yaml
from check_release_recovery import REQUIRED_JOBS, validate
from prepare_pypi_release import missing_files
from check_release_licenses import verify_archive_license


class LicenseArchiveTests(unittest.TestCase):
    def test_archive_requires_complete_license_text(self) -> None:
        expected = b"Copyright notice\nPermission terms\nDisclaimer\n"
        with tempfile.TemporaryDirectory() as directory:
            for suffix in (".whl", ".crate", ".tar.gz", ".tgz"):
                for name, content in (("README.md", expected),
                                      ("LICENSE", b"MIT-CMU"),
                                      ("LICENSE", expected)):
                    with self.subTest(format=suffix, name=name, content=content):
                        archive = Path(directory) / ("package" + suffix)
                        if suffix == ".whl":
                            with zipfile.ZipFile(archive, "w") as package:
                                package.writestr("package.dist-info/licenses/" + name, content)
                        else:
                            with tarfile.open(archive, "w:gz") as package:
                                member = tarfile.TarInfo("package/" + name)
                                member.size = len(content)
                                package.addfile(member, io.BytesIO(content))
                        if name == "LICENSE" and content == expected:
                            verify_archive_license(archive, expected)
                        else:
                            with self.assertRaises(ValueError):
                                verify_archive_license(archive, expected)


class WorkflowInputTests(unittest.TestCase):
    def test_setup_node_cache_selects_a_supported_package_manager(self) -> None:
        # setup-node v5's cache input selects npm/yarn/pnpm. Its separate
        # package-manager-cache input disables automatic detection. A boolean
        # cache value becomes the unsupported manager "false" on the runner.
        # Contract: actions/setup-node@a0853c2/action.yml and cache-restore.ts.
        workflows = Path(__file__).resolve().parent.parent / ".github" / "workflows"
        checked = 0
        for path in sorted(workflows.glob("*.yml")):
            document = yaml.safe_load(path.read_text())
            for job_name, job in document.get("jobs", {}).items():
                for step in job.get("steps", []):
                    if step.get("uses", "").split("@", 1)[0] != "actions/setup-node":
                        continue
                    checked += 1
                    cache = step.get("with", {}).get("cache", "")
                    with self.subTest(workflow=path.name, job=job_name):
                        self.assertIsInstance(cache, str)
                        self.assertIn(cache.strip(), ("", "npm", "yarn", "pnpm"))
        self.assertGreater(checked, 0, "no setup-node actions were checked")


class RecoveryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.run = dict(id=71, name="Release", path=".github/workflows/release.yml",
                        event="push", head_branch="v0.1.2", head_sha="a" * 40,
                        status="completed", run_attempt=2, conclusion="failure")
        self.jobs = [dict(name=name, run_id=71, head_sha="a" * 40,
                          run_attempt=2, status="completed", conclusion="success")
                     for name in sorted(REQUIRED_JOBS)]

    def check(self, jobs: list[dict]) -> None:
        validate(self.run, jobs, tag="v0.1.2", commit="a" * 40)

    def test_final_job_failure_can_be_recovered_after_all_registries_succeed(self) -> None:
        self.check(self.jobs)

    def test_every_failed_skipped_or_missing_publish_gate_blocks_recovery(self) -> None:
        for index in range(len(self.jobs)):
            for result in ["failure", "skipped", "cancelled", None]:
                jobs = copy.deepcopy(self.jobs)
                jobs[index]["conclusion"] = result
                with self.assertRaises(ValueError):
                    self.check(jobs)
            with self.assertRaises(ValueError):
                self.check(self.jobs[:index] + self.jobs[index + 1:])

    def test_unrelated_runs_attempts_or_commits_cannot_supply_evidence(self) -> None:
        for field, value in [("run_id", 72), ("run_attempt", 1), ("head_sha", "b" * 40)]:
            jobs = copy.deepcopy(self.jobs)
            jobs[0][field] = value
            with self.assertRaises(ValueError):
                self.check(jobs)
        self.run["event"] = "pull_request"
        with self.assertRaises(ValueError):
            self.check(self.jobs)


class PyPIPublicationTests(unittest.TestCase):
    def test_partial_publication_stages_only_missing_identical_version_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.whl"
            second = Path(directory) / "second.whl"
            first.write_bytes(b"verified first wheel")
            second.write_bytes(b"verified second wheel")
            metadata = {"urls": [{"filename": first.name, "digests": {
                "sha256": hashlib.sha256(first.read_bytes()).hexdigest()}}]}
            self.assertEqual(missing_files([first, second], None), [first, second])
            self.assertEqual(missing_files([first, second], metadata), [second])
            self.assertEqual(missing_files([first], metadata), [])
            first.write_bytes(b"rebuilt with different bytes")
            with self.assertRaises(ValueError):
                missing_files([first, second], metadata)

    def test_empty_or_missing_package_never_counts_as_published(self) -> None:
        with self.assertRaises(ValueError):
            missing_files([], None)
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                missing_files([Path(directory) / "missing.whl"], None)


if __name__ == "__main__":
    unittest.main()

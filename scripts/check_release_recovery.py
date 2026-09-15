#!/usr/bin/env python3
"""Require successful registry jobs before recovering GitHub release assets."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

REQUIRED_JOBS = {
    "Release preflight", "Package release artifacts",
    "Publish pillow-rs to crates.io", "Publish pillow-rs to PyPI",
    "Publish pillow-rs to npm",
}


def validate(run: dict, jobs: list[dict], *, tag: str, commit: str) -> None:
    if (
        run.get("name") != "Release"
        or run.get("path", "").split("@")[0] != ".github/workflows/release.yml"
        or run.get("event") != "push"
        or run.get("head_branch") != tag
        or run.get("head_sha") != commit
        or run.get("status") != "completed"
    ):
        raise ValueError("recovery requires the completed release workflow on the exact tag commit")
    for name in sorted(REQUIRED_JOBS):
        matches = [job for job in jobs if job.get("name") == name]
        if len(matches) != 1:
            raise ValueError(f"expected one required release job: {name}")
        job = matches[0]
        if (
            job.get("run_id") != run.get("id")
            or job.get("head_sha") != commit
            or job.get("run_attempt") != run.get("run_attempt")
            or job.get("status") != "completed"
            or job.get("conclusion") != "success"
        ):
            raise ValueError(f"required publish/verification job did not succeed: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--jobs", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    try:
        validate(json.loads(args.run.read_text()), json.loads(args.jobs.read_text())["jobs"], tag=args.tag, commit=args.commit)
    except (KeyError, OSError, ValueError) as error:
        raise SystemExit(f"Release asset recovery is not eligible: {error}") from error
    print("all required registry and verification jobs succeeded on the exact release attempt")


if __name__ == "__main__":
    main()

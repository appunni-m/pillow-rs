#!/usr/bin/env python3
"""Select public benchmark data without executing code from an artifact."""
from __future__ import annotations

import json
import os
from pathlib import Path
import re
import urllib.request


ROOT = Path(__file__).resolve().parent.parent


def read_json(url: str) -> dict:
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "public-docs-benchmark"}
    token = os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as response:
        return json.load(response)


def trusted_run(run: dict, repository: str) -> bool:
    return (
        run.get("path") == ".github/workflows/benchmark.yml"
        and run.get("event") in {"push", "schedule", "workflow_dispatch"}
        and run.get("status") == "completed"
        and run.get("conclusion") == "success"
        and run.get("head_branch") == "main"
        and run.get("head_repository", {}).get("full_name") == repository
        and isinstance(run.get("id"), int)
        and run["id"] > 0
        and re.fullmatch(r"[0-9a-f]{40}", run.get("head_sha", "")) is not None
    )


def select_run(repository: str, event_name: str, event: dict, fetch=read_json) -> dict | None:
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repository):
        raise ValueError("Expected owner/repository")
    if event_name in {"pull_request", "pull_request_target"}:
        return None
    base = f"https://api.github.com/repos/{repository}/actions"
    release_event = event_name == "workflow_run" and event.get("workflow_run", {}).get("path") == ".github/workflows/release.yml"
    if release_event:
        run = event["workflow_run"]
        if not (run.get("name") == "Release" and run.get("conclusion") == "success"
                and run.get("status") == "completed" and run.get("event") in {"push", "workflow_dispatch"}
                and re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", run.get("head_branch", ""))
                and run.get("head_repository", {}).get("full_name") == repository):
            raise ValueError("Release event is not a successful tag run from this repository")
    explicit = event_name == "workflow_run" and not release_event
    if explicit:
        runs = [event["workflow_run"]]
        if not trusted_run(runs[0], repository):
            raise ValueError("Benchmark event is not a successful run from this repository's main branch")
    else:
        # Keep routine docs updates from replacing a hosted measurement with
        # the committed fallback. Only the newest successful main run is used;
        # expired/missing data falls back visibly, not to an arbitrary older run.
        runs = fetch(f"{base}/workflows/benchmark.yml/runs?branch=main&status=success&per_page=1")["workflow_runs"]
        if runs and not trusted_run(runs[0], repository):
            raise ValueError("Latest benchmark run does not match the trusted main workflow")
    if not runs:
        return None
    run = runs[0]
    payload = fetch(f"{base}/runs/{run['id']}/artifacts?per_page=100")
    artifacts = [artifact for artifact in payload["artifacts"]
                 if artifact.get("name") == "public-benchmark" and not artifact.get("expired", True)]
    if len(artifacts) > 1:
        raise ValueError("Benchmark run contains duplicate public data artifacts")
    if not artifacts:
        if explicit:
            raise ValueError("Completed benchmark event has no available public data artifact")
        return None
    return {"run_id": str(run["id"]), "head_sha": run["head_sha"]}


def main() -> None:
    config = json.loads((ROOT / "documentation.json").read_text())
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    event = json.loads(Path(event_path).read_text()) if event_path else {}
    selected = select_run(config["repository"], os.environ.get("GITHUB_EVENT_NAME", ""), event)
    if selected:
        message = f"Using benchmark run {selected['run_id']} at {selected['head_sha']}"
    else:
        message = "Using the committed benchmark snapshot: no eligible hosted artifact for this event"
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path and selected:
        with open(output_path, "a", encoding="utf-8") as output:
            for key, value in selected.items():
                output.write(f"{key}={value}\n")
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as summary:
            summary.write(f"{message}\n")
    print(message)


if __name__ == "__main__":
    main()

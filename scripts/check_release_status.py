#!/usr/bin/env python3
"""Read public GitHub release/CI conclusions and failure annotations without credentials."""
from __future__ import annotations

import argparse
from html.parser import HTMLParser
import json
from pathlib import PurePosixPath
import re
import urllib.error
import urllib.parse
import urllib.request


class PageText(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.parts: list[str] = []
        self.hidden = 0

    def handle_starttag(self, tag: str, attrs: list) -> None:
        if tag in {"script", "style"}:
            self.hidden += 1

    def handle_endtag(self, tag: str) -> None:
        if tag in {"script", "style"}:
            self.hidden = max(0, self.hidden - 1)

    def handle_data(self, data: str) -> None:
        if not self.hidden and data.strip():
            self.parts.append(data.strip())


def read_url(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "pillow-rs-release-status"})
    with urllib.request.urlopen(request, timeout=40) as response:
        return response.read()


def public_page(url: str) -> dict[str, str]:
    parser = PageText()
    parser.feed(read_url(url).decode("utf-8"))
    page = "\n".join(parser.parts)
    start = page.find("Triggered via")
    end = page.find("Oh hello!", start)
    annotations = page.find("Annotations")
    artifacts = page.find("Artifacts\nProduced", annotations)
    return {
        "url": url,
        "github_page_summary": page[start:end] if start >= 0 and end > start else "unavailable",
        "annotations": page[annotations:artifacts if artifacts >= 0 else None]
        if annotations >= 0 else "none visible",
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--commit", help="exact 40-character commit SHA")
    selection.add_argument("--run-url", help="public GitHub Actions run URL; also works without API quota")
    parser.add_argument("--repo", default="appunni-m/pillow-rs", help="GitHub owner/repository")
    args = parser.parse_args()
    try:
        if args.run_url:
            if not re.fullmatch(r"https://github\.com/[\w.-]+/[\w.-]+/actions/runs/[0-9]+", args.run_url):
                parser.error("expected a GitHub Actions run URL without a query or fragment")
            print(json.dumps(public_page(args.run_url), indent=2))
            return
        if not re.fullmatch(r"[\w.-]+/[\w.-]+", args.repo):
            parser.error("expected owner/repository")
        if not re.fullmatch(r"[0-9a-fA-F]{40}", args.commit):
            parser.error("expected a full commit SHA")
        base = f"https://api.github.com/repos/{args.repo}/actions"
        query = urllib.parse.urlencode({"head_sha": args.commit, "per_page": 100})
        payload = json.loads(read_url(f"{base}/runs?{query}"))
        result = []
        for run in payload["workflow_runs"]:
            if run["head_sha"] != args.commit or PurePosixPath(run["path"]).name not in {"ci.yml", "release.yml"}:
                continue
            row = {key: run.get(key) for key in (
                "id", "name", "event", "head_branch", "head_sha", "status", "conclusion", "html_url",
            )}
            jobs = json.loads(read_url(f"{base}/runs/{run['id']}/jobs?per_page=100"))
            row["jobs"] = []
            for job in jobs["jobs"]:
                entry = {key: job.get(key) for key in ("id", "name", "status", "conclusion", "html_url")}
                entry["active_or_failed_steps"] = [
                    {key: step.get(key) for key in ("name", "status", "conclusion")}
                    for step in job.get("steps", [])
                    if step.get("status") == "in_progress"
                    or step.get("conclusion") in {"failure", "timed_out", "action_required"}
                ]
                row["jobs"].append(entry)
            if run["conclusion"] in {"failure", "timed_out", "action_required"} or any(
                job.get("conclusion") in {"failure", "timed_out", "action_required"}
                for job in jobs["jobs"]
            ):
                url = f"https://github.com/{args.repo}/actions/runs/{int(run['id'])}"
                row["annotations"] = public_page(url)["annotations"]
            result.append(row)
        print(json.dumps({"repository": args.repo, "commit": args.commit, "runs": result}, indent=2))
    except (urllib.error.URLError, KeyError, ValueError) as error:
        raise SystemExit(f"GitHub status unavailable: {error}. Use --run-url for a known public run when API quota is exhausted.") from error


if __name__ == "__main__":
    main()

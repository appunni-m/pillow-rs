#!/usr/bin/env python3
"""Keep reader-facing release references current without rewriting historical evidence."""
from __future__ import annotations

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
VERSION = r"[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha|beta|rc)\.[0-9]+)?"
BLOCK = re.compile(r"<!-- release:(summary|cargo|npm|python|sdk|rust-api) -->\n.*?<!-- /release:\1 -->", re.S)
AUDIENCES = {"user", "contributor", "reference", "project", "historical"}
CONTRIBUTOR_COMMAND = re.compile(
    r"(?:^|\n|`)\s*(?:\$\s*)?(?:make\s+\S+|cargo\s+(?:test|bench|clippy|run --example)\b"
    r"|npm\s+(?:test|ci|run\s+(?:test|build|bench))\b|git\s+clone\b"
    r"|python[0-9.]*\s+(?:scripts/|[^\n]*-m\s+(?:pytest|unittest)))"
)


def published(config: dict) -> dict:
    release = config["published_release"]
    if not re.fullmatch(VERSION, release["version"]):
        raise ValueError("invalid published release version")
    if not re.fullmatch(r"[0-9a-f]{40}", release["revision"]):
        raise ValueError("published release needs the exact tag commit")
    datetime.fromisoformat(release["published_at"].replace("Z", "+00:00"))
    if release["revision"] != config["release_revision"]:
        raise ValueError("benchmark release identity differs from the published release")
    return release


def check_source_version(config: dict, version: str) -> None:
    """The source candidate and public installation version have separate roles."""
    if config["version"] != version:
        raise ValueError("documentation source version differs from the package version")
    published(config)


def release_block(kind: str, config: dict) -> str:
    release = published(config)
    version, name = release["version"], config["project"]
    url = f"https://github.com/{config['repository']}/releases"
    if kind == "summary":
        content = f"**Latest release: [{version}]({url}/tag/v{version}).**"
    elif kind == "rust-api":
        module = name.replace("-", "_")
        content = f"[Rust API reference](https://docs.rs/{name}/{version}/{module}/)."
    elif kind == "cargo":
        content = f'```toml\n[dependencies]\n{name} = "={version}"\n```'
    elif kind == "npm":
        if "npm" not in config["registries"]:
            raise ValueError("no npm distribution for this project")
        content = f"```sh\nnpm install {name}@{version}\n```"
    elif kind == "python":
        if "pypi" not in config["registries"]:
            raise ValueError("no Python distribution for this project")
        content = f"```sh\npython3 -m venv .venv\n.venv/bin/python -m pip install {name}=={version}\n```"
    elif kind == "sdk":
        asset = f"fontdone-c-abi-{version}-aarch64-apple-darwin.tar.gz"
        content = f"[Download the macOS ARM64 C SDK]({url}/download/v{version}/{asset})."
    else:
        raise ValueError(f"unknown release block: {kind}")
    return f"<!-- release:{kind} -->\n{content}\n<!-- /release:{kind} -->"


def render_blocks(text: str, config: dict) -> str:
    # Every marker must belong to a complete, known pair; a typo must not
    # silently turn a generated installation command into unchecked prose.
    remainder = BLOCK.sub("", text)
    if re.search(r"<!--\s*/?release:", remainder):
        raise ValueError("malformed release block")
    return BLOCK.sub(lambda match: release_block(match[1], config), text)


def check_reader_page(text: str, source: str, config: dict) -> list[str]:
    errors = []
    expected = published(config)["version"]
    if render_blocks(text, config) != text:
        errors.append(f"{source}: stale release block; run make docs-release-refresh")
    if "<!-- release:summary -->" not in text:
        errors.append(f"{source}: missing current release reference")
    if CONTRIBUTOR_COMMAND.search(text) or re.search(r"(?:tests/fixtures/|target/coverage/|build/migration-parity/)", text):
        errors.append(f"{source}: move repository build/test instructions to Contribute")
    if re.search(r"\b(?:unreleased|current candidate|until publication|after this candidate is published)\b", text, re.I):
        errors.append(f"{source}: published user guide describes an unreleased candidate")
    name = re.escape(config["project"])
    patterns = [rf"{name}(?:==|@=?)({VERSION})",
                rf'{name}\s*=\s*"=?({VERSION})"',
                rf'{name}\s*=\s*\{{[^}}\n]*version\s*=\s*"=?({VERSION})"',
                rf"https://docs\.rs/{name}/({VERSION})(?:/|\))"]
    for pattern in patterns:
        for version in re.findall(pattern, text):
            if version != expected:
                errors.append(f"{source}: install/API reference uses {version}; published version is {expected}")
    return errors


def check_documents(root: Path, config: dict) -> None:
    published(config)
    errors = []
    for page in config["pages"]:
        if page.get("audience") not in AUDIENCES:
            errors.append(f"{page['source']}: assign an explicit documentation audience")
        if page.get("audience") in AUDIENCES:
            path = (root / page["source"]).resolve()
            if not path.is_relative_to(root.resolve()):
                raise ValueError("documentation source leaves the repository")
            text = path.read_text()
            if render_blocks(text, config) != text:
                errors.append(f"{page['source']}: stale release block; run make docs-release-refresh")
            if page.get("audience") == "user":
                errors.extend(check_reader_page(text, page["source"], config))
    readme = next((p for p in config["pages"] if p["source"] == "README.md"), {})
    if readme.get("audience") != "user":
        errors.append("README must remain a user page")
    if errors:
        raise ValueError("\n".join(errors))


def get_json(url: str) -> dict | list:
    headers = {"User-Agent": "public-documentation-release-check"}
    # Never send the GitHub credential to a package registry or payload URL.
    if url.startswith("https://api.github.com/") and os.environ.get("GH_TOKEN"):
        headers["Authorization"] = f"Bearer {os.environ['GH_TOKEN']}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as response:
        return json.load(response)


def newest_release(releases: list[dict]) -> dict:
    candidates = [r for r in releases if not r.get("draft") and r.get("published_at")]
    if not candidates:
        raise ValueError("no published GitHub release")
    # GitHub /releases/latest excludes prereleases. These projects ship alphas.
    return max(candidates, key=lambda r: r["published_at"])


def remote_release(config: dict, fetch=get_json) -> dict:
    repo = config["repository"]
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        raise ValueError("invalid repository identity")
    base = f"https://api.github.com/repos/{repo}"
    releases = []
    for page in range(1, 11):
        batch = fetch(f"{base}/releases?per_page=100&page={page}")
        releases.extend(batch)
        if len(batch) < 100:
            break
    else:
        raise ValueError("release history exceeds the bounded lookup")
    release = newest_release(releases)
    tag = release["tag_name"]
    if not re.fullmatch("v" + VERSION, tag):
        raise ValueError(f"unsupported published release tag: {tag}")
    obj = fetch(f"{base}/git/ref/tags/{tag}")["object"]
    for _ in range(5):
        if obj["type"] == "commit":
            break
        if obj["type"] != "tag":
            raise ValueError("release tag does not reference a commit")
        obj = fetch(f"{base}/git/tags/{obj['sha']}")["object"]
    else:
        raise ValueError("release tag nesting exceeds the bounded lookup")
    version = tag[1:]
    python_version = re.sub(r"-(alpha|beta|rc)\.(\d+)$", lambda m: {"alpha": "a", "beta": "b", "rc": "rc"}[m[1]] + m[2], version)
    name = config["project"]
    for registry in config["registries"]:
        if registry == "cargo":
            actual = fetch(f"https://crates.io/api/v1/crates/{name}/{version}")["version"]["num"]
        elif registry == "npm":
            actual = fetch(f"https://registry.npmjs.org/{name}/{version}")["version"]
        elif registry == "pypi":
            actual = fetch(f"https://pypi.org/pypi/{name}/{python_version}/json")["info"]["version"]
        else:
            raise ValueError(f"unknown registry: {registry}")
        if actual != (python_version if registry == "pypi" else version):
            raise ValueError(f"{registry} release version mismatch")
    return {"version": version, "revision": obj["sha"], "published_at": release["published_at"]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--online", action="store_true", help="verify the latest published GitHub and registry versions")
    parser.add_argument("--refresh", action="store_true", help="refresh the release record and marked blocks from registries")
    args = parser.parse_args()
    path = ROOT / "documentation.json"
    config = json.loads(path.read_text())
    if args.online or args.refresh:
        remote = remote_release(config)
        if args.refresh:
            config["published_release"] = remote
            config["release_revision"] = remote["revision"]
            path.write_text(json.dumps(config, indent=2) + "\n")
            for page in config["pages"]:
                if page.get("audience") in AUDIENCES:
                    source = (ROOT / page["source"]).resolve()
                    if not source.is_relative_to(ROOT):
                        raise ValueError("documentation source leaves the repository")
                    source.write_text(render_blocks(source.read_text(), config))
        elif remote != published(config):
            raise ValueError(f"documentation release is stale; latest is {remote['version']}. Run make docs-release-refresh")
    check_documents(ROOT, config)
    print(f"Release documentation checked: {published(config)['version']}; reader/contributor separation checked")


if __name__ == "__main__":
    main()

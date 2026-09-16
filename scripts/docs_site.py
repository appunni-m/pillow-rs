#!/usr/bin/env python3
"""Build the public documentation surface from reviewed repository sources.

This tool does not run benchmarks, reinterpret failed comparisons, publish a
site, or modify fixture inputs. MkDocs owns Markdown rendering and search.
"""
from __future__ import annotations

import argparse
from collections import Counter
from fnmatch import fnmatchcase
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
LINK = re.compile(r"(?<=\]\()([^\s)]+)(?=\))")
FENCE = re.compile(r"^(`{3,}|~{3,}).*?^\1\s*$", re.MULTILINE | re.DOTALL)
MAKE_TARGET = re.compile(r"^([a-zA-Z0-9_. -]+?)\s*:", re.MULTILINE)


def read_config(root: Path) -> dict:
    config = json.loads((root / "documentation.json").read_text())
    snapshot = os.environ.get("DOCS_BENCHMARK_SNAPSHOT")
    if snapshot:
        from docs_evidence import validate
        path = source_path(root, snapshot)
        data = json.loads(path.read_text())
        validate(data, config["repository"])
        revision = os.environ.get("DOCS_BENCHMARK_REVISION")
        if not revision or data["revision"] != revision:
            raise ValueError("downloaded benchmark does not match the trusted workflow source revision")
        config["benchmark"]["source"] = snapshot
    return config


def source_path(root: Path, relative: str) -> Path:
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"documentation path leaves the repository: {relative}")
    return path


def anchors(text: str) -> set[str]:
    text = FENCE.sub("", text)
    result = set(re.findall(r'<(?:a|h[1-6])\s+[^>]*id="([^"]+)"', text))
    counts: Counter[str] = Counter()
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", text, re.MULTILINE):
        slug = re.sub(r"[^\w\s-]", "", heading.lower()).strip().replace(" ", "-")
        slug = re.sub(r"-+", "-", slug)
        result.add(slug if not counts[slug] else f"{slug}-{counts[slug]}")
        counts[slug] += 1
    return result


def tracked_markdown(root: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "*.md"],
        cwd=root, capture_output=True, check=True,
    )
    return sorted({root / name.decode() for name in result.stdout.split(b"\0")
                   if name and (root / name.decode()).is_file()})


def check_sources(root: Path, config: dict) -> None:
    from docs_release import check_documents
    check_documents(root, config)
    errors = []
    pages = config["pages"]
    if len({p["output"] for p in pages}) != len(pages):
        errors.append("duplicate published page paths")
    targets = set()
    for makefile in (root / "Makefile", root / "docs.mk"):
        for names in MAKE_TARGET.findall(makefile.read_text()):
            targets.update(names.split())
    for page in pages:
        path = source_path(root, page["source"])
        if not path.is_file():
            errors.append(f"missing public source: {page['source']}")
            continue
        text = path.read_text()
        for destination in LINK.findall(text):
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc:
                continue
            target = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path
            if not target.is_relative_to(root.resolve()) or not target.exists():
                errors.append(f"{page['source']}: broken local link {destination}")
            elif parsed.fragment and target.suffix == ".md":
                if unquote(parsed.fragment) not in anchors(target.read_text()):
                    errors.append(f"{page['source']}: missing anchor {destination}")
        for target in re.findall(r"(?:`|\n)\s*(?:[A-Z][A-Z_0-9]*=\S+\s+)*make\s+([a-z][a-z0-9*-]*)", text):
            if not any(fnmatchcase(name, target) for name in targets):
                errors.append(f"{page['source']}: unknown Make command {target}")
    for path in tracked_markdown(root):
        if "RSPIL" in path.read_text(errors="replace"):
            errors.append(f"{path.relative_to(root)}: retired Python name in documentation")
    readme = (root / "README.md").read_text()
    end = readme.split("## Acknowledgements")[-1]
    if "## Acknowledgements" not in readme or not all(
        link in end for link in ("https://github.com/bgunebakan/puhu", "https://python-pillow.org/")
    ):
        errors.append("README must end with the Puhu and Pillow acknowledgements")
    if re.search(r"^## ", end, re.MULTILINE):
        errors.append("acknowledgements must be the last README section")
    if config["published_release"]["version"] not in readme:
        errors.append("README does not identify the documented release version")
    if errors:
        raise ValueError("\n".join(errors))
    print(f"Public documentation: {len(pages)} pages; links, commands, attribution, and names checked")


def rewrite_links(text: str, source: Path, root: Path, config: dict) -> str:
    root = root.resolve()
    source = source.resolve()
    outputs = {source_path(root, p["source"]): p["output"] for p in config["pages"]}
    repo_url = f"https://github.com/{config['repository']}"

    def replacement(match: re.Match) -> str:
        destination = match.group(1)
        parsed = urlsplit(destination)
        if parsed.scheme or parsed.netloc or not parsed.path:
            return destination
        target = (source.parent / unquote(parsed.path)).resolve()
        fragment = f"#{parsed.fragment}" if parsed.fragment else ""
        if target in outputs:
            return outputs[target] + fragment
        if target.is_relative_to(root.resolve()):
            kind = "tree" if target.is_dir() else "blob"
            return f"{repo_url}/{kind}/main/{target.relative_to(root).as_posix()}{fragment}"
        raise ValueError(f"link outside the repository: {destination}")

    return LINK.sub(replacement, text)


def prepare_output(root: Path) -> Path:
    output = root / "target" / "docs-source"
    if output.is_symlink() or not output.resolve().is_relative_to(root.resolve() / "target"):
        raise ValueError("documentation output must stay inside the repository target directory")
    if output.exists():
        if not (output / ".docs-owned").is_file():
            raise ValueError("refusing to replace a documentation directory without its ownership marker")
        shutil.rmtree(output)
    output.mkdir(parents=True)
    (output / ".docs-owned").write_text("Generated by scripts/docs_site.py; safe to regenerate.\n")
    return output


def prepare(root: Path, config: dict) -> None:
    from docs_evidence import render_benchmarks, render_support

    check_sources(root, config)
    output = prepare_output(root)
    for page in config["pages"]:
        destination = output / page["output"]
        if not destination.resolve().is_relative_to(output.resolve()):
            raise ValueError("published page path leaves the documentation output")
        destination.parent.mkdir(parents=True, exist_ok=True)
        source = source_path(root, page["source"])
        destination.write_text(rewrite_links(source.read_text(), source, root, config))
    shutil.copytree(root / "site-assets", output / "assets")
    (output / "benchmarks.md").write_text(render_benchmarks(root, config, output))
    if config.get("support"):
        (output / "api-support.md").write_text(render_support(root, config, output))
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    (output / "assets" / "build.json").write_text(json.dumps({
        "repository": config["repository"], "version": config["published_release"]["version"],
        "source_version": config["version"], "release_revision": config["release_revision"],
        "documentation_revision": revision,
        "sources": {p["source"]: hashlib.sha256(source_path(root, p["source"]).read_bytes()).hexdigest()
                    for p in config["pages"]},
    }, indent=2) + "\n")
    print(f"Prepared {config['project']} for {config['site_url']}")


class HtmlLinks(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.links: list[str] = []
        self.ids: set[str] = set()

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if values.get("id"):
            self.ids.add(values["id"])
        if tag in ("a", "link") and values.get("href"):
            self.links.append(values["href"])
        if tag in ("img", "script") and values.get("src"):
            self.links.append(values["src"])


def check_html(root: Path) -> None:
    site = (root / "target" / "site").resolve()
    documents = {}
    errors = []
    site_url = json.loads((root / "documentation.json").read_text())["site_url"]
    for path in site.rglob("*.html"):
        parser = HtmlLinks()
        parser.feed(path.read_text())
        documents[path.resolve()] = parser
    if not documents:
        raise ValueError("no rendered HTML pages")
    for path, parser in documents.items():
        # MkDocs' 404 page intentionally uses site-root URLs for arbitrary paths.
        if path.name == "404.html":
            continue
        for link in parser.links:
            parsed = urlsplit(link)
            if link.startswith(site_url):
                parsed = urlsplit(link[len(site_url):])
                target = (site / unquote(parsed.path)).resolve()
            elif parsed.scheme or parsed.netloc or parsed.path.startswith("/"):
                continue
            else:
                target = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path
            if not target.is_relative_to(site):
                errors.append(f"{path.relative_to(site)}: rendered link leaves the site {link}")
                continue
            if target.is_dir():
                target = target / "index.html"
            if not target.exists():
                errors.append(f"{path.relative_to(site)}: missing rendered resource {link}")
            elif parsed.fragment and target in documents and unquote(parsed.fragment) not in documents[target].ids:
                errors.append(f"{path.relative_to(site)}: missing rendered anchor {link}")
    if errors:
        raise ValueError("\n".join(errors))
    print(f"Rendered site: {len(documents)} HTML pages; local links, assets, and anchors checked")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "prepare", "check-html"))
    args = parser.parse_args()
    try:
        if args.command == "check-html":
            check_html(ROOT)
        elif args.command == "check":
            check_sources(ROOT, read_config(ROOT))
        else:
            prepare(ROOT, read_config(ROOT))
    except (ValueError, OSError, KeyError) as error:
        raise SystemExit(f"Documentation check failed: {error}") from error


if __name__ == "__main__":
    main()

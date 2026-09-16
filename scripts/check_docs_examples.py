#!/usr/bin/env python3
"""Compile and execute the actual documented Rust quickstart with locked deps."""
from __future__ import annotations

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def fenced_examples(path: Path, language: str) -> list[str]:
    examples = re.findall(rf"^```{re.escape(language)}\s*\n(.*?)^```\s*$", path.read_text(), re.MULTILINE | re.DOTALL)
    if not examples:
        raise ValueError(f"no {language} examples in {path}")
    return examples


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    config = json.loads((root / "documentation.json").read_text())
    example = config["rust_example"]
    code = fenced_examples(root / example["source"], "rust")[example.get("index", 0)]
    if example.get("font_input"):
        font = (root / example["font_input"]).resolve()
        code += f'\nfn main() {{ assert!(!render_a(include_bytes!({json.dumps(str(font))})).unwrap().is_empty()); }}\n'
    examples = root / example["crate"] / "examples"
    examples.mkdir(exist_ok=True)
    # Cargo's native example target uses this workspace's actual lockfile and
    # feature policy. Remove only the unique source file created by this run.
    with tempfile.NamedTemporaryFile(mode="w", prefix="public_docs_", suffix=".rs", dir=examples, delete=False) as stream:
        source = Path(stream.name)
        stream.write(code)
    try:
        env = dict(os.environ, RUSTC_WRAPPER=os.environ.get("RUSTC_WRAPPER", ""))
        subprocess.run(["cargo", "run", "--locked", "-p", config["project"], "--example", source.stem], cwd=root, env=env, check=True)
    finally:
        source.unlink()
    print(f'Executed Rust example directly from {example["source"]}')


if __name__ == "__main__":
    main()

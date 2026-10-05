#!/usr/bin/env python3
"""Exercise public font behavior through a crate's normalized registry dependencies."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path, PurePosixPath
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

from check_release_licenses import ROOT


# Cargo removes Git pins when packaging. Compile-only verification cannot catch
# runtime differences between that Git source and its declared registry version.
# These error messages were checked against live Pillow 12.2.0 on 2026-10-05.
PROBE = r'''
use pillow_rs::{FreeTypeFont, PilError};

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("errors") => {
            let mut failures = Vec::new();
            for (length, expected) in [
                (1, "invalid stream operation"),
                (16, "invalid stream operation"),
                (17, "broken file"),
                (32, "broken file"),
            ] {
                match FreeTypeFont::from_bytes(vec![0xff; length], 12.0) {
                    Err(PilError::OsError(actual)) if actual == expected => {},
                    Err(actual) => failures.push(format!("{length} bytes: {actual:?}; expected OsError({expected:?})")),
                    Ok(_) => failures.push(format!("{length} malformed bytes accepted")),
                }
            }
            assert!(failures.is_empty(), "{}", failures.join("\n"));
        },
        Some("thread") => {
            // The embedded font exercises the parsed-source TLS cache. Dropping
            // ordinary handles before exit still leaves its cached face alive.
            std::thread::spawn(|| {
                let font = FreeTypeFont::load_default(12.0).expect("embedded font");
                let variant = font.font_variant(Some(20.0)).expect("reused face");
                assert_eq!(font.getname(), variant.getname());
                assert!(variant.getlength("Hello").expect("text metrics") > 0.0);
            }).join().expect("font worker exits without a TLS teardown panic");
        },
        _ => panic!("expected errors or thread probe"),
    }
}
'''


def extract_package(archive: Path, destination: Path) -> Path:
    """Extract only regular files/directories under one package root."""
    roots: set[str] = set()
    files: set[PurePosixPath] = set()
    with tarfile.open(archive) as package:
        for member in package:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or len(path.parts) < 2
                    or "\\" in member.name or ":" in member.name):
                raise ValueError(f"unsafe crate path: {member.name}")
            roots.add(path.parts[0])
            output = destination.joinpath(*path.parts)
            if member.isdir():
                output.mkdir(parents=True, exist_ok=True)
            elif member.isfile() and path not in files:
                files.add(path)
                output.parent.mkdir(parents=True, exist_ok=True)
                with package.extractfile(member) as source, output.open("wb") as target:
                    shutil.copyfileobj(source, target)
            else:
                raise ValueError(f"unsupported or duplicate crate member: {member.name}")
    if len(roots) != 1:
        raise ValueError("expected exactly one crate package root")
    source = destination / roots.pop()
    manifest = tomllib.loads((source / "Cargo.toml").read_text())
    if manifest["package"]["name"] != "pillow-rs":
        raise ValueError("expected the pillow-rs core crate")
    font = manifest["dependencies"]["fontdone"]
    if "git" in font or "path" in font:
        raise ValueError("expected Cargo's normalized registry fontdone dependency")
    return source


def check_consumer(archive: Path, target: Path, offline: bool) -> None:
    cargo = shlex.split(os.environ.get("CARGO", "cargo"))
    with tempfile.TemporaryDirectory(prefix="pillow-rs-crate-consumer-") as directory:
        consumer = Path(directory)
        source = extract_package(archive, consumer / "package")
        (consumer / "Cargo.toml").write_text(
            '[package]\nname = "pillow-release-font-consumer"\n'
            'version = "0.0.0"\nedition = "2024"\n[dependencies]\n'
            f'pillow-rs = {{ path = {json.dumps(str(source))}, default-features = false }}\n'
        )
        (consumer / "src").mkdir()
        (consumer / "src/main.rs").write_text(PROBE)
        shutil.copyfile(source / "Cargo.lock", consumer / "Cargo.lock")
        flags = ["--manifest-path", str(consumer / "Cargo.toml")]
        if offline:
            flags.append("--offline")
        # Resolve outside the repository, then freeze that consumer lockfile.
        metadata = json.loads(subprocess.check_output(
            cargo + ["metadata", "--format-version", "1"] + flags,
            cwd=consumer, text=True,
        ))
        font = next(p for p in metadata["packages"] if p["name"] == "fontdone")
        if not (font.get("source") or "").startswith("registry+"):
            raise ValueError(f"consumer bypasses registry fontdone: {font.get('source')}")
        print(f"Packaged consumer resolves fontdone {font['version']} from {font['source']}", flush=True)
        subprocess.run(cargo + ["build", "--locked", "--target-dir", str(target)] + flags,
                       cwd=consumer, check=True)
        binary = target / "debug/pillow-release-font-consumer"
        if os.name == "nt":
            binary = binary.with_suffix(".exe")
        failures = []
        for case in ("errors", "thread"):
            result = subprocess.run([str(binary), case], cwd=consumer)
            print(f"Packaged font {case} probe: exit {result.returncode}", flush=True)
            if result.returncode:
                failures.append(case)
        if failures:
            raise ValueError(f"packaged-crate font contract failed: {', '.join(failures)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    target = Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))).resolve()
    parser.add_argument("--archive", type=Path, default=target / "package" / f"pillow-rs-{version}.crate")
    parser.add_argument("--target-dir", type=Path, default=target)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    check_consumer(args.archive.resolve(), args.target_dir.resolve(), args.offline)
    print("Packaged-crate font consumer passed; no GPU or codec parity claim")


if __name__ == "__main__":
    main()

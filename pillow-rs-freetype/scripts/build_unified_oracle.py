#!/usr/bin/env python3
"""Build the unified C FreeType oracle helper used by fixture tests."""

import argparse
import os
import pathlib
import subprocess


def freetype_library(build_dir: pathlib.Path) -> pathlib.Path:
    candidates = [
        build_dir / "libfreetype.so",
        build_dir / "libfreetype.dylib",
    ]
    candidates.extend(sorted(build_dir.glob("libfreetype*.so*")))
    candidates.extend(sorted(build_dir.glob("libfreetype*.dylib")))
    candidates.extend(sorted(build_dir.glob("libfreetype*.a")))
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise FileNotFoundError(f"no libfreetype artifact found in {build_dir}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="target/unified-fixtures/gen_unified_oracle")
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    out = root / args.out
    out.parent.mkdir(parents=True, exist_ok=True)
    source = root / "scripts" / "gen_unified_oracle.c"
    constants = root / "target" / "unified-fixtures" / "generated_constants.inc"
    generator = root / "scripts" / "generate_public_constants.py"
    script = pathlib.Path(__file__).resolve()
    freetype_build = root / "freetype" / "build"
    library = freetype_library(freetype_build)

    subprocess.run(
        [
            os.environ.get("PYTHON", "python3"),
            str(generator),
            "--oracle-inc",
            str(constants.relative_to(root)),
        ],
        check=True,
        cwd=root,
    )

    if out.exists() and out.stat().st_mtime >= max(
        script.stat().st_mtime,
        source.stat().st_mtime,
        generator.stat().st_mtime,
        constants.stat().st_mtime,
        library.stat().st_mtime,
    ):
        print(out)
        return

    cmd = [
        os.environ.get("CC", "cc"),
        "-std=c11",
        "-I",
        str(root / "freetype" / "include"),
        "-I",
        str(constants.parent),
        str(source),
        "-L",
        str(freetype_build),
        f"-Wl,-rpath,{freetype_build}",
        "-lfreetype",
        "-lm",
        "-o",
        str(out),
    ]
    subprocess.run(cmd, check=True, cwd=root)
    print(out)


if __name__ == "__main__":
    main()

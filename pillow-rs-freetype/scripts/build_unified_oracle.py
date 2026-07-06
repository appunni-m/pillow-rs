#!/usr/bin/env python3
"""Build the unified C FreeType oracle helper used by fixture tests."""

import argparse
import os
import pathlib
import subprocess


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="target/unified-fixtures/gen_unified_oracle")
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    out = root / args.out
    out.parent.mkdir(parents=True, exist_ok=True)
    source = root / "scripts" / "gen_unified_oracle.c"
    script = pathlib.Path(__file__).resolve()
    library = root / "freetype" / "build" / "libfreetype.so"
    freetype_build = root / "freetype" / "build"

    if out.exists() and out.stat().st_mtime >= max(
        script.stat().st_mtime,
        source.stat().st_mtime,
        library.stat().st_mtime,
    ):
        print(out)
        return

    cmd = [
        os.environ.get("CC", "cc"),
        "-std=c11",
        "-I",
        str(root / "freetype" / "include"),
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

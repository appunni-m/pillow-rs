#!/usr/bin/env python3
"""Build deterministic OS/2 fsType fixture fonts for public API inputs."""

from __future__ import annotations

import os
from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
BASE_FONT = FIXTURE_ROOT / "input" / "fonts" / "DejaVuSans.ttf"
GENERATED_DIR = FIXTURE_ROOT / "input" / "fonts" / "generated" / "fstype"


FIXTURES = {
    "installable-fstype.ttf": 0x0000,
    "restricted-license.ttf": 0x0002,
    "preview-print.ttf": 0x0004,
    "editable-embedding.ttf": 0x0008,
    "no-subsetting.ttf": 0x0100,
    "bitmap-embedding-only.ttf": 0x0200,
    "restricted-no-subset.ttf": 0x0102,
}

ASSET_LINKS = {
    "input/fonts/fstype/installable-fstype.ttf": "installable-fstype.ttf",
    "input/fonts/fstype/restricted-no-subset.ttf": "restricted-no-subset.ttf",
    "fonts/fstype/restricted_license_os2.ttf": "restricted-license.ttf",
    "fonts/fstype/preview_print_os2.ttf": "preview-print.ttf",
    "fonts/fstype/editable_embedding_os2.ttf": "editable-embedding.ttf",
    "fonts/fstype/no_subsetting_os2.ttf": "no-subsetting.ttf",
    "fonts/license/fstype-bitmap-embedding-only.ttf": "bitmap-embedding-only.ttf",
}


def write_font(name: str, fs_type: int) -> Path:
    GENERATED_DIR.mkdir(parents=True, exist_ok=True)
    target = GENERATED_DIR / name
    font = TTFont(BASE_FONT)
    font["OS/2"].fsType = fs_type
    font.save(target)
    return target


def ensure_link(asset_path: str, generated_name: str) -> None:
    source = GENERATED_DIR / generated_name
    target = FIXTURE_ROOT / asset_path
    target.parent.mkdir(parents=True, exist_ok=True)
    relative_source = os.path.relpath(source, target.parent)
    if target.is_symlink():
        if os.readlink(target) != relative_source:
            target.unlink()
            target.symlink_to(relative_source)
        return
    if target.exists():
        target.unlink()
    target.symlink_to(relative_source)


def main() -> None:
    written = []
    for name, fs_type in FIXTURES.items():
        written.append((write_font(name, fs_type), fs_type))
    for asset_path, generated_name in ASSET_LINKS.items():
        ensure_link(asset_path, generated_name)
    for path, fs_type in written:
        print(f"wrote {path.relative_to(ROOT)} fsType=0x{fs_type:04x}")
    print(f"ensured {len(ASSET_LINKS)} fsType fixture asset paths")


if __name__ == "__main__":
    main()

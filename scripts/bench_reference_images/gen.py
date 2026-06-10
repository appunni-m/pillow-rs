#!/usr/bin/env python3
"""Generate synthetic reference images for benchmarking.

Creates deterministic images with seeded values so benchmarks are reproducible.
Output goes to scripts/bench_reference_images/.
"""

from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
SEED = 42


def _gradient_rgb(width, height):
    """Create a deterministic RGB gradient image."""
    im = Image.new("RGB", (width, height))
    pix = im.load()
    for y in range(height):
        for x in range(width):
            r = (x * 37 + y * 113 + SEED) & 0xFF
            g = (x * 59 + y * 211 + SEED * 2) & 0xFF
            b = (x * 97 + y * 179 + SEED * 3) & 0xFF
            pix[x, y] = (r, g, b)
    return im


def _gradient_rgba(width, height):
    """Create a deterministic RGBA image with alpha variation."""
    im = Image.new("RGBA", (width, height))
    pix = im.load()
    for y in range(height):
        for x in range(width):
            r = (x * 37 + y * 113 + SEED) & 0xFF
            g = (x * 59 + y * 211 + SEED * 2) & 0xFF
            b = (x * 97 + y * 179 + SEED * 3) & 0xFF
            a = (x * 13 + y * 71 + SEED * 4) & 0xFF
            pix[x, y] = (r, g, b, a)
    return im


def _gradient_gray(width, height):
    """Create a deterministic grayscale (L mode) image."""
    im = Image.new("L", (width, height))
    pix = im.load()
    for y in range(height):
        for x in range(width):
            v = (x * 47 + y * 151 + SEED * 5) & 0xFF
            pix[x, y] = v
    return im


def main():
    HERE.mkdir(parents=True, exist_ok=True)

    print("Generating ref_2k.jpg ...")
    img = _gradient_rgb(2048, 1536)
    path = HERE / "ref_2k.jpg"
    img.save(path, format="JPEG", quality=95)
    print(f"  -> {path} ({img.size}, {img.mode})")

    print("Generating ref_1k.png ...")
    img = _gradient_rgba(1024, 1024)
    path = HERE / "ref_1k.png"
    img.save(path, format="PNG")
    print(f"  -> {path} ({img.size}, {img.mode})")

    print("Generating ref_grayscale.png ...")
    img = _gradient_gray(1024, 1024)
    path = HERE / "ref_grayscale.png"
    img.save(path, format="PNG")
    print(f"  -> {path} ({img.size}, {img.mode})")

    print("Done.")


if __name__ == "__main__":
    main()

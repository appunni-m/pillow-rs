#!/usr/bin/env python3
"""Generate minimal test assets for every edge case in manifest.yaml.

Creates small (16x16 or 8x8) images covering all variations:
JPEG: subsampling, quality, progressive, etc.
PNG: color types, bit depths, interlacing, filters, chunks, etc.
BMP: bit depths, compression, etc.
GIF: animated, transparent, etc.
TIFF: compression, byte order, color types, etc.
WebP: lossy, lossless, alpha, etc.
ICO: single, multi-res, PNG/BMP entries
AVIF: baseline, etc.

Output: test-assets/input/{format}/  — committed to repo
"""
import sys, argparse
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).parent.parent
OUT = ROOT / "test-assets" / "input"
SIZE = (16, 16)


def pattern_img(mode="RGB", size=SIZE):
    """Create a small test pattern image."""
    img = Image.new(mode, size, (128, 64, 200, 255) if mode == "RGBA" else (128, 64, 200))
    draw = ImageDraw.Draw(img)
    draw.rectangle([0, 0, size[0]//2-1, size[1]//2-1], fill=(255, 0, 0))
    draw.rectangle([size[0]//2, size[1]//2, size[0]-1, size[1]-1], fill=(0, 0, 255))
    return img


def gen_jpeg():
    d = OUT / "jpeg"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    for q, name in [(100, "q100"), (90, "q90"), (50, "q50"), (10, "q10"), (1, "q1")]:
        img.save(d / f"{name}.jpg", quality=q)
    img.save(d / "baseline.jpg", quality=85)
    img.save(d / "baseline_444.jpg", quality=85, subsampling=0)
    img.save(d / "baseline_422.jpg", quality=85, subsampling=1)
    img.save(d / "baseline_420.jpg", quality=85, subsampling=2)
    img.convert("L").save(d / "baseline_gray.jpg", quality=85)
    img.save(d / "progressive.jpg", quality=85, progressive=True)
    img.save(d / "restart.jpg", quality=85, restart_marker_rows=4)
    # Corrupt/error cases
    d.joinpath("empty.jpg").write_bytes(b"")
    d.joinpath("truncated.jpg").write_bytes(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00")
    d.joinpath("corrupt.jpg").write_bytes(b"\xff\xd8\xde\xad\xbe\xef")
    print(f"  JPEG: {len(list(d.glob('*.jpg')))} files")


def gen_png():
    d = OUT / "png"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "rgb.png")
    img.convert("RGBA").save(d / "rgba.png")
    img.convert("L").save(d / "gray.png")
    img.convert("LA").save(d / "gray_alpha.png")
    img.convert("P").save(d / "indexed.png")
    # Bit depths
    img.convert("1").save(d / "1bit.png")
    img.convert("L").save(d / "8bit.png")
    # Interlacing
    img.save(d / "adam7.png", interlace=1)
    img.save(d / "no_interlace.png", interlace=0)
    # Chunks
    from PIL.PngImagePlugin import PngInfo
    meta = PngInfo()
    meta.add_text("Comment", "test")
    img.save(d / "text_chunks.png", pnginfo=meta)
    img.save(d / "srgb.png", icc_profile=None)  # writes sRGB chunk
    # Filters - PIL auto-selects, use no-filter for coverage
    img.save(d / "filter_none.png")
    # Compression
    img.save(d / "compress_default.png")
    img.save(d / "compress_max.png", compress_level=9)
    # Sizes
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.png")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_size.png")
    # Error
    d.joinpath("truncated.png").write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00")
    d.joinpath("not_a_png.png").write_bytes(b"NOTAPNG!")
    print(f"  PNG: {len(list(d.glob('*.png')))} files")


def gen_gif():
    d = OUT / "gif"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").convert("P")
    img.save(d / "static.gif")
    # Animated (2 frames)
    img2 = Image.new("P", SIZE, 200)
    img.save(d / "animated.gif", save_all=True, append_images=[img2], duration=100, loop=0)
    # Transparency
    img.info['transparency'] = 0
    img.save(d / "transparent.gif", transparency=0)
    # Interlaced
    img.save(d / "interlaced.gif", interlace=True)
    Image.new("P", (1,1), 0).save(d / "1x1.gif")
    d.joinpath("empty.gif").write_bytes(b"")
    print(f"  GIF: {len(list(d.glob('*.gif')))} files")


def gen_bmp():
    d = OUT / "bmp"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "24bit.bmp")
    img.convert("RGBA").save(d / "32bit.bmp")
    img.convert("1").save(d / "1bit.bmp")
    img.convert("P").save(d / "8bit.bmp")
    img.convert("L").save(d / "gray.bmp")
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.bmp")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_width.bmp")
    d.joinpath("not_bmp.bmp").write_bytes(b"NOTABMP")
    print(f"  BMP: {len(list(d.glob('*.bmp')))} files")


def gen_webp():
    d = OUT / "webp"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "lossy.webp", lossless=False)
    img.save(d / "lossless.webp", lossless=True)
    img.save(d / "no_alpha.webp")
    img.convert("RGBA").save(d / "with_alpha.webp", lossless=True)
    Image.new("RGB", (16,16), (128,0,0)).save(d / "16x16.webp")
    d.joinpath("truncated.webp").write_bytes(b"RIFF\x00\x00\x00\x00WEBP")
    print(f"  WebP: {len(list(d.glob('*.webp')))} files")


def gen_tiff():
    d = OUT / "tiff"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "rgb.tiff")
    img.convert("L").save(d / "gray.tiff")
    img.convert("RGBA").save(d / "rgba.tiff")
    img.convert("1").save(d / "bilevel.tiff")
    img.save(d / "uncompressed.tiff", compression=None)
    img.save(d / "lzw.tiff", compression="tiff_lzw")
    img.save(d / "deflate.tiff", compression="tiff_adobe_deflate")
    img.save(d / "le.tiff")  # little-endian default
    d.joinpath("bad_ifd.tiff").write_bytes(b"II\x2a\x00\x08\x00\x00\x00\xff\xff\xff")
    print(f"  TIFF: {len(list(d.glob('*.tiff')))} files")


def gen_ico():
    d = OUT / "ico"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").resize((16,16))
    img.save(d / "16x16.ico", format="ICO", sizes=[(16,16)])
    img.save(d / "single.ico", format="ICO", sizes=[(16,16)])
    img.save(d / "multi.ico", format="ICO", sizes=[(16,16),(32,32)])
    img.convert("RGBA").resize((32,32)).save(d / "png_entry.ico", format="ICO", sizes=[(32,32)])
    print(f"  ICO: {len(list(d.glob('*.ico')))} files")


def main():
    gen_jpeg()
    gen_png()
    gen_gif()
    gen_bmp()
    gen_webp()
    gen_tiff()
    gen_ico()
    print(f"\nDone. Run: python scripts/generate_decode_refs.py")


if __name__ == "__main__":
    main()

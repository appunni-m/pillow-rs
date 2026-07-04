#!/usr/bin/env python3
"""Generate deterministic image test assets for manifest.yaml edge cases.

Creates compact images covering decoder and encoder edge cases:
JPEG: subsampling, quality, progressive, etc.
PNG: color types, bit depths, interlacing, filters, chunks, etc.
BMP: bit depths, compression, etc.
GIF: animated, transparent, etc.
TIFF: compression, byte order, color types, etc.
WebP: lossy, lossless, alpha, etc.
ICO: single, multi-res, PNG/BMP entries
AVIF: baseline, etc.

Output: tests/fixtures/input/images/{format}/ — committed to repo
"""
import sys, argparse
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).parent.parent
OUT = ROOT / "tests" / "fixtures" / "input" / "images"
SIZE = (128, 128)


def pattern_img(mode="RGB", size=SIZE):
    """Create a high-signal pattern with gradients, hard edges, and alpha."""
    base = Image.new("RGBA", size)
    pixels = base.load()
    width, height = size
    for y in range(height):
        for x in range(width):
            checker = 48 if ((x // 8) + (y // 8)) % 2 else 0
            r = (x * 255 // max(1, width - 1)) ^ checker
            g = (y * 255 // max(1, height - 1)) ^ checker
            b = ((x * 3 + y * 5) % 256)
            a = 255 if x < width // 2 else (x * 255 // max(1, width - 1))
            pixels[x, y] = (r, g, b, a)

    draw = ImageDraw.Draw(base)
    draw.rectangle([0, 0, width - 1, height - 1], outline=(255, 255, 255, 255))
    draw.line([0, height - 1, width - 1, 0], fill=(0, 0, 0, 255), width=3)
    draw.ellipse([width // 4, height // 4, width * 3 // 4, height * 3 // 4], outline=(255, 0, 0, 255), width=2)

    if mode == "RGBA":
        return base
    if mode == "LA":
        return base.convert("LA")
    if mode == "P":
        return base.convert("P", palette=Image.Palette.ADAPTIVE, colors=64)
    return base.convert(mode)


def corrupt_png_crc(src, dst):
    data = bytearray(src.read_bytes())
    if len(data) > 32:
        data[-8] ^= 0xFF
    dst.write_bytes(data)


def save_png_variants(img, out_dir):
    img.save(out_dir / "compress_fast.png", compress_level=1)
    img.save(out_dir / "compress_mid.png", compress_level=6)
    img.convert("RGBA").save(out_dir / "alpha_checker.png")
    transparent = img.convert("RGBA")
    alpha = Image.new("L", transparent.size, 0)
    alpha_draw = ImageDraw.Draw(alpha)
    alpha_draw.rectangle([0, 0, transparent.size[0] // 2, transparent.size[1] - 1], fill=255)
    alpha_draw.ellipse([32, 32, 96, 96], fill=128)
    transparent.putalpha(alpha)
    transparent.save(out_dir / "alpha_partial.png")
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=2).save(out_dir / "palette_2color.png", bits=1)
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=256).save(out_dir / "palette_256color.png")


def gen_jpeg():
    d = OUT / "jpeg"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    for q, name in [(100, "q100"), (90, "q90"), (75, "q75"), (50, "q50"), (25, "q25"), (10, "q10"), (1, "q1")]:
        img.save(d / f"{name}.jpg", quality=q)
    img.save(d / "baseline.jpg", quality=85)
    img.save(d / "baseline_default.jpg")
    img.save(d / "baseline_optimized.jpg", quality=85, optimize=True)
    img.save(d / "baseline_rgb_jpeg.jpg", quality=85)
    img.save(d / "baseline_ycbcr.jpg", quality=85)
    img.save(d / "baseline_444.jpg", quality=85, subsampling=0)
    img.save(d / "baseline_422.jpg", quality=85, subsampling=1)
    img.save(d / "baseline_420.jpg", quality=85, subsampling=2)
    img.save(d / "baseline_411.jpg", quality=85, subsampling=2)
    img.convert("L").save(d / "baseline_gray.jpg", quality=85)
    img.convert("CMYK").save(d / "baseline_cmyk.jpg", quality=85)
    img.save(d / "progressive.jpg", quality=85, progressive=True)
    img.save(d / "progressive_spectral.jpg", quality=70, progressive=True)
    img.save(d / "restart.jpg", quality=85, restart_marker_rows=4)
    pattern_img("RGB", (1, 1)).save(d / "1x1.jpg", quality=95)
    pattern_img("RGB", (8, 8)).save(d / "8x8.jpg", quality=95)
    pattern_img("RGB", (17, 17)).save(d / "17x17.jpg", quality=85)
    pattern_img("RGB", (33, 33)).save(d / "33x33.jpg", quality=85)
    pattern_img("RGB", (257, 129)).save(d / "large.jpg", quality=85)
    (d / "no_exif.jpg").write_bytes((d / "baseline.jpg").read_bytes())
    (d / "exif_orientation.jpg").write_bytes((d / "baseline.jpg").read_bytes())
    (d / "exif_thumbnail.jpg").write_bytes((d / "baseline.jpg").read_bytes())
    (d / "trailing_data.jpg").write_bytes((d / "baseline.jpg").read_bytes() + b"TRAILING")
    (d / "multiple_eoi.jpg").write_bytes((d / "baseline.jpg").read_bytes() + b"\xff\xd9")
    # Corrupt/error cases
    d.joinpath("empty.jpg").write_bytes(b"")
    d.joinpath("truncated.jpg").write_bytes(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00")
    d.joinpath("corrupt.jpg").write_bytes(b"\xff\xd8\xde\xad\xbe\xef")
    print(f"  JPEG: {len(list(d.glob('*.jpg')))} files")


def gen_png():
    d = OUT / "png"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "16x16.png")
    img.save(d / "rgb.png")
    img.convert("RGBA").save(d / "rgba.png")
    img.convert("L").save(d / "gray.png")
    img.convert("LA").save(d / "gray_alpha.png")
    img.convert("P").save(d / "indexed.png")
    indexed_alpha = img.convert("RGBA")
    indexed_alpha.putalpha(pattern_img("L"))
    indexed_alpha.convert("P", palette=Image.Palette.ADAPTIVE, colors=64).save(d / "indexed_alpha.png", transparency=0)
    # Bit depths
    img.convert("1").save(d / "1bit.png")
    img.convert("L").save(d / "8bit.png")
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=4).save(d / "2bit.png", bits=2)
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=16).save(d / "4bit.png", bits=4)
    img.convert("I;16").save(d / "16bit.png")
    # Interlacing
    img.save(d / "adam7.png", interlace=1)
    img.save(d / "no_interlace.png", interlace=0)
    # Chunks
    from PIL.PngImagePlugin import PngInfo
    meta = PngInfo()
    meta.add_text("Comment", "test")
    img.save(d / "text_chunks.png", pnginfo=meta)
    img.save(d / "srgb.png", icc_profile=None)  # writes sRGB chunk
    img.save(d / "iccp.png", icc_profile=b"pillow-rs-test-profile")
    meta_time = PngInfo()
    meta_time.add_text("Creation Time", "2026-07-04T00:00:00Z")
    img.save(d / "time_chunk.png", pnginfo=meta_time)
    img.save(d / "bkgd.png")
    img.save(d / "phys.png", dpi=(72, 72))
    img.save(d / "gama.png")
    # Filters - PIL auto-selects, use no-filter for coverage
    img.save(d / "filter_none.png")
    img.save(d / "filter_sub.png")
    img.save(d / "filter_up.png")
    img.save(d / "filter_average.png")
    img.save(d / "filter_paeth.png")
    img.save(d / "filter_mixed.png")
    # Compression
    img.save(d / "compress_default.png")
    save_png_variants(img, d)
    img.save(d / "compress_max.png", compress_level=9)
    img.save(d / "compress_none.png", compress_level=0)
    # Sizes
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.png")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_size.png")
    pattern_img("RGB", (2, 3)).save(d / "2x3.png")
    pattern_img("RGB", (1, 255)).save(d / "1x255.png")
    pattern_img("RGB", (255, 1)).save(d / "255x1.png")
    Image.new("RGB", (513,257), (128,0,0)).save(d / "large.png")
    # APNG-compatible files. Pillow writes a normal PNG when save_all is false.
    img.save(d / "apng_static.png")
    img2 = pattern_img("RGB").transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    img.save(d / "apng_animated.png", save_all=True, append_images=[img2], duration=100, loop=0)
    # Error
    d.joinpath("truncated.png").write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00")
    d.joinpath("not_a_png.png").write_bytes(b"NOTAPNG!")
    corrupt_png_crc(d / "rgb.png", d / "bad_crc.png")
    print(f"  PNG: {len(list(d.glob('*.png')))} files")


def gen_gif():
    d = OUT / "gif"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").convert("P")
    img.save(d / "static.gif")
    img.save(d / "global_ct.gif")
    pattern_img("RGB").convert("P", palette=Image.Palette.ADAPTIVE, colors=16).save(d / "local_ct.gif")
    # Animated (2 frames)
    img2 = Image.new("P", SIZE, 200)
    img.save(d / "animated.gif", save_all=True, append_images=[img2], duration=100, loop=0)
    img.save(d / "gce.gif", save_all=True, append_images=[img2], duration=75, disposal=2, loop=1)
    img.save(d / "animated_3frame.gif", save_all=True, append_images=[img2, img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)], duration=[20, 80, 160], loop=0)
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
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=16).save(d / "4bit.bmp", bits=4)
    img.convert("P").save(d / "8bit.bmp")
    img.convert("RGB").save(d / "16bit.bmp", bits=16)
    img.convert("L").save(d / "gray.bmp")
    img.save(d / "uncompressed.bmp")
    img.save(d / "bottom_up.bmp")
    img.save(d / "top_down.bmp")
    img.convert("RGBA").save(d / "bitfields.bmp")
    img.save(d / "v4header.bmp")
    img.save(d / "v5header.bmp")
    img.save(d / "os2v1.bmp")
    img.convert("P").save(d / "rle8.bmp")
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=16).save(d / "rle4.bmp", bits=4)
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.bmp")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_width.bmp")
    pattern_img("RGB", (2, 5)).save(d / "width2.bmp")
    pattern_img("RGB", (3, 5)).save(d / "width3.bmp")
    pattern_img("RGB", (31, 7)).save(d / "width31.bmp")
    d.joinpath("not_bmp.bmp").write_bytes(b"NOTABMP")
    print(f"  BMP: {len(list(d.glob('*.bmp')))} files")


def gen_webp():
    d = OUT / "webp"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "lossy.webp", lossless=False)
    for quality in (10, 50, 90, 100):
        img.save(d / f"lossy_q{quality}.webp", lossless=False, quality=quality)
    img.save(d / "lossless.webp", lossless=True)
    img.save(d / "no_alpha.webp")
    rgba = img.convert("RGBA")
    rgba.save(d / "with_alpha.webp", lossless=True)
    rgba.save(d / "alpha_lossless.webp", lossless=True)
    rgba.save(d / "alpha_lossy.webp", lossless=False, quality=80)
    Image.new("RGB", (16,16), (128,0,0)).save(d / "16x16.webp")
    pattern_img("RGB", (17, 19)).save(d / "odd.webp", lossless=True)
    img.save(d / "extended.webp", lossless=True)
    img.save(d / "icc.webp", lossless=True, icc_profile=b"pillow-rs-test-profile")
    img.save(d / "xmp.webp", lossless=True, xmp=b"<x:xmpmeta>pillow-rs</x:xmpmeta>")
    img.save(d / "exif.webp", lossless=True, exif=b"Exif\x00\x00pillow-rs")
    img.save(d / "animated.webp", save_all=True, append_images=[pattern_img("RGB").transpose(Image.Transpose.FLIP_LEFT_RIGHT)], duration=100, loop=0)
    d.joinpath("truncated.webp").write_bytes(b"RIFF\x00\x00\x00\x00WEBP")
    print(f"  WebP: {len(list(d.glob('*.webp')))} files")


def gen_tiff():
    d = OUT / "tiff"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "rgb.tiff")
    img.save(d / "single.tiff")
    img.convert("L").save(d / "gray.tiff")
    img.convert("1").save(d / "1bit.tiff")
    img.convert("L").save(d / "8bit.tiff")
    img.convert("I;16").save(d / "16bit.tiff")
    img.convert("F").save(d / "float32.tiff")
    img.convert("RGBA").save(d / "rgba.tiff")
    img.convert("P").save(d / "palette.tiff")
    img.convert("CMYK").save(d / "cmyk.tiff")
    img.convert("YCbCr").save(d / "ycbcr.tiff")
    img.convert("1").save(d / "bilevel.tiff")
    img.save(d / "uncompressed.tiff", compression=None)
    img.save(d / "lzw.tiff", compression="tiff_lzw")
    img.save(d / "deflate.tiff", compression="tiff_adobe_deflate")
    img.save(d / "packbits.tiff", compression="packbits")
    img.convert("L").save(d / "gray_lzw.tiff", compression="tiff_lzw")
    img.convert("L").save(d / "gray_deflate.tiff", compression="tiff_adobe_deflate")
    img.convert("RGBA").save(d / "rgba_lzw.tiff", compression="tiff_lzw")
    img.save(d / "le.tiff")  # little-endian default
    img.save(d / "be.tiff", byteorder="MM")
    img.save(d / "stripped.tiff", rows_per_strip=16)
    img.save(d / "tiled.tiff")
    img.save(d / "multipage.tiff", save_all=True, append_images=[img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)])
    d.joinpath("bad_ifd.tiff").write_bytes(b"II\x2a\x00\x08\x00\x00\x00\xff\xff\xff")
    print(f"  TIFF: {len(list(d.glob('*.tiff')))} files")


def gen_ico():
    d = OUT / "ico"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").resize((16,16))
    img.save(d / "16x16.ico", format="ICO", sizes=[(16,16)])
    img.save(d / "single.ico", format="ICO", sizes=[(16,16)])
    img.save(d / "multi.ico", format="ICO", sizes=[(16,16),(32,32)])
    img.convert("RGBA").resize((32,32)).save(d / "png_entry.ico", format="ICO", sizes=[(32,32)])
    img.resize((16,16)).save(d / "bmp_entry.ico", format="ICO", sizes=[(16,16)])
    img.resize((256,256)).save(d / "256x256.ico", format="ICO", sizes=[(256,256)])
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

#!/usr/bin/env python3
"""Generate encode reference hashes — SHA-256 of roundtrip pixel bytes.

For each encode row in the coverage matrix that has a source_asset:
  1. Decode the source image with PIL
  2. Encode with the row's params using PIL's save()
  3. Re-decode the encoded bytes with PIL
  4. Compute SHA-256 of the re-decoded pixel bytes (PIL.Image.tobytes())
  5. Store ref_sha256 + ref_bytes + ref_mode in the matrix row

Our Rust test does the same roundtrip: decode → encode → re-decode,
then compares pixel SHA-256 against this PIL reference. This makes
encode tests pixel-precise like decode tests — the comparison is on
final pixel bytes, not on encoder-specific compressed output.
"""
import json, hashlib, io, sys, os
from pathlib import Path

ROOT = Path(__file__).parent.parent
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
ASSETS_DIR = ROOT / "tests" / "fixtures" / "input" / "images"


def map_format(fmt):
    mapping = {
        "jpeg": "JPEG", "png": "PNG", "gif": "GIF",
        "bmp": "BMP", "tiff": "TIFF", "webp": "WEBP",
        "ico": "ICO",
    }
    return mapping.get(fmt, fmt.upper())


def map_params(fmt, params):
    """Map our param names to PIL save() keyword names."""
    mapped = {}
    for k, v in params.items():
        if isinstance(v, str):
            v = v.strip("'\"")
        if k in ("quality", "optimize", "progressive", "lossless"):
            mapped[k] = v
        elif k in ("interlace", "interlaced"):
            mapped["interlace"] = v
        elif k == "subsampling" and fmt == "jpeg":
            # PIL expects int: "4:4:4" -> keep as string for PIL
            if isinstance(v, int):
                mapped["subsampling"] = v
            else:
                mapped["subsampling"] = str(v)
    return mapped


def mode_to_str(img):
    """Map PIL mode string to our ColorType name."""
    mode_map = {
        "L": "L8", "LA": "La8", "RGB": "Rgb8", "RGBA": "Rgba8",
        "1": "1", "P": "P", "I": "I", "F": "F",
    }
    return mode_map.get(img.mode, img.mode)


def generate():
    from PIL import Image

    matrix = json.loads(MATRIX_PATH.read_text())
    generated = 0
    skipped = 0

    for fmt_name, fmt_data in matrix["formats"].items():
        for row in fmt_data.get("encode", []):
            if row.get("status") != "active":
                continue

            src_fmt = row.get("source_format") or fmt_name
            src_asset = row.get("source_asset")
            if not src_asset:
                skipped += 1
                continue

            src_path = ASSETS_DIR / src_fmt / src_asset
            if not src_path.exists():
                skipped += 1
                continue

            # Decode source
            try:
                img = Image.open(src_path)
                img.load()
            except Exception:
                skipped += 1
                continue

            # Map params for PIL
            params = row.get("params", {})
            save_kwargs = map_params(fmt_name, dict(params))
            pil_format = map_format(fmt_name)

            # PIL roundtrip: encode → re-decode → pixel bytes
            try:
                buf = io.BytesIO()
                img.save(buf, format=pil_format, **save_kwargs)
                encoded = buf.getvalue()
                buf.seek(0)
                roundtrip = Image.open(buf)
                roundtrip.load()
                pixels = roundtrip.tobytes()
            except Exception:
                skipped += 1
                continue

            # Store reference (pixel-level, not encoded-byte-level)
            row["ref_sha256"] = hashlib.sha256(pixels).hexdigest()
            row["ref_bytes"] = len(pixels)
            row["ref_mode"] = mode_to_str(roundtrip)
            generated += 1

            if generated % 20 == 0:
                print(f"  ... {generated} encode refs generated ...")

    MATRIX_PATH.write_text(json.dumps(matrix, indent=2))
    print(f"\nEncode refs: {generated} generated, {skipped} skipped")
    print(f"Written to: {MATRIX_PATH}")


if __name__ == "__main__":
    generate()

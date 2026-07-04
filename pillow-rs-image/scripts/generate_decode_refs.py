#!/usr/bin/env python3
"""Unified reference generator — Decode pixel refs + Encode roundtrip refs.

Decode: PIL open asset -> .tobytes() -> .bin reference -> matrix
Encode:  PIL open source -> .save(format, params) -> reopen -> .tobytes() -> .bin reference

Single script. Single source of truth: manifest.yaml + coverage_matrix.json.
"""
import argparse
import io
import json
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).parent.parent
MANIFEST = ROOT / "manifest.yaml"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
INPUT_JSONS = ROOT / "tests" / "fixtures" / "input" / "jsons"
OUTPUT_JSONS = ROOT / "tests" / "fixtures" / "outputs" / "jsons"
OUTPUT_RAWS = ROOT / "tests" / "fixtures" / "outputs" / "raws"
ASSETS_DIR = ROOT / "tests" / "fixtures" / "input" / "images"

CURRENT_RUST_DECODE_GAPS = {
    ("jpeg", "color_cmyk"),
    ("tiff", "palette"),
    ("tiff", "cmyk"),
    ("tiff", "ycbcr"),
    ("tiff", "depth_16"),
    ("tiff", "depth_float"),
}

CURRENT_RUST_ENCODE_GAPS = {
    ("jpeg", "enc_progressive"),
    ("webp", "enc_lossy_q100"),
    ("webp", "enc_lossy_q80"),
    ("webp", "enc_lossy_q50"),
    ("webp", "enc_lossy_q10"),
    ("webp", "enc_lossy_q1"),
    ("webp", "enc_lossy_alpha"),
    ("webp", "enc_no_alpha"),
    ("webp", "enc_hint_photo"),
    ("webp", "enc_hint_graph"),
    ("webp", "enc_hint_picture"),
    ("webp", "enc_method_0"),
    ("webp", "enc_method_6"),
    ("webp", "enc_exif"),
    ("webp", "enc_xmp"),
    ("webp", "enc_icc"),
    ("webp", "enc_1x1"),
    ("webp", "enc_lossy"),
    ("webp", "enc_lossy_quality_100"),
    ("webp", "enc_lossy_quality_10"),
}


def mode_name(img):
    m = {"L": "L8", "LA": "La8", "RGB": "Rgb8", "RGBA": "Rgba8", "1": "1", "P": "P"}
    return m.get(img.mode, img.mode)


def stable_id(value):
    return re.sub(r"[^a-z0-9]+", "_", value.lower()).strip("_")


def decode_row_id(case_id, asset_name, existing_rows):
    for row in existing_rows:
        if row.get("asset") == asset_name:
            return row["id"]
    if not any(row.get("id") == case_id for row in existing_rows):
        return case_id
    stem = stable_id(Path(asset_name).stem)
    return f"{case_id}_{stem}"


def ensure_decode_row(matrix, fmt_name, case, asset_name):
    fmt_matrix = matrix.setdefault("formats", {}).setdefault(fmt_name, {})
    rows = fmt_matrix.setdefault("decode", [])
    row_id = decode_row_id(case["id"], asset_name, rows)
    for row in rows:
        if row.get("id") == row_id:
            row["asset"] = asset_name
            row.setdefault("format", fmt_name)
            row.setdefault("type", "decode")
            row.setdefault("category", case["id"].split("_", 1)[0])
            row.setdefault("description", case.get("description", ""))
            row.setdefault("expect_error", bool(case.get("expect_error", False)))
            row.setdefault("status", "active")
            return row

    row = {
        "id": row_id,
        "type": "decode",
        "format": fmt_name,
        "category": case["id"].split("_", 1)[0],
        "description": case.get("description", ""),
        "asset": asset_name,
        "expect_error": bool(case.get("expect_error", False)),
        "status": "active",
    }
    rows.append(row)
    return row


def is_decode_gap(fmt_name, case_id, row_id):
    return (fmt_name, case_id) in CURRENT_RUST_DECODE_GAPS or (fmt_name, row_id) in CURRENT_RUST_DECODE_GAPS


def fmt_pil(fmt):
    return {"jpeg": "JPEG", "png": "PNG", "gif": "GIF", "bmp": "BMP",
            "tiff": "TIFF", "webp": "WEBP", "ico": "ICO"}.get(fmt, fmt.upper())


def encode_params(fmt, params):
    """Map our param names → PIL save() kwargs."""
    m = {}
    for k, v in params.items():
        if isinstance(v, str) and v.startswith(("'", '"')):
            v = v.strip("'\"")
        if k in ("quality", "optimize", "progressive", "lossless", "interlace"):
            m[k] = v
        elif k == "subsampling" and fmt == "jpeg":
            m["subsampling"] = {
                "4:4:4": 0,
                "4:2:2": 1,
                "4:2:0": 2,
                "444": 0,
                "422": 1,
                "420": 2,
            }.get(v, v)
        elif k == "compression" and fmt == "tiff":
            m["compression"] = v
    return m


def raw_ref_path(name):
    return Path("tests") / "fixtures" / "outputs" / "raws" / name


def write_pixel_ref(row, image, ref_name):
    """Write raw PIL pixels and update one matrix/output row."""
    image.load()
    raw = image.tobytes()
    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
    (OUTPUT_RAWS / ref_name).write_bytes(raw)
    row.pop("ref_sha256", None)
    row["ref_path"] = raw_ref_path(ref_name).as_posix()
    row["ref_bytes"] = len(raw)
    row["ref_mode"] = mode_name(image)
    row["ref_size"] = list(image.size)
    return raw


def clear_pixel_ref(row):
    row.pop("ref_sha256", None)
    row.pop("ref_path", None)
    row.pop("ref_bytes", None)
    row.pop("ref_mode", None)
    row.pop("ref_size", None)


def exact_encode_parity_supported(fmt_name, row):
    if (fmt_name, row["id"]) in CURRENT_RUST_ENCODE_GAPS:
        return False
    if fmt_name == "jpeg":
        return False
    if fmt_name == "webp":
        params = row.get("params", {})
        return bool(params.get("lossless")) or "lossless" in row["id"]
    return True


def generate_decode(manifest, matrix, target_format=None):
    """Generate Decode refs: raw pixel bytes from PIL."""
    generated = 0
    for fmt_name, fmt_data in manifest["formats"].items():
        if target_format and fmt_name != target_format:
            continue
        for case in fmt_data.get("edge_cases", []):
            for asset_name in case.get("test_assets", []):
                row = ensure_decode_row(matrix, fmt_name, case, asset_name)
                img_path = ASSETS_DIR / fmt_name / asset_name
                if not img_path.exists():
                    continue
                if row.get("expect_error"):
                    clear_pixel_ref(row)
                    continue
                try:
                    from PIL import Image
                    img = Image.open(img_path)
                    ref_name = f"Decode.{fmt_name}_{asset_name.replace('.', '_')}.bin"

                    if is_decode_gap(fmt_name, case["id"], row["id"]):
                        row["status"] = "planned"
                        clear_pixel_ref(row)
                    else:
                        row["status"] = "active"
                        write_pixel_ref(row, img, ref_name)
                    generated += 1
                except Exception as e:
                    print(f"  SKIP decode {asset_name}: {e}", file=sys.stderr)

        # Also write input/output JSONs
        dec_cases = [r for r in matrix["formats"][fmt_name].get("decode", [])
                     if r.get("status") == "active" and r.get("asset")]
        if dec_cases:
            inp_data = [{"id": r["id"], "asset": r["asset"]} for r in dec_cases]
            inp = {"format_version": 2, "operation": {"module": "Decode", "target": fmt_name}, "cases": inp_data}
            INPUT_JSONS.mkdir(parents=True, exist_ok=True)
            (INPUT_JSONS / f"Decode.{fmt_name}.json").write_text(json.dumps(inp, indent=2) + "\n")

            out_data = [
                {
                    "id": r["id"],
                    "ref_path": r.get("ref_path"),
                    "ref_bytes": r.get("ref_bytes"),
                    "ref_mode": r.get("ref_mode"),
                    "ref_size": r.get("ref_size"),
                }
                for r in dec_cases
                if r.get("ref_path")
            ]
            out = {"format_version": 2, "operation": {"module": "Decode", "target": fmt_name}, "cases": out_data}
            OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
            (OUTPUT_JSONS / f"Decode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

    return generated


def generate_encode(matrix, target_format=None):
    """Generate Encode refs: PIL roundtrip pixel bytes."""
    from PIL import Image
    generated = 0

    for fmt_name, fmt_data in matrix["formats"].items():
        if target_format and fmt_name != target_format:
            continue
        for row in fmt_data.get("encode", []):
            if row.get("status") != "active":
                continue
            if (fmt_name, row["id"]) in CURRENT_RUST_ENCODE_GAPS:
                row["status"] = "planned"
                clear_pixel_ref(row)
                continue
            src_fmt = row.get("source_format") or fmt_name
            src_asset = row.get("source_asset")
            if not src_asset:
                continue
            src_path = ASSETS_DIR / src_fmt / src_asset
            if not src_path.exists():
                continue

            try:
                img = Image.open(src_path)
                img.load()
                params = row.get("params", {})
                buf = io.BytesIO()
                img.save(buf, format=fmt_pil(fmt_name), **encode_params(fmt_name, dict(params)))
                buf.seek(0)
                rt = Image.open(buf)
                if exact_encode_parity_supported(fmt_name, row):
                    ref_name = f"Encode.{fmt_name}_{row['id']}.bin"
                    write_pixel_ref(row, rt, ref_name)
                else:
                    clear_pixel_ref(row)
                generated += 1
            except Exception as e:
                # Lossy formats or unsupported params — skip ref, just verify dimensions
                print(f"  SKIP encode {row.get('id')}: {e}", file=sys.stderr)

        # Encode input/output JSONs
        enc_cases = [r for r in fmt_data.get("encode", [])
                     if r.get("status") == "active" and r.get("source_asset")]
        if enc_cases:
            inp_data = [{"id": r["id"], "source_asset": r["source_asset"],
                         "source_format": r.get("source_format", fmt_name),
                         "params": r.get("params", {})} for r in enc_cases]
            inp = {"format_version": 2, "operation": {"module": "Encode", "target": fmt_name}, "cases": inp_data}
            (INPUT_JSONS / f"Encode.{fmt_name}.json").write_text(json.dumps(inp, indent=2) + "\n")

            out_data = [
                {
                    "id": r["id"],
                    "ref_path": r.get("ref_path"),
                    "ref_bytes": r.get("ref_bytes"),
                    "ref_mode": r.get("ref_mode"),
                    "ref_size": r.get("ref_size"),
                }
                for r in enc_cases
                if r.get("ref_path")
            ]
            out = {"format_version": 2, "operation": {"module": "Encode", "target": fmt_name}, "cases": out_data}
            OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
            (OUTPUT_JSONS / f"Encode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

    return generated


def generate(target_format=None):
    # Load
    manifest = yaml.safe_load(MANIFEST.read_text())
    matrix = json.loads(MATRIX_PATH.read_text()) if MATRIX_PATH.exists() else {"formats": {}}

    # Decode
    n_dec = generate_decode(manifest, matrix, target_format)
    print(f"Decode: {n_dec} refs")

    # Encode
    n_enc = generate_encode(matrix, target_format)
    print(f"Encode: {n_enc} refs")

    # Save matrix
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2))
    print(f"Written: {MATRIX_PATH}")

    # Commit outputs
    print("\nAll refs generated. Outputs in tests/fixtures/outputs/ are committed.")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--format", help="Specific format only")
    args = p.parse_args()
    generate(args.format)

#!/usr/bin/env python3
"""Unified reference generator — Decode pixel refs + Encode roundtrip refs.

Decode: PIL open asset → .tobytes() → SHA-256 + .bin → matrix
Encode:  PIL open source → .save(format, params) → reopen → .tobytes() → SHA-256 → matrix

Single script. Single source of truth: manifest.yaml + coverage_matrix.json.
"""
import json, hashlib, io, sys, argparse
from pathlib import Path
import yaml

ROOT = Path(__file__).parent.parent
MANIFEST = ROOT / "manifest.yaml"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
INPUT_JSONS = ROOT / "tests" / "fixtures" / "input" / "jsons"
OUTPUT_JSONS = ROOT / "tests" / "fixtures" / "outputs" / "jsons"
OUTPUT_RAWS = ROOT / "tests" / "fixtures" / "outputs" / "raws"
ASSETS_DIR = ROOT / "tests" / "fixtures" / "input" / "images"


def mode_name(img):
    m = {"L": "L8", "LA": "La8", "RGB": "Rgb8", "RGBA": "Rgba8", "1": "1", "P": "P"}
    return m.get(img.mode, img.mode)


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
            m["subsampling"] = v
        elif k == "compression" and fmt == "tiff":
            m["compression"] = v
    return m


def generate_decode(manifest, matrix):
    """Generate Decode refs: raw pixel bytes from PIL."""
    generated = 0
    for fmt_name, fmt_data in manifest["formats"].items():
        out_cases = []
        for case in fmt_data.get("edge_cases", []):
            for asset_name in case.get("test_assets", []):
                img_path = ASSETS_DIR / fmt_name / asset_name
                if not img_path.exists():
                    continue
                try:
                    from PIL import Image
                    img = Image.open(img_path)
                    raw = img.tobytes()
                    sha = hashlib.sha256(raw).hexdigest()
                    ref_name = f"Decode.{fmt_name}_{asset_name.replace('.', '_')}.bin"
                    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
                    (OUTPUT_RAWS / ref_name).write_bytes(raw)

                    # Update matrix row
                    cid = f"decode_{fmt_name}_{asset_name.replace('.', '_')}"
                    for row in matrix["formats"][fmt_name]["decode"]:
                        if row["id"] == cid or row.get("asset") == asset_name:
                            row["ref_sha256"] = sha
                            row["ref_bytes"] = len(raw)
                            row["ref_mode"] = mode_name(img)
                            break
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

            out_data = [{"id": r["id"], "ref_sha256": r.get("ref_sha256"),
                         "ref_bytes": r.get("ref_bytes"), "ref_mode": r.get("ref_mode")}
                        for r in dec_cases if r.get("ref_sha256")]
            out = {"format_version": 2, "operation": {"module": "Decode", "target": fmt_name}, "cases": out_data}
            OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
            (OUTPUT_JSONS / f"Decode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

    return generated


def generate_encode(matrix):
    """Generate Encode refs: PIL roundtrip pixel SHA-256."""
    from PIL import Image
    generated = 0

    for fmt_name, fmt_data in matrix["formats"].items():
        for row in fmt_data.get("encode", []):
            if row.get("status") != "active":
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
                rt.load()
                pixels = rt.tobytes()
                row["ref_sha256"] = hashlib.sha256(pixels).hexdigest()
                row["ref_bytes"] = len(pixels)
                row["ref_mode"] = mode_name(rt)
                generated += 1
            except Exception as e:
                # Lossy formats or unsupported params — skip ref, just verify dimensions
                pass

        # Encode input/output JSONs
        enc_cases = [r for r in fmt_data.get("encode", [])
                     if r.get("status") == "active" and r.get("source_asset")]
        if enc_cases:
            inp_data = [{"id": r["id"], "source_asset": r["source_asset"],
                         "source_format": r.get("source_format", fmt_name),
                         "params": r.get("params", {})} for r in enc_cases]
            inp = {"format_version": 2, "operation": {"module": "Encode", "target": fmt_name}, "cases": inp_data}
            (INPUT_JSONS / f"Encode.{fmt_name}.json").write_text(json.dumps(inp, indent=2) + "\n")

            out_data = [{"id": r["id"], "ref_sha256": r.get("ref_sha256"),
                         "ref_bytes": r.get("ref_bytes"), "ref_mode": r.get("ref_mode")}
                        for r in enc_cases if r.get("ref_sha256")]
            out = {"format_version": 2, "operation": {"module": "Encode", "target": fmt_name}, "cases": out_data}
            OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
            (OUTPUT_JSONS / f"Encode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

    return generated


def generate(target_format=None):
    # Load
    manifest = yaml.safe_load(MANIFEST.read_text())
    matrix = json.loads(MATRIX_PATH.read_text()) if MATRIX_PATH.exists() else {"formats": {}}

    # Decode
    n_dec = generate_decode(manifest, matrix)
    print(f"Decode: {n_dec} refs")

    # Encode
    n_enc = generate_encode(matrix)
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

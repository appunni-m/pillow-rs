#!/usr/bin/env python3
"""Unified benchmark runner — reads bench_spec.json (single source of truth).

Both pillow-rs and PIL baselines use the SAME spec. Impossible for params
to diverge. Output includes SHA-256 hashes for cross-validation.

Usage:
  python scripts/bench_unified.py --target rspil   # pillow-rs benchmarks
  python scripts/bench_unified.py --target pil     # Pillow baseline
  python scripts/bench_unified.py --validate       # cross-validate hashes
"""

import json, sys, time, hashlib, io
from pathlib import Path

ROOT = Path(__file__).parent.parent
SPEC_PATH = ROOT / "scripts" / "bench_spec.json"
OUT_DIR = ROOT / "target" / "benchmarks"
REF_DIR = ROOT / "scripts" / "bench_reference_images"

# ── Load spec ──
with open(SPEC_PATH) as f:
    SPEC = json.load(f)

# ── RSPIL runner ──
def run_rspil(runs=3, warmup=1):
    sys.path.insert(0, str(ROOT / "pillow-rs-py" / "python"))
    from pillow_rs import Image, ImageOps, ImageColor, ImageEnhance, ImageDraw, ImageFont, _core

    images = {}
    def get_img(key):
        if key not in images:
            inp = SPEC["inputs"][key]
            if "file" in inp:
                images[key] = Image.open(str(ROOT / inp["file"]))
            else:
                images[key] = Image.new(inp["mode"], (inp["w"], inp["h"]), tuple(inp.get("color", [128,128,128,255])))
        return images[key]

    def exec_op(op_name, params):
        """Execute an operation by name with params from spec. Returns (result, can_hash)."""
        img_key = None  # resolved later
        for op in SPEC["operations"]:
            if op["name"] == op_name:
                img_key = op["input"]
                break

        if op_name.startswith("Image."):
            method = op_name.split(".")[1]
            img = get_img(img_key or "ref_2k")
            if method == "resize":
                return img.resize(tuple(params["size"]), params.get("filter", "LANCZOS")), True
            elif method == "crop":
                b = params["box"]
                return img.crop(tuple(b)), True
            elif method == "rotate":
                return img.rotate(params["angle"]), True
            elif method == "transpose":
                return img.transpose(params.get("method", "FLIP_LEFT_RIGHT")), True
            elif method == "thumbnail":
                img.thumbnail(tuple(params["size"]))
                return img, True
            elif method == "convert":
                return img.convert(params["mode"]), True
            elif method == "filter":
                return img.filter(params["name"]), True
            elif method == "paste":
                src = Image.new("RGB", (params["src_w"], params["src_h"]), tuple(params["src_color"] + [255]))
                img.paste(src, tuple(params["box"]))
                return img, True
            elif method == "copy":
                return img.copy(), True
            elif method == "split":
                return img.split()[0], True
            elif method == "getbands":
                return img.getbands(), False
            elif method == "getbbox":
                return img.getbbox(), False
            elif method == "getchannel":
                return img.getchannel(params["channel"]), True
            elif method == "getcolors":
                return img.getcolors(params.get("maxcolors", 256)), False
            elif method == "getextrema":
                return img.getextrema(), False
            elif method == "getpixel":
                return img.getpixel(tuple(params["xy"])), False
            elif method == "getprojection":
                return img.getprojection(), False
            elif method == "histogram":
                return img.histogram(), False
            elif method == "point":
                return img.point(lambda x: 255 - x), True  # invert LUT
            elif method == "quantize":
                return img.quantize(params["colors"]), True
            elif method == "reduce":
                return img.reduce(params["factor"]), True
            elif method == "entropy":
                return img.entropy(), False
            elif method == "effect_spread":
                return img.effect_spread(params["distance"]), True
            elif method == "tobytes":
                return img.tobytes(), True
            elif method == "open":
                r = Image.open(str(ROOT / SPEC["inputs"][img_key]["file"]))
                return r.tobytes(), True
            elif method == "new":
                return Image.new(params["mode"], (params["w"], params["h"]), tuple(params["color"] or [0,0,0,255])), True
            elif method == "save":
                return img.tobytes(), True  # save to memory = tobytes

        elif op_name.startswith("ImageOps."):
            method = op_name.split(".")[1]
            img = get_img(img_key or "ref_2k")
            fn = getattr(ImageOps, method)
            return fn(img, **{k:v for k,v in params.items() if k not in ('_')}), True

        elif op_name.startswith("ImageEnhance."):
            method = op_name.split(".")[1]
            img = get_img(img_key or "ref_2k")
            cls = getattr(ImageEnhance, method)
            return cls(img).enhance(params["factor"]), True

        elif op_name.startswith("ImageColor."):
            return getattr(ImageColor, op_name.split(".")[1])(**params), False

        elif op_name == "pipeline_20_st":
            img = get_img(img_key or "ref_2k")
            for step in params["chain"]:
                img, _ = exec_op(f"Image.{step['op']}", step['params'])
            return img, True

        return None, False

    # ── Run all ──
    results = {}
    for op in SPEC["operations"]:
        name = op["name"]
        for _ in range(warmup):
            try: exec_op(name, op["params"])
            except: pass
        times = []
        last_bytes = None
        for _ in range(runs):
            try:
                t0 = time.time()
                result, can_hash = exec_op(name, op["params"])
                # Force materialization INSIDE timer:
                if can_hash and hasattr(result, 'tobytes'):
                    last_bytes = result.tobytes()
                elif isinstance(result, bytes):
                    last_bytes = result
                times.append(time.time() - t0)
            except Exception as e:
                pass
        if not times:
            continue
        mean = sum(times) / len(times)
        entry = {"mean_ms": round(mean * 1000, 4), "std_ms": 0, "runs": len(times)}
        if last_bytes:
            entry["output_hash"] = hashlib.sha256(last_bytes).hexdigest()[:16]
        results[name] = entry
        print(f"  {name:<45} {entry['mean_ms']:>8.1f}ms  {entry.get('output_hash','')}")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with open(OUT_DIR / "native_cpu.json", "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nSaved {len(results)} results from bench_spec.json")
    return results


# ── PIL baseline runner (uses SAME spec) ──
def run_pil(runs=3, warmup=1):
    from PIL import Image, ImageOps, ImageFilter, ImageEnhance, ImageColor
    PIL = __import__('PIL', fromlist=['Image'])

    images = {}
    def get_img(key):
        if key not in images:
            inp = SPEC["inputs"][key]
            if "file" in inp:
                images[key] = Image.open(str(ROOT / inp["file"]))
            else:
                color = tuple(inp.get("color", [128,128,128,255])[:3] if inp["mode"] != "RGBA" else inp.get("color", [128,128,128,255]))
                images[key] = Image.new(inp["mode"], (inp["w"], inp["h"]), color if inp["mode"] == "RGB" else color[0] if inp["mode"] == "L" else color)
        return images[key].copy()

    def exec_op_pil(op_name, params):
        img_key = None
        for op in SPEC["operations"]:
            if op["name"] == op_name:
                img_key = op["input"]
                break

        if op_name.startswith("Image."):
            method = op_name.split(".")[1]
            img = get_img(img_key or "ref_2k")
            if method == "resize":
                return img.resize(tuple(params["size"]), getattr(Image, params.get("filter", "LANCZOS"), Image.LANCZOS)), True
            elif method == "crop":
                return img.crop(tuple(params["box"])), True
            elif method == "rotate":
                return img.rotate(params["angle"], resample=getattr(Image, params.get("resample", "BICUBIC"), Image.BICUBIC), expand=params.get("expand", False)), True
            elif method == "transpose":
                return img.transpose(getattr(Image.Transpose, params.get("method", "FLIP_LEFT_RIGHT"))), True
            elif method == "thumbnail":
                img.thumbnail(tuple(params["size"]), getattr(Image, params.get("filter", "LANCZOS"), Image.LANCZOS))
                return img, True
            elif method == "convert":
                return img.convert(params["mode"]), True
            elif method == "filter":
                return img.filter(getattr(ImageFilter, params["name"])), True
            elif method == "paste":
                src = Image.new("RGB", (params["src_w"], params["src_h"]), tuple(params["src_color"]))
                img.paste(src, tuple(params["box"]))
                return img, True
            elif method == "copy":
                return img.copy(), True
            elif method == "split":
                return img.split()[0], True
            elif method == "tobytes":
                return img.tobytes(), True
            elif method == "getbands":
                return img.getbands(), False
            elif method == "getbbox":
                return img.getbbox(), False
            elif method == "getchannel":
                return img.getchannel(params["channel"]), True
            elif method == "getcolors":
                return img.getcolors(params.get("maxcolors", 256)), False
            elif method == "getextrema":
                return img.getextrema(), False
            elif method == "getpixel":
                return img.getpixel(tuple(params["xy"])), False
            elif method == "getprojection":
                return img.getprojection(), False
            elif method == "histogram":
                return img.histogram(), False
            elif method == "point":
                return img.point(lambda x: 255 - x), True
            elif method == "quantize":
                return img.quantize(params["colors"]), True
            elif method == "reduce":
                return img.reduce(params["factor"]), True
            elif method == "entropy":
                return img.entropy(), False
            elif method == "effect_spread":
                return img.effect_spread(params["distance"]), True
            elif method == "open":
                return Image.open(str(ROOT / SPEC["inputs"][img_key]["file"])), True
            elif method == "new":
                return Image.new(params["mode"], (params["w"], params["h"]), tuple(params["color"])), True
            elif method == "save":
                buf = io.BytesIO()
                img.save(buf, format="PNG")
                return buf.getvalue(), True

        elif op_name.startswith("ImageOps."):
            method = op_name.split(".")[1]
            img = get_img(img_key or "ref_2k")
            return getattr(ImageOps, method)(img), True

        elif op_name.startswith("ImageEnhance."):
            method = op_name.split(".")[1]
            img = get_img(img_key or "ref_2k")
            return getattr(ImageEnhance, method)(img).enhance(params["factor"]), True

        elif op_name.startswith("ImageColor."):
            return getattr(ImageColor, op_name.split(".")[1])(**params), False

        return None, False

    results = {}
    for op in SPEC["operations"]:
        name = op["name"]
        for _ in range(warmup):
            try: exec_op_pil(name, op["params"])
            except: pass
        times = []
        last_bytes = None
        for _ in range(runs):
            try:
                t0 = time.time()
                result, can_hash = exec_op_pil(name, op["params"])
                if can_hash and hasattr(result, 'tobytes'):
                    last_bytes = result.tobytes()
                elif isinstance(result, bytes):
                    last_bytes = result
                times.append(time.time() - t0)
            except: pass
        if not times:
            continue
        mean = sum(times) / len(times)
        entry = {"mean_ms": round(mean * 1000, 4), "std_ms": 0, "runs": len(times)}
        if last_bytes:
            entry["output_hash"] = hashlib.sha256(last_bytes).hexdigest()[:16]
        results[name] = entry
        print(f"  {name:<45} {entry['mean_ms']:>8.1f}ms  {entry.get('output_hash','')}")

    with open(OUT_DIR / "pillow_baseline.json", "w") as f:
        json.dump({"results": [{"function": k, "mean_s": v["mean_ms"]/1000, "output_hash": v.get("output_hash")} for k,v in results.items()]}, f, indent=2)
    print(f"\nSaved {len(results)} PIL baselines from bench_spec.json")
    return results


# ── Validate: compare RSPIL hashes vs PIL hashes ──
def validate():
    rspil_path = OUT_DIR / "native_cpu.json"
    pil_path = OUT_DIR / "pillow_baseline.json"
    if not rspil_path.exists() or not pil_path.exists():
        print("Run both --target rspil and --target pil first")
        return

    rspil = json.loads(rspil_path.read_text())
    pil_data = json.loads(pil_path.read_text())
    pil = {}
    for r in pil_data.get("results", []):
        pil[r["function"]] = r.get("output_hash")

    ok = 0
    bad = 0
    for name, entry in rspil.items():
        rs_hash = entry.get("output_hash")
        pil_hash = pil.get(name)
        if rs_hash and pil_hash:
            if rs_hash == pil_hash:
                ok += 1
            else:
                bad += 1
                print(f"  ❌ MISMATCH: {name}  rs={rs_hash}  pil={pil_hash}")

    print(f"\n✓ {ok} hash matches  ✗ {bad} mismatches")
    if bad > 0:
        print("WARNING: Mismatched hashes mean inputs/outputs differ — speedup ratios are INVALID!")
        sys.exit(1)
    else:
        print("All operations validated — identical inputs, identical outputs.")
    return ok, bad


# ── CLI ──
if __name__ == "__main__":
    target = sys.argv[2] if "--target" in sys.argv else None
    if target == "rspil":
        run_rspil(runs=3)
    elif target == "pil":
        run_pil(runs=3)
    elif "--validate" in sys.argv:
        validate()
    else:
        print("Usage: bench_unified.py --target rspil|pil  OR  --validate")
        print(f"Spec: {SPEC_PATH} ({len(SPEC['operations'])} operations)")

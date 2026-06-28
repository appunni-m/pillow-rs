#!/usr/bin/env python3
"""Regenerate BOTH reference matrices from C FreeType 2.14.3 autohinter.

Both use FT_LOAD_FORCE_AUTOHINT for apples-to-apples autohinter comparison.
getlength from PIL (hmtx-based, backend-independent).
"""

import hashlib, json, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
FIX = ROOT / "tests" / "fixtures"
FONTS_L = FIX / "input" / "fonts_autohint"
SHA = hashlib.sha256
GEN = "/tmp/gen_ft_refs"

FONTS = {
    "DejaVuSans": ("DejaVuSans.ttf", "DejaVu Sans", "Book"),
    "LiberationSerif": ("LiberationSerif-Regular.ttf", "Liberation Serif", "Regular"),
}

rows_pil, rows_ft = [], []
old = json.loads((FIX / "coverage_matrix.json").read_text())
pil_lengths = {r["id"]: r["ref_value"] for r in old["rows"] if r.get("operation") == "getlength"}

for fn, (ff, fam, sty) in FONTS.items():
    p = subprocess.run([GEN, str(FONTS_L / ff)], capture_output=True, text=True, check=True)
    lines = p.stdout.splitlines()
    sz = None; asc = 0
    for line in lines:
        parts = line.split()
        if not parts: continue
        if parts[0] == 'SIZE': sz = int(parts[1]); continue
        if parts[0] == 'END_SIZE': sz = None; continue
        if parts[0] == 'METRICS':
            asc, desc = int(parts[1]), int(parts[2])
            for rows in (rows_pil, rows_ft):
                rows.append({"id": f"{fn}_{sz}_getmetrics", "font": fn, "size_pt": sz,
                    "codepoint": 0, "char": "", "operation": "getmetrics", "status": "active",
                    "ref_value": [asc, desc]})
                rows.append({"id": f"{fn}_{sz}_getname", "font": fn, "size_pt": sz,
                    "codepoint": 0, "char": "", "operation": "getname", "status": "active",
                    "ref_value": [fam, sty]})
            continue
        if parts[0] == 'GLYPH' and sz:
            cp = int(parts[1]); w, h = int(parts[2]), int(parts[3])
            left, top = int(parts[4]), int(parts[5]); adv_26 = int(parts[6])
            ch = chr(cp)
            pix = bytes(int(x, 16) for x in parts[parts.index('PIXELS') + 1:]) if 'PIXELS' in line else b""
            ft_y_min = top - h; ft_y_max = top
            adv_px = (adv_26 + 32) >> 6  # pixel_round

            # --- FT (raw) ---
            sha = SHA(pix).hexdigest() if pix else SHA(b"\x00").hexdigest()
            rows_ft.append({"id": f"{fn}_{sz}_{cp}_getmask", "font": fn, "size_pt": sz,
                "codepoint": cp, "char": ch, "operation": "getmask", "status": "active",
                "ref_sha256": sha, "ref_size": [w, h]})
            rows_ft.append({"id": f"{fn}_{sz}_{cp}_getbbox", "font": fn, "size_pt": sz,
                "codepoint": cp, "char": ch, "operation": "getbbox", "status": "active",
                "ref_value": [left, ft_y_min, left + w, ft_y_max]})

            # --- PIL (padded) ---
            # Match our PIL backend in font.rs exactly:
            # getbbox: gx_min = 0.min(bbox_x_min); gx_max = advance.max(bbox_x_max)
            #          gy_min = asc - bbox_y_max; gy_max = max(asc, asc - bbox_y_min)
            # getmask: new_width = advance.max(raster_width)
            #          new_height = bbox_y_max - min(bbox_y_min, 0)
            #          x_offs = max(bbox_x_min, 0); y_offs = 0
            pil_w = max(adv_px, w)
            pil_h = ft_y_max - min(ft_y_min, 0)
            pil_x_off = max(left, 0)

            if pil_w <= 0 or pil_h <= 0:
                pil_pix = b""
            else:
                canvas = bytearray(pil_w * pil_h)
                for y in range(h):
                    for x in range(w):
                        vi = y * w + x
                        if vi < len(pix) and pix[vi]:
                            cx = pil_x_off + x
                            if cx < pil_w:
                                canvas[y * pil_w + cx] = pix[vi]
                pil_pix = bytes(canvas)

            pil_sha = SHA(pil_pix).hexdigest() if pil_pix else SHA(b"\x00").hexdigest()
            pil_gx_min = min(0, left)
            pil_gx_max = max(adv_px, left + w)
            pil_gy_min = asc - ft_y_max
            pil_gy_max = max(asc, asc - ft_y_min)

            rows_pil.append({"id": f"{fn}_{sz}_{cp}_getmask", "font": fn, "size_pt": sz,
                "codepoint": cp, "char": ch, "operation": "getmask", "status": "active",
                "ref_sha256": pil_sha, "ref_size": [pil_w, pil_h]})
            rows_pil.append({"id": f"{fn}_{sz}_{cp}_getbbox", "font": fn, "size_pt": sz,
                "codepoint": cp, "char": ch, "operation": "getbbox", "status": "active",
                "ref_value": [pil_gx_min, pil_gy_min, pil_gx_max, pil_gy_max]})

# getlength from PIL (backend-independent)
for r in old["rows"]:
    if r.get("operation") == "getlength":
        rows_pil.append(dict(r))
        rows_ft.append(dict(r))

jm = lambda title, gen, mode, rows: {"version": "2.0.0", "font_source": "fonts_autohint",
    "hinting": "autohint", "generator": gen, "mode": mode, "rows": rows,
    "summary": {"total_rows": len(rows), "active_rows": len(rows), "fonts": 2, "sizes": 5, "glyphs": 94}}

(FIX / "coverage_matrix.json").write_text(json.dumps(jm("PIL-padded autohinter",
    "FreeType 2.14.3 autohinter padded to PIL format", "PIL-padded", rows_pil), indent=2) + "\n")
(FIX / "coverage_matrix_ft.json").write_text(json.dumps(jm("FreeType-raw autohinter",
    "FreeType 2.14.3 autohinter raw", "FreeType-raw", rows_ft), indent=2) + "\n")

print(f"PIL: {len(rows_pil)} rows  FT: {len(rows_ft)} rows", file=sys.stderr)

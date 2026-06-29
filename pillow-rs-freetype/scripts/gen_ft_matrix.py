#!/usr/bin/env python3
"""Generate the FreeType raw reference matrix using the locally-built
FreeType 2.14.3 binary (/tmp/gen_ft_refs).

Requires: gen_ft_refs.c compiled with:
  gcc -o /tmp/gen_ft_refs scripts/gen_ft_refs.c \
    -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype \
    -Wl,-rpath,$HOME/.local/lib

Also requires: FreeType 2.14.3 built via scripts/build_ft.sh

Note: getlength uses hmtx-based computation (backend-independent), so
values are taken from the PIL matrix which is the reference for advance widths.
"""

import hashlib
import json
import subprocess
from pathlib import Path
import sys

ROOT = Path(__file__).parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
INPUT_FONTS = FIXTURES / "input" / "fonts_autohint"
MATRIX_PATH = FIXTURES / "coverage_matrix_ft.json"

FONTS = {
    "DejaVuMathTeXGyre": ("DejaVuMathTeXGyre.ttf", "DejaVu Math TeX Gyre", "Regular"),
    "DejaVuSans-Bold": ("DejaVuSans-Bold.ttf", "DejaVu Sans", "Bold"),
    "DejaVuSans-BoldOblique": ("DejaVuSans-BoldOblique.ttf", "DejaVu Sans", "Bold Oblique"),
    "DejaVuSans": ("DejaVuSans.ttf", "DejaVu Sans", "Book"),
    "DejaVuSans-Oblique": ("DejaVuSans-Oblique.ttf", "DejaVu Sans", "Oblique"),
    "DejaVuSansCondensed-Bold": ("DejaVuSansCondensed-Bold.ttf", "DejaVu Sans Condensed", "Bold"),
    "DejaVuSansCondensed-BoldOblique": ("DejaVuSansCondensed-BoldOblique.ttf", "DejaVu Sans Condensed", "Bold Oblique"),
    "DejaVuSansCondensed": ("DejaVuSansCondensed.ttf", "DejaVu Sans Condensed", "Book"),
    "DejaVuSansCondensed-Oblique": ("DejaVuSansCondensed-Oblique.ttf", "DejaVu Sans Condensed", "Oblique"),
    "DejaVuSans-ExtraLight": ("DejaVuSans-ExtraLight.ttf", "DejaVu Sans Light", "ExtraLight"),
    "DejaVuSansMono-Bold": ("DejaVuSansMono-Bold.ttf", "DejaVu Sans Mono", "Bold"),
    "DejaVuSansMono-BoldOblique": ("DejaVuSansMono-BoldOblique.ttf", "DejaVu Sans Mono", "Bold Oblique"),
    "DejaVuSansMono": ("DejaVuSansMono.ttf", "DejaVu Sans Mono", "Book"),
    "DejaVuSansMono-Oblique": ("DejaVuSansMono-Oblique.ttf", "DejaVu Sans Mono", "Oblique"),
    "DejaVuSerif-Bold": ("DejaVuSerif-Bold.ttf", "DejaVu Serif", "Bold"),
    "DejaVuSerif-BoldItalic": ("DejaVuSerif-BoldItalic.ttf", "DejaVu Serif", "Bold Italic"),
    "DejaVuSerif": ("DejaVuSerif.ttf", "DejaVu Serif", "Book"),
    "DejaVuSerif-Italic": ("DejaVuSerif-Italic.ttf", "DejaVu Serif", "Italic"),
    "DejaVuSerifCondensed-Bold": ("DejaVuSerifCondensed-Bold.ttf", "DejaVu Serif Condensed", "Bold"),
    "DejaVuSerifCondensed-BoldItalic": ("DejaVuSerifCondensed-BoldItalic.ttf", "DejaVu Serif Condensed", "Bold Italic"),
    "DejaVuSerifCondensed": ("DejaVuSerifCondensed.ttf", "DejaVu Serif Condensed", "Book"),
    "DejaVuSerifCondensed-Italic": ("DejaVuSerifCondensed-Italic.ttf", "DejaVu Serif Condensed", "Italic"),
    "LiberationMono-Bold": ("LiberationMono-Bold.ttf", "Liberation Mono", "Bold"),
    "LiberationMono-BoldItalic": ("LiberationMono-BoldItalic.ttf", "Liberation Mono", "Bold Italic"),
    "LiberationMono-Italic": ("LiberationMono-Italic.ttf", "Liberation Mono", "Italic"),
    "LiberationMono-Regular": ("LiberationMono-Regular.ttf", "Liberation Mono", "Regular"),
    "LiberationSans-Bold": ("LiberationSans-Bold.ttf", "Liberation Sans", "Bold"),
    "LiberationSans-BoldItalic": ("LiberationSans-BoldItalic.ttf", "Liberation Sans", "Bold Italic"),
    "LiberationSans-Italic": ("LiberationSans-Italic.ttf", "Liberation Sans", "Italic"),
    "LiberationSans-Regular": ("LiberationSans-Regular.ttf", "Liberation Sans", "Regular"),
    "LiberationSansNarrow-Bold": ("LiberationSansNarrow-Bold.ttf", "Liberation Sans Narrow", "Bold"),
    "LiberationSansNarrow-BoldItalic": ("LiberationSansNarrow-BoldItalic.ttf", "Liberation Sans Narrow", "Bold Italic"),
    "LiberationSansNarrow-Italic": ("LiberationSansNarrow-Italic.ttf", "Liberation Sans Narrow", "Italic"),
    "LiberationSansNarrow-Regular": ("LiberationSansNarrow-Regular.ttf", "Liberation Sans Narrow", "Regular"),
    "LiberationSerif-Bold": ("LiberationSerif-Bold.ttf", "Liberation Serif", "Bold"),
    "LiberationSerif-BoldItalic": ("LiberationSerif-BoldItalic.ttf", "Liberation Serif", "Bold Italic"),
    "LiberationSerif-Italic": ("LiberationSerif-Italic.ttf", "Liberation Serif", "Italic"),
    "LiberationSerif-Regular": ("LiberationSerif-Regular.ttf", "Liberation Serif", "Regular"),
    "NotoMono-Regular": ("NotoMono-Regular.ttf", "Noto Mono", "Regular"),
    "NotoSans-Bold": ("NotoSans-Bold.ttf", "Noto Sans", "Bold"),
    "NotoSans-BoldItalic": ("NotoSans-BoldItalic.ttf", "Noto Sans", "Bold Italic"),
    "NotoSans-Italic": ("NotoSans-Italic.ttf", "Noto Sans", "Italic"),
    "NotoSans-Regular": ("NotoSans-Regular.ttf", "Noto Sans", "Regular"),
    "NotoSansDisplay-Bold": ("NotoSansDisplay-Bold.ttf", "Noto Sans Display", "Bold"),
    "NotoSansDisplay-BoldItalic": ("NotoSansDisplay-BoldItalic.ttf", "Noto Sans Display", "Bold Italic"),
    "NotoSansDisplay-Italic": ("NotoSansDisplay-Italic.ttf", "Noto Sans Display", "Italic"),
    "NotoSansDisplay-Regular": ("NotoSansDisplay-Regular.ttf", "Noto Sans Display", "Regular"),
    "NotoSansMath-Regular": ("NotoSansMath-Regular.ttf", "Noto Sans Math", "Regular"),
    "NotoSansMono-Bold": ("NotoSansMono-Bold.ttf", "Noto Sans Mono", "Bold"),
    "NotoSansMono-Regular": ("NotoSansMono-Regular.ttf", "Noto Sans Mono", "Regular"),
    "NotoSerif-Bold": ("NotoSerif-Bold.ttf", "Noto Serif", "Bold"),
    "NotoSerif-BoldItalic": ("NotoSerif-BoldItalic.ttf", "Noto Serif", "Bold Italic"),
    "NotoSerif-Italic": ("NotoSerif-Italic.ttf", "Noto Serif", "Italic"),
    "NotoSerif-Regular": ("NotoSerif-Regular.ttf", "Noto Serif", "Regular"),
    "NotoSerifDisplay-Bold": ("NotoSerifDisplay-Bold.ttf", "Noto Serif Display", "Bold"),
    "NotoSerifDisplay-BoldItalic": ("NotoSerifDisplay-BoldItalic.ttf", "Noto Serif Display", "Bold Italic"),
    "NotoSerifDisplay-Italic": ("NotoSerifDisplay-Italic.ttf", "Noto Serif Display", "Italic"),
    "NotoSerifDisplay-Regular": ("NotoSerifDisplay-Regular.ttf", "Noto Serif Display", "Regular"),
    "Ubuntu-Italic[wdth,wght]": ("Ubuntu-Italic[wdth,wght].ttf", "Ubuntu", "Italic"),
    "Ubuntu[wdth,wght]": ("Ubuntu[wdth,wght].ttf", "Ubuntu", "Regular"),
    "UbuntuMono-Italic[wght]": ("UbuntuMono-Italic[wght].ttf", "Ubuntu Mono", "Italic"),
    "UbuntuMono[wght]": ("UbuntuMono[wght].ttf", "Ubuntu Mono", "Regular"),
    "UbuntuSans-Italic[wdth,wght]": ("UbuntuSans-Italic[wdth,wght].ttf", "Ubuntu Sans", "Italic"),
    "UbuntuSans[wdth,wght]": ("UbuntuSans[wdth,wght].ttf", "Ubuntu Sans", "Regular"),
    "UbuntuSansMono-Italic[wght]": ("UbuntuSansMono-Italic[wght].ttf", "Ubuntu Sans Mono", "Italic"),
    "UbuntuSansMono[wght]": ("UbuntuSansMono[wght].ttf", "Ubuntu Sans Mono", "Regular"),
}
GEN_BINARY = "/tmp/gen_ft_refs"
SHA = hashlib.sha256

if not Path(GEN_BINARY).exists():
    print("ERROR: Build gen_ft_refs first:", file=sys.stderr)
    print(f"  gcc -o {GEN_BINARY} scripts/gen_ft_refs.c \\", file=sys.stderr)
    print("    -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype \\", file=sys.stderr)
    print("    -Wl,-rpath,$HOME/.local/lib", file=sys.stderr)
    sys.exit(1)

# Build gen_ft_refs if needed (it's just a C file)
if not Path(GEN_BINARY).exists():
    subprocess.run(["gcc", "-o", GEN_BINARY, str(ROOT / "scripts" / "gen_ft_refs.c"),
                    "-I$HOME/.local/include/freetype2", "-L$HOME/.local/lib", "-lfreetype",
                    "-Wl,-rpath,$HOME/.local/lib"], check=True)

rows = []
for fn, (ff, fam, sty) in FONTS.items():
    font_path = INPUT_FONTS / ff
    proc = subprocess.run([GEN_BINARY, str(font_path)],
                         capture_output=True, text=True, check=True)
    lines = proc.stdout.splitlines()
    
    sz = None
    for line in lines:
        parts = line.split()
        if not parts:
            continue
        if parts[0] == 'SIZE':
            sz = int(parts[1])
            continue
        if parts[0] == 'METRICS':
            asc, desc = int(parts[1]), int(parts[2])
            rows.append({"id": f"{fn}_{sz}_getmetrics", "font": fn, "size_pt": sz,
                        "codepoint": 0, "char": "", "operation": "getmetrics", "status": "active",
                        "ref_value": [asc, desc]})
            rows.append({"id": f"{fn}_{sz}_getname", "font": fn, "size_pt": sz,
                        "codepoint": 0, "char": "", "operation": "getname", "status": "active",
                        "ref_value": [fam, sty]})
            # getlength: use PIL reference (backend-independent hmtx-based advance)
            continue
        if parts[0] == 'END_SIZE':
            sz = None
            continue
        if parts[0] == 'GLYPH' and sz:
            cp = int(parts[1])
            w, h = int(parts[2]), int(parts[3])
            left, top = int(parts[4]), int(parts[5])
            ch = chr(cp)
            if 'PIXELS' in line:
                pidx = parts.index('PIXELS')
                pix = bytes(int(x, 16) for x in parts[pidx + 1:])
            else:
                pix = b""
            sha = SHA(pix).hexdigest() if pix else SHA(b"\x00").hexdigest()
            rows.append({"id": f"{fn}_{sz}_{cp}_getmask", "font": fn, "size_pt": sz,
                        "codepoint": cp, "char": ch, "operation": "getmask", "status": "active",
                        "ref_sha256": sha, "ref_size": [w, h]})
            rows.append({"id": f"{fn}_{sz}_{cp}_getbbox", "font": fn, "size_pt": sz,
                        "codepoint": cp, "char": ch, "operation": "getbbox", "status": "active",
                        "ref_value": [left, top - h, left + w, top]})

matrix = {"version": "2.0.0", "font_source": "fonts_autohint", "hinting": "autohint",
          "generator": "FreeType 2.14.3 (locally built from vendored source) FT_LOAD_FORCE_AUTOHINT",
          "mode": "FreeType-raw", "rows": rows,
          "summary": {"total_rows": len(rows), "active_rows": len(rows), "fonts": len(FONTS), "sizes": 5, "glyphs": 94}}

# getlength: add from PIL matrix (backend-independent, hmtx-based advance widths)
# Remap old PIL font names to current fixture filenames
PIL_FONT_REMAP = {
    "DejaVuSans": "DejaVuSans",
    "DejaVuSerif": "DejaVuSerif",
    "DejaVuSansMono": "DejaVuSansMono",
    "DejaVuSansCondensed": "DejaVuSansCondensed",
    "LiberationSerif": "LiberationSerif-Regular",
    "NotoSans": "NotoSans-Regular",
    "Ubuntu": "Ubuntu[wdth,wght]",
}
pil_matrix = json.loads((FIXTURES / "coverage_matrix.json").read_text())
for r in pil_matrix["rows"]:
    if r.get("operation") == "getlength":
        r = dict(r)  # copy
        old_font = r.get("font", "")
        if old_font in PIL_FONT_REMAP:
            r["font"] = PIL_FONT_REMAP[old_font]
            r["id"] = r["id"].replace(old_font, PIL_FONT_REMAP[old_font])
        rows.append(r)

MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
print(f"FT 2.14.3 matrix: {len(rows)} rows -> {MATRIX_PATH}", file=sys.stderr)

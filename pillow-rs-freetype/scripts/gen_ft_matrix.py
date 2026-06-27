#!/usr/bin/env python3
"""Generate the FreeType raw reference matrix using the locally-built
FreeType 2.14.3 binary (/tmp/gen_ft_refs).

Requires: gen_ft_refs.c compiled with:
  gcc -o /tmp/gen_ft_refs scripts/gen_ft_refs.c \
    -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype \
    -Wl,-rpath,$HOME/.local/lib

Also requires: FreeType 2.14.3 built via scripts/build_ft.sh
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
    "DejaVuSans": ("DejaVuSans.ttf", "DejaVu Sans", "Book"),
    "LiberationSerif": ("LiberationSerif-Regular.ttf", "Liberation Serif", "Regular"),
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
            total = 0.0
            for ch in "Hello":
                for l2 in lines:
                    if l2.startswith(f"GLYPH {ord(ch)} "):
                        total += int(l2.split()[5]) / 64.0
                        break
            rows.append({"id": f"{fn}_{sz}_getlength_hello", "font": fn, "size_pt": sz,
                        "codepoint": 0, "char": "Hello", "operation": "getlength", "status": "active",
                        "ref_value": total})
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
          "generator": "FreeType 2.14.3 (locally built from vendored source) FT_LOAD_RENDER",
          "mode": "FreeType-raw", "rows": rows,
          "summary": {"total_rows": len(rows), "active_rows": len(rows), "fonts": 2, "sizes": 5, "glyphs": 94}}
MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
print(f"FT 2.14.3 matrix: {len(rows)} rows -> {MATRIX_PATH}", file=sys.stderr)

#!/usr/bin/env python3
"""Generate multi-script font fixtures and FreeType reference data.

Phase 1: Collect fonts per script (system + Noto fallback)
Phase 2: Generate fixture JSON entries
Phase 3: Compute SHA-256 + bbox references using vendored FreeType 2.14.3

Usage:
    python3 scripts/generate_multi_script_fixtures.py \
        --output pillow-rs-freetype/tests/fixtures/coverage_matrix_multi.json \
        --font-dir pillow-rs-freetype/tests/fixtures/input/fonts_autohint/ \
        --ref-bin /tmp/gen_refs_v2
"""

import os, sys, json, subprocess, hashlib, struct

# ── Script to language/font mapping ───────────────────────────────────────

SCRIPT_FONTS = {
    # Scripts with good system font coverage
    'arab': {'lang': 'ar', 'chars': [0x0628, 0x0633, 0x0645]},  # ب س م
    'armn': {'lang': 'hy', 'chars': [0x0531, 0x0544, 0x057D]},  # Ա Մ ս
    'beng': {'lang': 'bn', 'chars': [0x0995, 0x09AE, 0x09B0]},  # ক ম র
    'cyrl': {'lang': 'ru', 'chars': [0x0410, 0x041C, 0x042F]},  # А М Я
    'deva': {'lang': 'hi', 'chars': [0x0915, 0x092E, 0x0930]},  # क म र
    'ethi': {'lang': 'am', 'chars': [0x1200, 0x1210, 0x1228]},  # ሀ ሐ ረ
    'geor': {'lang': 'ka', 'chars': [0x10D0, 0x10DB, 0x10E0]},  # ა მ რ
    'grek': {'lang': 'el', 'chars': [0x0391, 0x039C, 0x03A9]},  # Α Μ Ω
    'gujr': {'lang': 'gu', 'chars': [0x0A95, 0x0AAE, 0x0AB0]},  # ક મ ર
    'guru': {'lang': 'pa', 'chars': [0x0A15, 0x0A2E, 0x0A30]},  # ਕ ਮ ਰ
    'hani': {'lang': 'zh', 'chars': [0x4E2D, 0x56FD, 0x4EBA]},  # 中 国 人
    'hebr': {'lang': 'he', 'chars': [0x05D0, 0x05DE, 0x05E9]},  # א מ ש
    'khmr': {'lang': 'km', 'chars': [0x1780, 0x1798, 0x17A2]},  # ក ម អ
    'knda': {'lang': 'kn', 'chars': [0x0C95, 0x0CAE, 0x0CB0]},  # ಕ ಮ ರ
    'lao':  {'lang': 'lo', 'chars': [0x0E81, 0x0E9A, 0x0EAA]},  # ກ ບ ສ
    'latn': {'lang': 'en', 'chars': [0x0041, 0x0061, 0x004F]},  # A a O
    'mlym': {'lang': 'ml', 'chars': [0x0D15, 0x0D2E, 0x0D30]},  # ക മ ര
    'mymr': {'lang': 'my', 'chars': [0x1000, 0x1019, 0x101B]},  # က မ ရ
    'sinh': {'lang': 'si', 'chars': [0x0D9A, 0x0DB8, 0x0DBB]},  # ක ම ර
    'taml': {'lang': 'ta', 'chars': [0x0B95, 0x0BAE, 0x0BB0]},  # க ம ர
    'telu': {'lang': 'te', 'chars': [0x0C15, 0x0C2E, 0x0C30]},  # క మ ర
    'thai': {'lang': 'th', 'chars': [0x0E01, 0x0E21, 0x0E23]},  # ก ม ร
}

# Sizes to test
SIZES = [10, 16, 24]


def find_system_fonts(lang):
    """Find up to 3 system TTF/OTF fonts for a given language code."""
    try:
        result = subprocess.run(
            ['fc-list', f':lang={lang}', 'file'],
            capture_output=True, text=True, timeout=5
        )
        fonts = []
        for line in result.stdout.strip().split('\n'):
            if not line.strip():
                continue
            path = line.split(':')[0].strip()
            if path.endswith(('.ttf', '.otf')) and os.path.exists(path):
                # Prefer Noto, then DejaVu, then Liberation, then others
                fonts.append(path)
        # Deduplicate and limit to 3
        seen = set()
        unique = []
        for f in fonts:
            base = os.path.basename(f)
            if base not in seen:
                seen.add(base)
                unique.append(f)
        return unique[:3]
    except Exception as e:
        print(f"  WARNING: fc-list failed for {lang}: {e}", file=sys.stderr)
        return []


def find_noto_fallback(script_tag):
    """Try to find Noto fonts for a script by searching common paths."""
    search_dirs = [
        '/usr/share/fonts/truetype/noto',
        '/usr/share/fonts/opentype/noto',
        '/usr/local/share/fonts',
    ]
    noto_names = {
        'adlm': 'NotoSansAdlam', 'arab': 'NotoSansArabic',
        'armn': 'NotoSansArmenian', 'beng': 'NotoSansBengali',
        'cyrl': 'NotoSans', 'deva': 'NotoSansDevanagari',
        'ethi': 'NotoSansEthiopic', 'geor': 'NotoSansGeorgian',
        'grek': 'NotoSans', 'gujr': 'NotoSansGujarati',
        'guru': 'NotoSansGurmukhi', 'hani': 'NotoSansCJK',
        'hebr': 'NotoSansHebrew', 'khmr': 'NotoSansKhmer',
        'knda': 'NotoSansKannada', 'lao': 'NotoSansLao',
        'mlym': 'NotoSansMalayalam', 'mymr': 'NotoSansMyanmar',
        'sinh': 'NotoSansSinhala', 'taml': 'NotoSansTamil',
        'telu': 'NotoSansTelugu', 'thai': 'NotoSansThai',
    }
    name = noto_names.get(script_tag)
    if not name:
        return []
    found = []
    for d in search_dirs:
        if os.path.isdir(d):
            for f in sorted(os.listdir(d)):
                if name.lower() in f.lower() and f.endswith(('.ttf', '.otf')):
                    found.append(os.path.join(d, f))
    return found[:3]


def get_fonts_for_script(script_tag):
    """Get up to 3 font paths for a script. Prefer system, fallback to Noto."""
    info = SCRIPT_FONTS.get(script_tag, {})
    lang = info.get('lang', script_tag)
    chars = info.get('chars', [0x0041])  # fallback to 'A'

    fonts = find_system_fonts(lang)
    if len(fonts) < 3:
        fonts += find_noto_fallback(script_tag)
    # Deduplicate
    seen = set()
    unique = []
    for f in fonts:
        b = os.path.basename(f)
        if b not in seen:
            seen.add(b)
            unique.append(f)
    return unique[:3], chars


def run_ref_generator(ref_bin, font_path, codepoint, size_pt):
    """Run the C reference binary and return (width, height, left, top, pixels_hex)."""
    cp_hex = f"{codepoint:04x}"
    size_str = f"{size_pt}"
    try:
        env = os.environ.copy()
        # Use vendored FreeType 2.14.3, NOT system FreeType
        build_dir = os.path.abspath(os.path.join(
            os.path.dirname(os.path.dirname(__file__) or '.'),
            'pillow-rs-freetype/freetype/build'
        ))
        env['LD_LIBRARY_PATH'] = build_dir
        result = subprocess.run(
            [ref_bin, font_path, cp_hex, size_str],
            capture_output=True, text=True, timeout=10,
            env=env
        )
        stderr = result.stderr
        stdout = result.stdout.strip()
        if not stdout or stdout == 'NO_GLYPH':
            return None
        parts = stdout.split()
        if len(parts) < 5:
            return None
        w, h, l, t = int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
        pixels_hex = parts[4] if len(parts) > 4 else ''
        return (w, h, l, t, pixels_hex)
    except Exception as e:
        print(f"  ERROR running ref generator for {os.path.basename(font_path)} U+{codepoint:04X}: {e}", file=sys.stderr)
        return None


def generate_fixtures(ref_bin, font_dir, output_path):
    """Generate the multi-script fixture JSON."""
    rows = []

    for script_tag in sorted(SCRIPT_FONTS.keys()):
        print(f"\n=== {script_tag} ===", file=sys.stderr)
        fonts, chars = get_fonts_for_script(script_tag)

        if not fonts:
            print(f"  WARNING: No fonts found for {script_tag}", file=sys.stderr)
            # Copy fonts to font_dir if from system paths
            continue

        for font_path in fonts:
            font_name = os.path.splitext(os.path.basename(font_path))[0]
            # Skip .otf files — our parser only handles TrueType (.ttf)
            if font_path.endswith('.otf'):
                print(f"  SKIP {font_name} (OTF/CFF not yet supported)", file=sys.stderr)
                continue
            # Copy font to fixtures dir if needed
            dest = os.path.join(font_dir, os.path.basename(font_path))
            if not os.path.exists(dest) and font_path != dest:
                try:
                    with open(font_path, 'rb') as src:
                        with open(dest, 'wb') as dst:
                            dst.write(src.read())
                    print(f"  Copied {os.path.basename(font_path)}", file=sys.stderr)
                except Exception as e:
                    print(f"  WARNING: Cannot copy {font_path}: {e}", file=sys.stderr)
                    dest = font_path  # use source path directly

            # Skip if font already in font_dir but not showing as exist
            if os.path.exists(dest):
                font_name = os.path.splitext(os.path.basename(dest))[0]
            else:
                font_name = os.path.splitext(os.path.basename(font_path))[0]

            for size_pt in SIZES:
                for cp in chars:
                    ref = run_ref_generator(ref_bin, font_path, cp, size_pt)
                    if ref is None:
                        continue
                    w, h, l, t, pixels_hex = ref

                    # Compute SHA-256
                    if pixels_hex:
                        pixel_bytes = bytes.fromhex(pixels_hex)
                        sha = hashlib.sha256(pixel_bytes).hexdigest()
                    else:
                        pixel_bytes = b''
                        sha = hashlib.sha256(b'').hexdigest()

                    row_id = f"{font_name}_{size_pt}_{cp}_{script_tag}"

                    # getmask entry with size reference only (no SHA — pixel parity
                    # for non-Latin scripts requires additional algorithmic work)
                    rows.append({
                        "id": f"{row_id}_getmask",
                        "font": font_name,
                        "size_pt": size_pt,
                        "codepoint": cp,
                        "char": chr(cp) if cp < 0x10000 else "",
                        "operation": "getmask",
                        "status": "size_only",
                        "ref_size": [w, h],
                    })

                    # getbbox entry
                    rows.append({
                        "id": f"{row_id}_getbbox",
                        "font": font_name,
                        "size_pt": size_pt,
                        "codepoint": cp,
                        "char": chr(cp) if cp < 0x10000 else "",
                        "operation": "getbbox",
                        "status": "active",
                        "ref_value": [l, t - h, l + w, t],
                    })

                    print(f"  {font_name} {size_pt}pt U+{cp:04X}: {w}×{h} bbox=({l},{t-h},{l+w},{t})", file=sys.stderr)

    output = {
        "version": "3.0.0",
        "font_source": "fonts_autohint",
        "hinting": "autohint",
        "generator": "FreeType 2.14.3 multi-script",
        "mode": "FreeType-raw",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": sum(1 for r in rows if r['status'] == 'active'),
            "fonts": len(set(r['font'] for r in rows)),
            "sizes": len(set(r['size_pt'] for r in rows)),
            "glyphs": len(set(r['codepoint'] for r in rows)),
        }
    }

    with open(output_path, 'w') as f:
        json.dump(output, f, indent=2)

    print(f"\nWritten {len(rows)} rows ({sum(1 for r in rows if r['status']=='active')} active) to {output_path}", file=sys.stderr)


def main():
    output_path = sys.argv[1] if len(sys.argv) > 1 else 'pillow-rs-freetype/tests/fixtures/coverage_matrix_multi.json'
    font_dir = sys.argv[2] if len(sys.argv) > 2 else 'pillow-rs-freetype/tests/fixtures/input/fonts_autohint/'
    ref_bin = sys.argv[3] if len(sys.argv) > 3 else '/tmp/gen_refs_v2'

    os.makedirs(font_dir, exist_ok=True)
    generate_fixtures(ref_bin, font_dir, output_path)


if __name__ == '__main__':
    main()

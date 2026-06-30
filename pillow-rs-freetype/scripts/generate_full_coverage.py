#!/usr/bin/env python3
"""Generate full blue-zone character coverage per script.

For each of 55 scripts, finds up to 3 TTF fonts, tests ALL blue zone
characters (from afblue.dat) at 2 sizes (10pt, 20pt).

Usage: python3 scripts/generate_full_coverage.py <font_dir> <output_json>
"""

import os, sys, json, subprocess, shutil, re

SIZES = (10, 20)
REF_BIN = '/tmp/gen_refs_v4'
BUILD_DIR = os.path.abspath('pillow-rs-freetype/freetype/build')

def get_all_blue_chars():
    """Parse afblue.dat to get ALL blue zone characters per script.
    Returns dict: script_tag → list of codepoints."""
    blue_path = os.path.join(os.path.dirname(__file__) or '.',
                             '../freetype/src/autofit/afblue.dat')
    # Try both relative paths
    if not os.path.exists(blue_path):
        blue_path = 'pillow-rs-freetype/freetype/src/autofit/afblue.dat'
    
    with open(blue_path) as f:
        text = f.read()

    # Parse strings
    strings = {}
    current = None
    for line in text.split('\n'):
        line = line.strip()
        if not line or line.startswith('//') or line.startswith('#'):
            continue
        m = re.match(r'AF_BLUE_STRING_(\w+)\s*$', line)
        if m and m.group(1) != 'MAX':
            current = m.group(1)
            strings[current] = []
            continue
        if current and line.startswith('"') and line.endswith('"'):
            chars = line[1:-1].split()
            strings[current].extend(c for c in chars if c.strip())

    # Parse stringsets — join multi-line braces first
    joined = []
    buf = ""
    for line in text.split('\n'):
        s = line.strip()
        if not s or s.startswith('//'): continue
        if s.startswith('{') and not buf: buf = s
        elif buf: buf += ' ' + s
        elif s.startswith('#ifdef') or s.startswith('#endif') or s.startswith('#ifndef') or s.startswith('#else'):
            continue
        else: joined.append(s)
        if buf and s.rstrip().endswith('}'): joined.append(buf); buf = ""

    stringsets = {}
    cur_script = None; cur_entries = []
    for line in joined:
        m = re.match(r'AF_BLUE_STRINGSET_(\w+)\s*$', line)
        if m:
            if cur_script: stringsets[cur_script] = cur_entries
            cur_script = m.group(1).lower(); cur_entries = []; continue
        m2 = re.match(r'\{\s*AF_BLUE_STRING_(\w+)\s*,\s*(.+?)\s*\}', line)
        if m2 and m2.group(1) != 'MAX': cur_entries.append(m2.group(1))
    if cur_script: stringsets[cur_script] = cur_entries

    # Build per-script sorted unique chars (as codepoints)
    result = {}
    for tag, entries in sorted(stringsets.items()):
        chars = set()
        for name in entries:
            if name in strings:
                for c in strings[name]:
                    chars.add(ord(c[0]))  # first char of cluster
        if chars:
            result[tag] = sorted(chars)

    return result

def find_fonts(ch_first, max_count=3):
    try:
        result = subprocess.run(
            ['fc-list', f':charset={ch_first:04X}', 'file'],
            capture_output=True, text=True, timeout=5)
    except: return []

    seen = set(); fonts = []
    for line in result.stdout.strip().split('\n'):
        if ':' not in line: continue
        p = line.split(':')[0].strip()
        if not p.endswith('.ttf') or not os.path.exists(p): continue
        name = os.path.splitext(os.path.basename(p))[0]
        if name not in seen:
            seen.add(name); fonts.append((p, name))
            if len(fonts) >= max_count: break
    return fonts

def gen_ref(font_path, cp, size):
    try:
        env = {'LD_LIBRARY_PATH': BUILD_DIR}
        result = subprocess.run(
            [REF_BIN, font_path, f'{cp:04X}', str(size)],
            capture_output=True, text=True, timeout=10, env=env)
        out = result.stdout.strip()
        if not out or out == 'NO_GLYPH': return None
        parts = out.split()
        if len(parts) < 5: return None
        return int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
    except: return None

def main():
    font_dir = sys.argv[1]
    output_json = sys.argv[2]
    os.makedirs(font_dir, exist_ok=True)

    all_chars = get_all_blue_chars()
    print(f"Loaded blue zone chars for {len(all_chars)} scripts", file=sys.stderr)

    rows = []; per_script = {}; total_refs = 0

    for script in sorted(all_chars):
        chars = all_chars[script]
        if not chars:
            continue

        found = find_fonts(chars[0], 3)
        if not found:
            print(f"SKIP {script}: no fonts ({len(chars)} chars)", file=sys.stderr)
            continue

        per_script[script] = len(found)
        script_entries = 0

        for path, name in found:
            dest = os.path.join(font_dir, f"{name}.ttf")
            if not os.path.exists(dest):
                shutil.copy2(path, dest)

            for size in SIZES:
                for cp in chars:
                    ref = gen_ref(dest, cp, size)
                    if ref is None: continue
                    w, h, l, t = ref
                    rid = f"{name}_{size}_{cp}_{script}"; total_refs += 1

                    rows.append({
                        "id": f"{rid}_getmask", "font": name,
                        "size_pt": size, "codepoint": cp,
                        "char": chr(cp) if cp < 0x10000 else "",
                        "operation": "getmask", "status": "size_only",
                        "ref_size": [w, h],
                    })
                    rows.append({
                        "id": f"{rid}_getbbox", "font": name,
                        "size_pt": size, "codepoint": cp,
                        "char": chr(cp) if cp < 0x10000 else "",
                        "operation": "getbbox", "status": "size_only",
                        "ref_value": [l, t - h, l + w, t],
                    })
                    script_entries += 2

        tag = f"{len(found)} font(s)" if len(found) >= 3 else f"{len(found)} font only"
        print(f"  {script:6s}: {tag:15s} {len(chars)} chars, {script_entries} entries",
              file=sys.stderr)

    dist = {}
    for s, n in per_script.items(): dist[n] = dist.get(n, 0) + 1

    fonts_set = sorted(set(r['font'] for r in rows))
    output = {
        "version": "3.0.0",
        "font_source": "fonts_autohint",
        "hinting": "autohint",
        "generator": f"FreeType 2.14.3 × {len(fonts_set)} fonts × all blue zone chars",
        "mode": "FreeType-raw",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "fonts": len(fonts_set),
            "sizes": len(SIZES),
            "scripts": len(per_script),
            "distribution": dist,
            "total_glyphs_tested": total_refs,
        }
    }
    with open(output_json, 'w') as f:
        json.dump(output, f, indent=2)

    print(f"\n{'='*60}", file=sys.stderr)
    print(f"Scripts: {len(per_script)}   Fonts: {len(fonts_set)}   Rows: {len(rows)}   Glyphs: {total_refs}",
          file=sys.stderr)
    for n in sorted(dist, reverse=True):
        print(f"  {n} font(s): {dist[n]} scripts", file=sys.stderr)

if __name__ == '__main__':
    main()

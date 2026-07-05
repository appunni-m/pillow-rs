#!/usr/bin/env python3
"""
build_fixtures.py — systematic force_autohint fixture pipeline.

Single source of truth: font_inventory.json
  → maps each font → {script_tag: [codepoints]}
  → maps each script → all blue zone codepoints

Pipeline:
  1. --inventory : probes fonts via FreeType, writes font_inventory.json
  2. (default)   : reads font_inventory.json, generates force_autohint_matrix.json

Usage:
  python3 scripts/build_fixtures.py                    # build force_autohint fixture
  python3 scripts/build_fixtures.py --inventory        # rebuild inventory
  python3 scripts/build_fixtures.py --full             # inventory + unified

Input:  tests/fixtures/font_inventory.json
Output: tests/fixtures/force_autohint_matrix.json (55-script pixel fixture)

Dependencies:
  - FT_REF_BIN or /tmp/gen_refs_v4 (FreeType 2.14.3 oracle helper)
"""

import os, sys, json, subprocess, hashlib, re, argparse

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)) or '.')
FONT_DIR = os.path.join(ROOT, 'tests/fixtures/input/fonts_autohint')
INVENTORY_PATH = os.path.join(ROOT, 'tests/fixtures/font_inventory.json')
UNIFIED_PATH = os.path.join(ROOT, 'tests/fixtures/force_autohint_matrix.json')
RAW_DIR = os.path.join(ROOT, 'tests/fixtures/outputs/raws_ft')
BLUE_DAT = os.path.join(ROOT, 'freetype/src/autofit/afblue.dat')
BUILD_DIR = os.path.join(ROOT, 'freetype/build')
REF_BIN = os.environ.get('FT_REF_BIN', '/tmp/gen_refs_v4')

SIZES = (10, 20)

# ── Blue zone parser ──────────────────────────────────────────────────────

def parse_blue_chars():
    """Parse afblue.dat → {script_tag: [codepoint, ...]}."""
    with open(BLUE_DAT) as f: text = f.read()

    strings = {}; current = None
    for line in text.split('\n'):
        line = line.strip()
        if not line or line.startswith('//') or line.startswith('#'): continue
        m = re.match(r'AF_BLUE_STRING_(\w+)\s*$', line)
        if m and m.group(1) != 'MAX': current = m.group(1); strings[current] = []; continue
        if current and line.startswith('"') and line.endswith('"'):
            strings[current].extend(c for c in line[1:-1].split() if c.strip())

    # Parse stringsets — join multi-line braces
    joined = []; buf = ""
    for line in text.split('\n'):
        s = line.strip()
        if not s or s.startswith('//'): continue
        if s.startswith('#ifdef') or s.startswith('#endif') or s.startswith('#ifndef') or s.startswith('#else'): continue
        if s.startswith('{') and not buf: buf = s
        elif buf: buf += ' ' + s
        else: joined.append(s)
        if buf and s.rstrip().endswith('}'): joined.append(buf); buf = ""

    stringsets = {}; cur_script = None; entries = []
    for line in joined:
        m = re.match(r'AF_BLUE_STRINGSET_(\w+)\s*$', line)
        if m:
            if cur_script: stringsets[cur_script] = entries
            cur_script = m.group(1).lower(); entries = []; continue
        m2 = re.match(r'\{\s*AF_BLUE_STRING_(\w+)\s*,\s*(.+?)\s*\}', line)
        if m2 and m2.group(1) != 'MAX': entries.append(m2.group(1))
    if cur_script: stringsets[cur_script] = entries

    result = {}
    for tag, strs in sorted(stringsets.items()):
        chars = set()
        for name in strs:
            if name in strings:
                for c in strings[name]: chars.add(ord(c[0]))
        if chars: result[tag] = sorted(chars)
    return result

# ── Inventory ──────────────────────────────────────────────────────────────

def build_inventory():
    """Probe every TTF via FreeType, return {font_name: {path, scripts: {script: [cps]}}}."""
    script_chars = parse_blue_chars()
    inventory = {}

    for fname in sorted(os.listdir(FONT_DIR)):
        if not fname.endswith('.ttf'): continue
        name = fname[:-4]
        path = os.path.join(FONT_DIR, fname)

        coverage = {}
        for script, codepoints in script_chars.items():
            env = {'LD_LIBRARY_PATH': BUILD_DIR}
            try:
                r = subprocess.run([REF_BIN, path, f'{codepoints[0]:04X}', '10'],
                                   capture_output=True, text=True, timeout=5, env=env)
                if r.stdout.strip() and 'NO_GLYPH' not in r.stdout:
                    existing = []
                    for cp in codepoints:
                        r2 = subprocess.run([REF_BIN, path, f'{cp:04X}', '10'],
                                            capture_output=True, text=True, timeout=5, env=env)
                        if r2.stdout.strip() and 'NO_GLYPH' not in r2.stdout:
                            existing.append(cp)
                    if existing:
                        coverage[script] = existing
            except: pass

        if coverage:
            inventory[name] = {"path": fname, "scripts": coverage}
            total = sum(len(cps) for cps in coverage.values())
            print(f"  {name}: {len(coverage)} scripts, {total} chars", file=sys.stderr)

    return inventory, script_chars

def load_inventory():
    if os.path.exists(INVENTORY_PATH):
        with open(INVENTORY_PATH) as f:
            data = json.load(f)
        return data['fonts'], data.get('script_chars', parse_blue_chars())
    print("font_inventory.json not found, building...", file=sys.stderr)
    inventory, script_chars = build_inventory()
    save_inventory(inventory, script_chars)
    return inventory, script_chars

def save_inventory(inventory, script_chars):
    os.makedirs(os.path.dirname(INVENTORY_PATH), exist_ok=True)
    doc = {
        "version": "1.0.0",
        "description": "Font→script→codepoint mapping. Single source of truth for fixture generation.",
        "pipeline": "python3 scripts/build_fixtures.py reads this -> generates force_autohint_matrix.json",
        "sizes": list(SIZES),
        "script_chars": {s: cps for s, cps in sorted(script_chars.items())},
        "fonts": inventory,
    }
    with open(INVENTORY_PATH, 'w') as f:
        json.dump(doc, f, indent=2)
    print(f"Inventory: {len(inventory)} fonts → {INVENTORY_PATH}", file=sys.stderr)

# ── Reference generation ──────────────────────────────────────────────────

def gen_ref(font_path, cp, size):
    try:
        env = {'LD_LIBRARY_PATH': BUILD_DIR}
        r = subprocess.run([REF_BIN, font_path, f'{cp:04X}', str(size)],
                           capture_output=True, text=True, timeout=10, env=env)
        out = r.stdout.strip()
        if not out or 'NO_GLYPH' in out: return None
        parts = out.split()
        if len(parts) < 5: return None
        w, h, l, t, px = int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3]), parts[4]
        sha = hashlib.sha256(bytes.fromhex(px)).hexdigest() if px else hashlib.sha256(b'').hexdigest()
        return (w, h, l, t, sha, px)
    except: return None

def build_fixture(inventory):
    rows = []
    os.makedirs(RAW_DIR, exist_ok=True)
    for font_name in sorted(inventory):
        info = inventory[font_name]
        path = os.path.join(FONT_DIR, info['path'])
        for script, codepoints in info['scripts'].items():
            for size in SIZES:
                for cp in codepoints:
                    ref = gen_ref(path, cp, size)
                    if ref is None: continue
                    w, h, l, t, sha, px = ref
                    rid = f"{font_name}_{size}_{cp}_{script}"
                    raw_name = f"{rid}_getmask.bin"
                    with open(os.path.join(RAW_DIR, raw_name), 'wb') as raw:
                        raw.write(bytes.fromhex(px) if px else b'')
                    rows.append({
                        "id": f"{rid}_force_autohint_getmask",
                        "fixture_family": "force_autohint",
                        "generator": "scripts/build_fixtures.py + scripts/gen_ft_refs.c",
                        "load_flags": ["FT_LOAD_RENDER", "FT_LOAD_FORCE_AUTOHINT"],
                        "render_mode": "FT_RENDER_MODE_NORMAL",
                        "font": font_name, "size_pt": size,
                        "script": script,
                        "codepoint": cp, "char": chr(cp) if cp < 0x10000 else "",
                        "operation": "getmask", "status": "active",
                        "metrics": {},
                        "bitmap": {"width": w, "rows": h, "left": l, "top": t},
                        "bbox": {"bitmap_pixels": {"x_min": l, "y_min": t - h, "x_max": l + w, "y_max": t}},
                        "bitmap_placement": {"left": l, "top": t},
                        "raw_pixels": px if px else "",
                        "ref_sha256": sha, "ref_size": [w, h],
                        "ref_raw": f"outputs/raws_ft/{raw_name}",
                    })
                    rows.append({
                        "id": f"{rid}_force_autohint_getbbox",
                        "fixture_family": "force_autohint",
                        "generator": "scripts/build_fixtures.py + scripts/gen_ft_refs.c",
                        "load_flags": ["FT_LOAD_RENDER", "FT_LOAD_FORCE_AUTOHINT"],
                        "render_mode": "FT_RENDER_MODE_NORMAL",
                        "font": font_name, "size_pt": size,
                        "script": script,
                        "codepoint": cp, "char": chr(cp) if cp < 0x10000 else "",
                        "operation": "getbbox", "status": "active",
                        "metrics": {},
                        "bitmap": {"width": w, "rows": h, "left": l, "top": t},
                        "bbox": {"bitmap_pixels": {"x_min": l, "y_min": t - h, "x_max": l + w, "y_max": t}},
                        "bitmap_placement": {"left": l, "top": t},
                        "raw_pixels": px if px else "",
                        "ref_value": [l, t - h, l + w, t],
                    })
    return rows

# ── Main ──────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Build fixture from font_inventory.json")
    parser.add_argument('--inventory', action='store_true', help='Rebuild inventory only')
    args = parser.parse_args()

    if args.inventory:
        print("Building font inventory...", file=sys.stderr)
        inventory, script_chars = build_inventory()
        save_inventory(inventory, script_chars)
        return

    inventory, _ = load_inventory()

    print(f"Building unified fixture from {len(inventory)} fonts...", file=sys.stderr)
    rows = build_fixture(inventory)
    fonts = sorted(set(r['font'] for r in rows))
    scripts = sorted(set(r['id'].rsplit('_')[-2] for r in rows))

    output = {
        "version": "5.1.0",
        "fixture_family": "force_autohint",
        "generator": "scripts/build_fixtures.py",
        "source": "font_inventory.json -> pinned FreeType 2.14.3",
        "font_source": "fonts_autohint",
        "load_flags": ["FT_LOAD_RENDER", "FT_LOAD_FORCE_AUTOHINT"],
        "render_mode": "FT_RENDER_MODE_NORMAL",
        "assert_pixel_parity": True,
        "rows": rows,
        "summary": {
            "total_rows": len(rows), "fonts": len(fonts),
            "sizes": list(SIZES), "scripts": len(scripts),
        }
    }
    with open(UNIFIED_PATH, 'w') as f:
        json.dump(output, f, indent=2)
    print(f"Unified: {len(rows)} rows, {len(fonts)} fonts, {len(scripts)} scripts → {UNIFIED_PATH}", file=sys.stderr)

if __name__ == '__main__':
    main()

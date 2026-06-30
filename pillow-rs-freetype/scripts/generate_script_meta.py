#!/usr/bin/env python3
r"""Generate per-script metadata: standard character + all blue zone chars.
Parsed from afscript.h (C source with hex escapes) + afblue.dat.
Appends to globals_data.rs.

Usage: python3 scripts/generate_script_meta.py \
    <afscript.h> <afblue.dat> \
    >> pillow-rs-freetype/src/autohint/globals_data.rs
"""

import re, sys

def decode_c_escapes(s):
    """Decode C-style \\xNN escaped UTF-8 bytes into a Python string."""
    # Match \xNN patterns
    result = bytearray()
    i = 0
    while i < len(s):
        if s[i] == '\\' and i + 3 < len(s) and s[i+1] == 'x':
            hex_val = s[i+2:i+4]
            try:
                result.append(int(hex_val, 16))
                i += 4
            except ValueError:
                result.append(ord(s[i]))
                i += 1
        else:
            result.append(ord(s[i]))
            i += 1
    return result.decode('utf-8', errors='replace')

def parse_script_meta(path):
    """Extract per-script metadata from afscript.h."""
    with open(path, 'rb') as f:
        raw = f.read().decode('utf-8', errors='replace')

    scripts = {}
    # SCRIPT( tag, TAG, "desc", HB_SCRIPT, HINTING_XXX, "charstring" )
    # The charstring may contain \xNN C escapes
    pattern = r'SCRIPT\(\s*(\w+),\s*\w+,\s*"([^"]*)",\s*\w+,\s*(\w+),\s*"([^"]*)"\s*\)'
    for m in re.finditer(pattern, raw):
        tag = m.group(1).lower()
        desc = m.group(2)
        hinting = m.group(3)
        scs_raw = m.group(4)  # may contain \xNN escapes
        
        # Decode C escapes
        scs = decode_c_escapes(scs_raw)
        # Split by spaces to get individual chars
        chars = scs.split()
        standard_char = chars[0] if chars else 'o'
        
        scripts[tag] = {
            'description': desc,
            'hinting': hinting,
            'standard_char': standard_char,
        }
    return scripts

def parse_blue_chars(path):
    """Extract all blue zone characters per script from afblue.dat."""
    with open(path) as f:
        text = f.read()

    # Parse strings section
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

    # Parse stringsets — join multi-line braces
    joined = []
    buf = ""
    for line in text.split('\n'):
        s = line.strip()
        if not s or s.startswith('//'):
            continue
        if s.startswith('{') and not buf:
            buf = s
        elif buf:
            buf += ' ' + s
        elif s.startswith('#ifdef') or s.startswith('#endif') or s.startswith('#ifndef') or s.startswith('#else'):
            continue
        else:
            joined.append(s)
        if buf and s.rstrip().endswith('}'):
            joined.append(buf)
            buf = ""

    stringsets = {}
    cur_script = None
    cur_entries = []
    for line in joined:
        m = re.match(r'AF_BLUE_STRINGSET_(\w+)\s*$', line)
        if m:
            if cur_script:
                stringsets[cur_script] = cur_entries
            cur_script = m.group(1).lower()
            cur_entries = []
            continue
        m2 = re.match(r'\{\s*AF_BLUE_STRING_(\w+)\s*,\s*(.+?)\s*\}', line)
        if m2 and m2.group(1) != 'MAX':
            cur_entries.append(m2.group(1))

    if cur_script:
        stringsets[cur_script] = cur_entries

    # Build per-script sorted unique chars
    result = {}
    for tag, entries in stringsets.items():
        chars = set()
        for name in entries:
            if name in strings:
                for c in strings[name]:
                    chars.add(c[0])  # first char of cluster
        if chars:
            result[tag] = sorted(chars)
    return result

def main():
    script_h = sys.argv[1]
    blue_dat = sys.argv[2]

    scripts = parse_script_meta(script_h)
    blue_chars = parse_blue_chars(blue_dat)

    print("// ── Per-script standard characters (from afscript.h) ──────────")
    print()
    print("/// Get the standard character for stem width detection for a script tag.")
    print("/// Falls back to 'o' for scripts not in the table.")
    print("pub fn standard_char_for_script(tag: &str) -> char {")
    print("    match tag {")
    for tag in sorted(scripts.keys()):
        ch = scripts[tag]['standard_char']
        # Could be multi-byte UTF-8. Just use the first char.
        print(f"        \"{tag}\" => '\\u{{{ord(ch):04X}}}',")
    print("        _ => 'o',")
    print("    }")
    print("}")
    print()

    # Blue zone chars per script
    print("/// Get all blue zone characters for a script tag.")
    print("/// Returns all characters that define that script's blue zones.")
    print("pub fn blue_chars_for_script(tag: &str) -> &'static [u32] {")
    print("    match tag {")
    for tag in sorted(blue_chars.keys()):
        if tag in ('latb', 'latp'):
            continue
        chars = blue_chars[tag]
        cp_list = ', '.join(f'0x{ord(c):04X}' for c in chars)
        print(f"        \"{tag}\" => &[{cp_list}],")
    print("        _ => &[],")
    print("    }")
    print("}")

    print(f"\n// Generated: {len(scripts)} scripts, {len(blue_chars)} with blue zone chars",
          file=sys.stderr)

if __name__ == '__main__':
    main()

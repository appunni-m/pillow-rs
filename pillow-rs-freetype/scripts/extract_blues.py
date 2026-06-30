#!/usr/bin/env python3
"""Extract blue string entries from FreeType's afblue.dat and generate Rust code.

Parses both the AF_BLUE_STRING_ENUM section (character data) and
AF_BLUE_STRINGSET_ENUM section (script-to-entries mapping with property flags).

Usage: python3 scripts/extract_blues.py \
    pillow-rs-freetype/freetype/src/autofit/afblue.dat \
    > pillow-rs-freetype/src/autohint/blue_strings.rs
"""

import re
import sys


def parse_strings(text):
    """Parse AF_BLUE_STRING_ENUM section to get character arrays per entry name.
    Returns dict: entry_name -> list of characters."""
    strings = {}
    # Find the strings section
    m = re.search(
        r'AF_BLUE_STRING_ENUM\s+AF_BLUE_STRINGS_ARRAY\s+AF_BLUE_STRING_MAX_LEN\s*:\n(.*?)(?=\nAF_BLUE_STRINGSET_ENUM|\Z)',
        text, re.DOTALL
    )
    if not m:
        print("ERROR: Could not find AF_BLUE_STRING_ENUM section", file=sys.stderr)
        sys.exit(1)

    section = m.group(1)
    current_name = None
    for line in section.split('\n'):
        line = line.strip()
        if not line or line.startswith('//') or line.startswith('#'):
            continue

        # Entry name line: AF_BLUE_STRING_XXX
        m2 = re.match(r'AF_BLUE_STRING_(\w+)\s*$', line)
        if m2:
            current_name = m2.group(1)
            if current_name == 'MAX':
                current_name = None
            else:
                strings[current_name] = []
            continue

        # Character data line: "a b c ..."
        if line.startswith('"') and line.endswith('"') and current_name:
            chars = line[1:-1].split()
            # Store only first char of each cluster for cmap lookup
            for c in chars:
                strings[current_name].append(c[0])

    return strings


def parse_stringsets(text):
    """Parse AF_BLUE_STRINGSET_ENUM section to get script-to-entries mapping.
    Returns dict: script_name -> list of (entry_name, props_int) tuples."""
    stringsets = {}
    # Find the stringsets section
    m = re.search(
        r'AF_BLUE_STRINGSET_ENUM\s+AF_BLUE_STRINGSETS_ARRAY\s+AF_BLUE_STRINGSET_MAX_LEN\s*:\n(.*)',
        text, re.DOTALL
    )
    if not m:
        print("ERROR: Could not find AF_BLUE_STRINGSET_ENUM section", file=sys.stderr)
        sys.exit(1)

    section = m.group(1)

    # First, join multi-line brace entries into single lines.
    # A brace entry starts with '{' and may span multiple lines.
    lines = section.split('\n')
    joined_lines = []
    in_brace = False
    brace_buffer = ""
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith('//'):
            continue
        if stripped.startswith('#ifdef') or stripped.startswith('#endif') or stripped.startswith('#ifndef') or stripped.startswith('#else'):
            continue
        if stripped.startswith('{') and not in_brace:
            in_brace = True
            brace_buffer = stripped
        elif in_brace:
            brace_buffer += ' ' + stripped
        else:
            joined_lines.append(stripped)
        if in_brace and stripped.endswith('}'):
            joined_lines.append(brace_buffer)
            in_brace = False
            brace_buffer = ""

    current_script = None
    current_entries = []

    for line in joined_lines:
        # Script header: AF_BLUE_STRINGSET_XXXX
        m2 = re.match(r'AF_BLUE_STRINGSET_(\w+)\s*$', line)
        if m2:
            if current_script and current_entries:
                stringsets[current_script] = current_entries
            current_script = m2.group(1)
            current_entries = []
            continue

        # Entry line: { AF_BLUE_STRING_XXX, PROPS }
        m3 = re.match(r'\{\s*AF_BLUE_STRING_(\w+)\s*,\s*(.+?)\s*\}', line)
        if m3:
            entry_name = m3.group(1)
            props_str = m3.group(2).strip()

            if entry_name == 'MAX':
                continue

            # Parse property flags into a u32
            props_val = parse_props(props_str)
            current_entries.append((entry_name, props_val))
            continue

    if current_script and current_entries:
        stringsets[current_script] = current_entries

    return stringsets


def parse_props(props_str):
    """Parse AF_BLUE_PROPERTY_* flags string into a u32 value."""
    val = 0
    # Map of property names to bit values (from aflatin.h / afblue.h in FreeType)
    prop_bits = {
        'AF_BLUE_PROPERTY_LATIN_TOP':             (1 << 0),
        'AF_BLUE_PROPERTY_LATIN_SUB_TOP':         (1 << 1),
        'AF_BLUE_PROPERTY_LATIN_NEUTRAL':         (1 << 2),
        'AF_BLUE_PROPERTY_LATIN_X_HEIGHT':        (1 << 3),
        'AF_BLUE_PROPERTY_LATIN_LONG':            (1 << 4),
        'AF_BLUE_PROPERTY_LATIN_CAPITAL_BOTTOM':  (1 << 5),
        'AF_BLUE_PROPERTY_LATIN_SMALL_BOTTOM':    (1 << 6),
        'AF_BLUE_PROPERTY_CJK_TOP':               (1 << 0),
        'AF_BLUE_PROPERTY_CJK_HORIZ':             (1 << 1),
        'AF_BLUE_PROPERTY_CJK_RIGHT':             (1 << 2),
    }
    for token in re.split(r'\s*\|\s*', props_str):
        token = token.strip()
        if token == '0' or not token:
            continue
        bit = prop_bits.get(token)
        if bit is None:
            print(f"WARNING: Unknown property flag: {token}", file=sys.stderr)
        else:
            val |= bit
    return val


def codepoint_to_rust(ch):
    """Convert a UTF-8 character/cluster to Rust char literal(s)."""
    if len(ch) == 1:
        cp = ord(ch)
        return f"'\\u{{{cp:04X}}}'"
    else:
        cps = [f"'\\u{{{ord(c):04X}}}'" for c in ch]
        return f"[{', '.join(cps)}]"


def entry_name_to_const(name):
    """Convert AF_BLUE_STRING_XXX to a Rust const name: XXX_CHARS."""
    return f"{name}_CHARS"


def script_name_to_const(name):
    """Convert AF_BLUE_STRINGSET_XXXX to a Rust const name: SCRIPT_XXXX."""
    return f"SCRIPT_{name}"


def main():
    if len(sys.argv) < 2:
        print("Usage: extract_blues.py <afblue.dat>", file=sys.stderr)
        sys.exit(1)

    with open(sys.argv[1], encoding='utf-8') as f:
        data = f.read()

    strings = parse_strings(data)
    stringsets = parse_stringsets(data)

    print("//! Auto-generated blue string table from FreeType's afblue.dat.")
    print("//! Generated by scripts/extract_blues.py — DO NOT EDIT BY HAND.")
    print()

    # --- Character data constants ---
    print("// ── Blue string character arrays ──────────────────────────────────")
    for name in sorted(strings.keys()):
        chars = strings[name]
        const_name = entry_name_to_const(name)
        rust_chars = ', '.join(codepoint_to_rust(c) for c in chars)
        print(f"const {const_name}: &[char] = &[{rust_chars}];")
    print()

    # --- BlueStringEntry struct ---
    print("/// One blue string entry mapping characters to property flags.")
    print("#[derive(Debug, Clone)]")
    print("pub struct BlueStringEntry {")
    print("    pub chars: &'static [char],")
    print("    pub props: u32,")
    print("}")
    print()

    # --- Per-script entry arrays ---
    print("// ── Per-script blue string entry arrays ───────────────────────────")
    for script in sorted(stringsets.keys()):
        entries = stringsets[script]
        const_name = script_name_to_const(script)
        print(f"pub const {const_name}: &[BlueStringEntry] = &[")
        for entry_name, props in entries:
            chars_const = entry_name_to_const(entry_name)
            print(f"    BlueStringEntry {{ chars: {chars_const}, props: {props:#010x} }},")
        print("];")
        print()

    # --- Script lookup table ---
    print("// ── Script detection table ────────────────────────────────────────")
    print("/// Script tag, first detection character, and blue string entries.")
    print("/// Ordered for script detection: first match wins (Latin is fallback).")
    print("pub const SCRIPT_TABLE: &[(&str, char, &[BlueStringEntry])] = &[")

    # Order: LATN first (most common), then other scripts, then special variants.
    # Most fonts use Latin blue zones. Only switch to non-Latin when Latin chars
    # are absent from the font.
    latn_script = None
    non_latin = []
    special_latin = []
    for script in sorted(stringsets.keys()):
        if script == 'LATN':
            latn_script = script
        elif script in ('LATB', 'LATP'):
            special_latin.append(script)
        else:
            non_latin.append(script)

    ordered = []
    if latn_script:
        ordered.append(latn_script)
    ordered.extend(non_latin)
    ordered.extend(special_latin)

    for script in ordered:
        entries = stringsets[script]
        const_name = script_name_to_const(script)
        first_entry_name = entries[0][0]
        first_char = strings.get(first_entry_name, [None])[0]
        if first_char is None:
            print(f"// WARNING: {script} has no characters, skipping", file=sys.stderr)
            continue
        tag = script.lower()
        rust_char = codepoint_to_rust(first_char)
        print(f"    (\"{tag}\", {rust_char}, {const_name}),")

    print("];")


if __name__ == '__main__':
    main()

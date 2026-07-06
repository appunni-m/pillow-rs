#!/usr/bin/env python3
"""Generate globals_data.rs from afranges.c, afstyles.h, blue_strings.rs.

Key design:
- afranges.c → per-script Unicode range arrays (RANGES_XXXX_UNI, RANGES_XXXX_NONBASE)
- afstyles.h → STYLE_TABLE ordering (60 styles matching FreeType's coverage scan order)
- blue_strings.rs → SCRIPT_XXXX blue entry constants (which ones exist)

Multi-line STYLE macros are joined before parsing.
"""

import re, sys

def parse_ranges(path):
    with open(path) as f: text = f.read()
    scripts = {}
    cur, buf = None, []
    for line in text.split('\n'):
        line = line.strip()
        if not line or line.startswith('//') or line.startswith('/*') or line.startswith('*'): continue
        m = re.match(r'const\s+AF_Script_UniRangeRec\s+af_(\w+)_((?:nonbase_)?)uniranges\s*\[\s*\]\s*=', line)
        if m:
            if cur: tag,typ = cur; scripts.setdefault(tag,{'uni':[],'nonbase':[]})[typ]=buf
            cur = (m.group(1), 'nonbase' if m.group(2) else 'uni'); buf = []; continue
        if cur:
            m2 = re.match(r'\s*AF_UNIRANGE_REC\(\s*(0x[0-9a-fA-F]+|\d+)\s*,\s*(0x[0-9a-fA-F]+|\d+)\s*\)', line)
            if m2: buf.append((int(m2.group(1),0), int(m2.group(2),0)))
    if cur: tag,typ = cur; scripts.setdefault(tag,{'uni':[],'nonbase':[]})[typ]=buf
    for s in scripts.values():
        for t in ('uni','nonbase'): s[t] = [(f,l) for f,l in s[t] if not (f==0 and l==0)]
    return scripts

def parse_styles(path):
    """Join multi-line STYLE macros, then extract ordered tags and info."""
    with open(path) as f:
        raw = f.read()

    # Join continuation lines (lines that don't start a new macro)
    lines = raw.split('\n')
    joined = []
    buf = ""
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith('//') or stripped.startswith('/*') or stripped.startswith('*') or stripped.startswith('#'):
            continue
        if re.match(r'(?:STYLE|META_STYLE_LATIN|STYLE_DEFAULT_INDIC)\(', stripped):
            if buf: joined.append(buf)
            buf = stripped
        else:
            buf += ' ' + stripped
    if buf: joined.append(buf)

    ordered, info = [], {}
    for line in joined:
        # STYLE(xxx_dflt, XXX_DFLT, "desc", WS, AF_SCRIPT_YYY, AF_BLUE_STRINGSET_ZZZ, AF_COVERAGE_DEFAULT)
        m = re.match(
            r'STYLE\(\s*(\w+)_dflt\s*,\s*\w+\s*,\s*"([^"]+)"\s*,\s*'
            r'AF_WRITING_SYSTEM_\w+\s*,\s*AF_SCRIPT_(\w+)\s*,\s*'
            r'AF_BLUE_STRINGSET_(\w+)\s*,\s*AF_COVERAGE_DEFAULT\s*\)',
            line
        )
        if m:
            tag = m.group(1)
            if tag != 'none':
                ordered.append(tag)
                info[tag] = {'desc': m.group(2), 'script': m.group(3), 'blue': m.group(4)}
            continue

        # META_STYLE_LATIN(tag, TAG, "desc")
        m2 = re.match(r'META_STYLE_LATIN\(\s*(\w+)\s*,\s*(\w+)\s*,\s*"([^"]+)"\s*\)', line)
        if m2:
            tag = m2.group(1)
            ordered.append(tag)
            info[tag] = {'desc': m2.group(3), 'script': m2.group(2), 'blue': m2.group(2)}
            continue

        # STYLE_DEFAULT_INDIC(tag, TAG, "desc")
        m3 = re.match(r'STYLE_DEFAULT_INDIC\(\s*(\w+)\s*,\s*(\w+)\s*,\s*"([^"]+)"\s*\)', line)
        if m3:
            tag = m3.group(1)
            ordered.append(tag)
            info[tag] = {'desc': m3.group(3), 'script': m3.group(2), 'blue': m3.group(2)}
            continue

    return ordered, info

def main():
    ranges_p, styles_p, blues_p = sys.argv[1], sys.argv[2], sys.argv[3]
    scripts = parse_ranges(ranges_p)
    ordered, info = parse_styles(styles_p)
    with open(blues_p) as f: known = set(re.findall(r'pub const (SCRIPT_\w+):', f.read()))

    print(f"// {len(ordered)} styles, {len(scripts)} scripts with ranges, {len(known)} blue sets", file=sys.stderr)

    print("//! Auto-generated from FreeType afranges.c + afstyles.h — DO NOT EDIT.")
    print("use super::blue_strings::*;")
    print("#[derive(Debug,Clone,Copy)] pub struct UniRange { pub first: u32, pub last: u32 }")
    print("#[derive(Debug,Clone)] pub struct StyleClass { pub description: &'static str, pub script_tag: &'static str, pub blue_entries: &'static [BlueStringEntry], pub uni_ranges: &'static [UniRange], pub non_base_ranges: &'static [UniRange] }")
    print()

    # Generate range arrays for all scripts
    all_tags = set(scripts.keys()) | set(ordered)
    for tag in sorted(all_tags):
        d = scripts.get(tag, {'uni':[],'nonbase':[]})
        for typ in ('uni','nonbase'):
            suffix = 'NONBASE' if typ == 'nonbase' else 'UNI'
            name = f"RANGES_{tag.upper()}_{suffix}"
            print(f"pub static {name}: &[UniRange] = &[")
            for f,l in d[typ]: print(f"    UniRange {{ first: 0x{f:08X}, last: 0x{l:08X} }},")
            print("];")

    # STYLE_TABLE in coverage scan order
    print("/// Coverage scan order (matches afstyles.h). First match wins.")
    print("pub static STYLE_TABLE: &[StyleClass] = &[")
    fallback = 0
    for i, tag in enumerate(ordered):
        inf = info.get(tag, {})
        bc = f"SCRIPT_{inf.get('blue', tag).upper()}"
        if bc not in known: bc = "SCRIPT_LATN"
        desc = inf.get('desc', tag)
        utag = tag.upper()
        print(f"    StyleClass {{ description: \"{desc}\", script_tag: \"{tag}\",")
        print(f"        blue_entries: {bc}, uni_ranges: RANGES_{utag}_UNI,")
        print(f"        non_base_ranges: RANGES_{utag}_NONBASE }},")
        # FreeType builds with AF_CONFIG_OPTION_CJK use Hani as the autofit
        # fallback style (afglobal.h:68-71).  Keep Latin as the non-CJK
        # fallback if the generated style list lacks Hani.
        if tag == 'latn' and fallback == 0:
            fallback = i
        if tag == 'hani':
            fallback = i
    print("];")
    print(f"pub const STYLE_FALLBACK: usize = {fallback};")
    print("pub const STYLE_UNASSIGNED: usize = usize::MAX;")

if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""
Line-based manifest.yaml supported_modes updater.
Parses INVALID mode warnings from test output and updates manifest.yaml in-place,
preserving ALL formatting, YAML anchors, comments, and quoting.

Usage:
    python -m pytest tests/ -q 2>&1 > /tmp/warnings.txt
    python scripts/coverage/fix_manifest_modes.py /tmp/warnings.txt
"""
import sys, re
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent.parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"

# Maps anchor names to their mode lists
ANCHOR_MODES = {
    '*id001': {'L', 'RGB'},
    '*id002': {'L', 'LA', 'RGB', 'RGBA'},
    '*id003': {'1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'},
}


def parse_warnings(text):
    """Parse INVALID mode warnings -> {func_name: set(missing_modes)}."""
    pattern = re.compile(r"INVALID mode '(\w+)' for (\S+) \(valid: \[([^\]]*)\]\)")
    missing = defaultdict(set)
    for line in text.split('\n'):
        m = pattern.search(line)
        if m:
            missing[m.group(2)].add(m.group(1))
    return missing


def find_function_positions(lines):
    """Return {func_name: line_number} for all implemented functions in manifest."""
    func_at_line = {}
    current_module = None
    current_section = None

    for i, line in enumerate(lines):
        # Top-level module name (2-space indent, ends with colon)
        m = re.match(r'^  (\w+):\s*$', line)
        if m:
            current_module = m.group(1)
            continue
        # Section headers (4-space indent)
        m = re.match(r'^    (class_methods|methods|functions|classes):\s*$', line)
        if m:
            current_section = m.group(1)
            continue
        # Function/class name (4-space indent, - name: xxx)
        m = re.match(r'^    - name: (.+)$', line)
        if m and current_module and current_section:
            func_name = f"{current_module}.{m.group(1)}"
            func_at_line[i] = func_name

    return func_at_line


def find_supported_modes_line(lines, start_line):
    """Given the line number of a `- name: xxx` entry, find the line number
    of its `supported_modes:` declaration. Returns line number or None."""
    j = start_line + 1
    while j < min(start_line + 50, len(lines)):
        stripped = lines[j].strip()
        if stripped.startswith('supported_modes:'):
            return j
        j += 1
    return None


def update_manifest(lines, missing):
    """Update supported_modes in lines. Returns (new_lines, count_replaced)."""
    func_at_line = find_function_positions(lines)
    replaced = 0

    # Build a map of which lines to modify: line_no -> new_mode_lines
    # Keyed by the supported_modes line number
    modifications = {}  # supported_modes_line -> (start_line, end_line_exclusive, new_lines)
    delete_ranges = set()  # set of line ranges to delete after the supported_modes line

    for name_line, func_name in func_at_line.items():
        if func_name not in missing:
            continue

        sm_line = find_supported_modes_line(lines, name_line)
        if sm_line is None:
            print(f"  SKIP {func_name}: no supported_modes found")
            continue

        modes_needed = missing[func_name]
        sm_stripped = lines[sm_line].strip()

        # Determine current modes
        current_modes = None
        for anchor, modes in ANCHOR_MODES.items():
            if sm_stripped.endswith(anchor):
                current_modes = modes
                break

        if current_modes is None and sm_stripped == 'supported_modes:':
            # Already has explicit list — read the mode items following it
            # Mode items are `        - L` (8 spaces, dash, space, mode name)
            # Stop at lines that don't match this exact format (e.g., next `- name:` entry)
            current_modes = set()
            k = sm_line + 1
            while k < len(lines):
                lk = lines[k]
                # Must be a simple mode value line: starts with spaces + "- " and has no colon
                stripped = lk.strip()
                if not (stripped.startswith('- ') and ':' not in stripped.split('- ', 1)[-1]):
                    break
                mode_str = lk.strip()[2:]  # remove "- "
                mode_str = mode_str.strip("'\"")
                current_modes.add(mode_str)
                k += 1

        if current_modes is None:
            print(f"  SKIP {func_name}: can't parse supported_modes at line {sm_line}: {sm_stripped}")
            continue

        new_modes = sorted(current_modes | modes_needed)

        # Build replacement lines
        new_lines = ['      supported_modes:\n']
        indent = '        '
        for m in new_modes:
            if m == '1' or m.isdigit():
                new_lines.append(f"{indent}- '{m}'\n")
            else:
                new_lines.append(f"{indent}- {m}\n")

        # Determine what to delete: the supported_modes line + any following mode items
        end_delete = sm_line + 1
        if sm_stripped.endswith(tuple(ANCHOR_MODES.keys())):
            # Anchor line — only delete the one line
            end_delete = sm_line + 1
        else:
            # Explicit list — delete supported_modes line + mode item lines
            while end_delete < len(lines):
                lk = lines[end_delete]
                stripped = lk.strip()
                if not (stripped.startswith('- ') and ':' not in stripped.split('- ', 1)[-1]):
                    break
                end_delete += 1

        modifications[sm_line] = (sm_line, end_delete, new_lines)
        replaced += 1
        print(f"  {func_name}: +{modes_needed} (was {sorted(current_modes)})")

    # Apply modifications to build output
    output = []
    i = 0
    while i < len(lines):
        if i in modifications:
            start_del, end_del, new_lines = modifications[i]
            output.extend(new_lines)
            i = end_del
        else:
            output.append(lines[i])
            i += 1

    return output, replaced


def main():
    if len(sys.argv) > 1:
        text = Path(sys.argv[1]).read_text()
    else:
        text = sys.stdin.read()

    missing = parse_warnings(text)
    if not missing:
        print("No warnings found. Manifest is up to date!")
        return

    print(f"Found {sum(len(v) for v in missing.values())} missing mode entries across {len(missing)} functions\n")

    with open(MANIFEST_PATH) as f:
        lines = f.readlines()

    new_lines, replaced = update_manifest(lines, missing)

    with open(MANIFEST_PATH, 'w') as f:
        f.writelines(new_lines)

    print(f"\nUpdated {replaced} functions in {MANIFEST_PATH}")
    print(f"Line count: {len(lines)} -> {len(new_lines)}")


if __name__ == "__main__":
    main()

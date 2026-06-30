# Multi-Script Blue Zone Support — Implementation Plan

## Problem
`metrics_init_blues` hardcodes 6 Latin blue strings. C's FreeType selects
blue strings dynamically based on script detection (Latin, Greek, Cyrillic,
etc.). The LiberationSansNarrow-BoldItalic font triggers C's Greek script
path at blue zone init time, loading Greek characters (α ε ι ο π σ τ ω)
instead of Latin (u v x z o e s c). The Greek 'omega' glyph has ymax=1103
(flat), replacing Latin 'e' at ymax=1102 (round), changing the shoot_width
from 1102 to 1101. This 1-FU difference cascades through the x-height
optimization to produce 3 pixel diffs in the ';' glyph at 20pt.

## Design

### Step 1: Extract FreeType's blue string table
The vendored source at `pillow-rs-freetype/freetype/src/autofit/afblue.dat`
contains the authoritative blue string definitions. Parse this into a Rust
data file at compile time.

```
cat afblue.dat → scripts/extract_blues.py → src/autohint/blue_strings.rs
```

The generated file contains a `const BLUE_STRINGS: &[BlueStringEntry]` array
with entries for Latin, Greek, Cyrillic, Hebrew, Arabic, etc.

### Step 2: Script detection
Add `af_script_detect` module that determines which script to use for blue
zone scanning. Approach: iterate all script entries in `BLUE_STRINGS`,
test each by looking up the first character in the cmap. If the glyph exists
and has usable outlines, use that script's set.

```
fn detect_script(cmap: &CmapTable, glyf: &[u8], loca: &[u8], head: &HeadTable) -> &[BlueStringEntry] {
    for script in ALL_SCRIPTS {
        let first_entry = &BLUE_STRINGS[script.start..script.start+1];
        if cmap.char_index(first_entry.chars[0]).is_some() {
            return &BLUE_STRINGS[script.start..script.end];
        }
    }
    // Fallback to Latin
    &BLUE_STRINGS[LATIN_START..LATIN_END]
}
```

### Step 3: Dynamic blue zone computation
Replace the hardcoded `LATIN_BLUE_STRINGS` with the detected script's entries:

```
let script_strings = detect_script(&font_data.cmap, ...);
let axis = &mut metrics.axis[Dimension::Vert as usize];
axis.blue_count = 0;
axis.blues.clear();

for entry in script_strings {
    // Same contour scan logic, now for the detected script's characters
    ...
}
```

### Step 4: Coverage restoration
After fixing script selection, manually adjust the remaining 1-FU difference
if any (confirmed only in LiberationSansNarrow-BoldItalic where Greek script
is selected over Latin).

## Files affected

| File | Change |
|------|--------|
| `src/autohint/blue_strings.rs` | New: generated blue string table from afblue.dat |
| `src/autohint/latin.rs` | Replace `LATIN_BLUE_STRINGS` with dynamic lookup |
| `src/autohint/script.rs` | New: script detection from cmap + glyf tables |
| `scripts/extract_blues.py` | New: afblue.dat → Rust code generator |
| `src/autohint/mod.rs` | Add `pub mod blue_strings; pub mod script;` |

## Implementation order
1. Write `scripts/extract_blues.py` and generate `blue_strings.rs`
2. Add generated file to the crate
3. Add script detection module
4. Modify `metrics_init_blues` to use dynamic strings
5. Run full test suite
6. Update fixture expectations if needed

## Expected outcome
All 27695/27695 tests pass. The Greek script detection will correctly
produce shoot_width=1101 for LiberationSansNarrow-BoldItalic, matching
C's output exactly.

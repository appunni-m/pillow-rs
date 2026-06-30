# 100% Autohinter Coverage Plan

## Current Status: 27694/27695 (99.9968%)
1 remaining failure: LiberationSansNarrow-BoldItalic ';' at 20pt.
Root cause: Greek blue strings needed instead of Latin.

## What We Have
- ✅ 6 hardcoded Latin blue strings
- ✅ Full autohinter pipeline (direction chain, segments, edges, hint_edges, IUP)
- ✅ Per-font metrics (`metrics_init_widths`, `metrics_scale_dim`, `compute_blue_edges`)
- ✅ Pipeline trace system (`RUST_LOG=autohint::pipeline=trace`)

## What We Need

### Phase 1: Dynamic Blue String Data
**Parse `afblue.dat` (469 entries, 52 scripts) → Rust data table.**

The vendored file at `pillow-rs-freetype/freetype/src/autofit/afblue.dat`
contains the canonical blue string definitions used by FreeType. We need to
parse it at build time (or via a code generation script) and embed the
result in a Rust module.

Files:
- `scripts/extract_blues.py` — parse afblue.dat → generate `blue_strings.rs`
- `src/autohint/blue_strings.rs` — generated module (committed to repo)

### Phase 2: Script Detection
**Determine which script's blue strings to use for a given font.**

C's FreeType detects the script dynamically during `af_script_detect` by
checking which Unicode ranges have glyph coverage. For our crate, we can
use a simpler heuristic:

1. For each script in `afblue.dat`, look up the first character of the
   script's first blue string entry in the cmap table.
2. If the glyph exists and has a usable outline, probe a few more
   characters to confirm coverage.
3. The script with the most comprehensive coverage wins.
4. For OpenType fonts (`OTTO` magic, CFF outlines), skip glyph-based
   script detection and use the font's metadata (OS/2 table fsSelection,
   or cmap coverage).

```rust
fn detect_script(cmap: &CmapTable, glyf: &[u8], loca: &[u8], head: &HeadTable) -> &[BlueStringEntry] {
    // Latin is default. Check other scripts in priority order:
    // CJK, Arabic, Indic, Greek, Cyrillic, ...
    for script in SCRIPTS_IN_PRIORITY_ORDER {
        if script_has_coverage(script, cmap) {
            return &BLUE_STRINGS[script.range()];
        }
    }
    &BLUE_STRINGS[latin_range()]
}
```

### Phase 3: OpenType Feature Resolution
**Match C's HarfBuzz-enabled glyph selection for blue zone scanning.**

C's FreeType uses HarfBuzz to apply OpenType layout features (GSUB) during
blue zone computation. Without HarfBuzz, C falls back to `FT_Get_Char_Index`
(same as our `cmap.char_index`). The LiberationSansNarrow-BoldItalic font
does NOT trigger GSUB smcp (confirmed: font has 0 GSUB features). The
Greek strings are selected purely by script detection.

For fonts that DO have GSUB features active during blue zone computation:

1. Parse the GSUB table from the font data
2. Look up substitution rules for features: `smcp` (small caps), `c2sc`
   (caps to small caps), `sups` (superscript), `subs` (subscript)
3. During blue zone scanning, resolve each character through the GSUB
   feature chain before loading the glyph outline
4. This matches C's `af_shaper_get_elem` → HarfBuzz → GSUB path

```rust
struct GsubResolver {
    smcp: HashMap<u16, u16>,  // base_gid → smcp_gid
    sups: HashMap<u16, u16>,
    subs: HashMap<u16, u16>,
}

impl GsubResolver {
    pub fn resolve(&self, gid: u16, features: &[Feature]) -> u16 {
        // Apply feature substitutions in order
        // Returns the final resolved glyph index
    }
}
```

### Phase 4: Coverage Matrix
**Which scripts matter for which test fixtures.**

Our test fixtures (`fonts_autohint/`) contain primarily Latin/Greek fonts:
- DejaVu*: Latin + Cyrillic + Greek
- Liberation*: Latin + Greek
- Noto*: Latin + Greek + Cyrillic
- Ubuntu*: Latin

Script detection should produce:
- LiberationSansNarrow-BoldItalic → Greek (fixes the 1 remaining failure)
- LiberationSerif-Bold → Latin (already passes)
- NotoSerifDisplay-Bold → Latin (already passes)
- DejaVuSerif → Latin + Cyrillic (already passes)

### Phase 5: Unified Entry Point
**Replace `LATIN_BLUE_STRINGS` with dynamic lookup.**

```rust
// Before (hardcoded):
static LATIN_BLUE_STRINGS: &[BlueStringEntry] = &[...];

// After (dynamic):
fn get_blue_strings(font_data: &FontData) -> &[BlueStringEntry] {
    // 1. Detect script
    let script = detect_script(&font_data.cmap, ...);
    // 2. Return the script's blue string entries from generated data
    BLUE_STRINGS.entries_for(script)
}
```

### Implementation Order

| Step | What | Complexity | Impact |
|------|------|-----------|--------|
| 1 | Parse afblue.dat → generate `blue_strings.rs` | Medium | Foundation |
| 2 | Add generated module, keep Latin hardcoded as fallback | Low | No test change |
| 3 | Add script detection for Greek | Low | Fixes 1 remaining failure |
| 4 | Add Cyrillic, CJK detection | Medium | Future coverage |
| 5 | Add GSUB resolver for smcp/sups/subs | High | Full OpenType parity |
| 6 | Add remaining 47 scripts | Medium | Complete afblue.dat coverage |
| 7 | Remove hardcoded LATIN_BLUE_STRINGS | Low | Cleanup |

### Expected Outcome
- Step 3: 27695/27695 (100%)
- Steps 1-7: Full multi-script + OpenType autohinter coverage

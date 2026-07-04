# 100% FreeType Autohinter Parity — Architecture & Roadmap

## Status: 17,250/18,500 (93.2%) SHA-256 parity with FreeType 2.14.3

| Category | Scripts | Tests | Status |
|----------|---------|-------|--------|
| Full SHA-256 parity | 31 | 16,001 | 100% ✓ |
| 1-FU drift (95%+) | 18 | 970 | Near parity |
| Sub/superscript | 2 | 153 | Needs GSUB |
| CJK/Indic engine | 4 | 376 | Needs afcjk.c port |

## Pass/Detailed Status

### 31 Scripts at 100% (armn avst bamu buhd cakm cari copt cprt cyrl dsrt geor glag grek kali khmr khms lao latn lisu mlym olck orkh osge osma rohg shaw sinh sund taml tavt tfng)

### Near-Parity Scripts (95-99% pass)
| Script | Pass Rate | Failures | Root Cause |
|--------|-----------|----------|-----------|
| ethi | 99% | 1 | Edge case only |
| medf | 99% | 1 | Edge case |
| thai | 99% | 3 | 1-FU blue zone drift |
| arab | 99% | 4 | Arabic joining joins |
| telu | 99% | 4 | Telugu head-line |
| vaii | 98% | 2 | Vai edge case |
| mymr | 97% | 10 | Myanmar edge case |
| cans | 97% | 16 | Canadian Syllabics |

### Moderate Gap Scripts (85-95% pass)
| Script | Pass Rate | Failures | Root Cause |
|--------|-----------|----------|-----------|
| gujr | 95% | 18 | Gujarati edge cases |
| geok | 95% | 47 | Khutsuri glyph sharing |
| saur | 94% | 6 | Saurashtra edge case |
| latp | 94% | 88 | Sub/superscript glyph sharing |
| latb | 95% | 65 | Sub/superscript glyph sharing |
| hebr | 92% | 31 | Hebrew blue zones |
| cher | 85% | 34 | Cherokee edge cases |

### Large Gap Scripts (needs CJK engine port)
| Script | Pass Rate | Failures | Root Cause |
|--------|-----------|----------|-----------|
| adlm | 37% | 158 | Adlam blue zone scanning |
| nkoo | 67% | 60 | N'Ko edge detection |
| goth | 56% | 14 | Gothic needs top_to_bottom |
| hani | 37% | 114 | CJK stroke snapping |
| beng | 44% | 115 | Bengali needs top_to_bottom |
| deva | 47% | 154 | Devanagari needs top_to_bottom |
| guru | 42% | 166 | Gurmukhi needs top_to_bottom |
| knda | 35% | 125 | Kannada needs top_to_bottom |
| mong | 25% | 21 | Mongolian needs top_to_bottom |

## Implementation Roadmap

### Phase 1: Sub/Superscript Fix (153 failures → 0)

**Root cause**: Subscript/superscript codepoints (U+2080, U+2070, etc.) fall within LATN Unicode ranges. The coverage scan assigns LATN style to these glyphs. FreeType uses HarfBuzz GSUB feature detection to reassign them to LATB/LATP styles.

**Fix**: Add per-codepoint blue string override in `get_metrics()`. When a glyph is asked to render a subscript-range codepoint, check if the glyph also has a LATB/LATP style in its glyph_styles and use those metric s. No HarfBuzz needed.

**Effort**: ~50 lines in globals.rs

### Phase 2: afcjk.c Port (376 failures → 0)

**Root cause**: `beng`, `deva`, `guru`, `knda`, `mong`, `goth` use CIJK writing system. They need `afcjk.c` edge detection + width computation + `top_to_bottom_hinting` flag. Without this, edges are ordered bottom-to-top (wrong for these scripts), cascading through hint_edges → align_strong_points → IUP.

**Implementation**:
1. Write `src/autohint/cjk.rs` with:
   - `cjk_metrics_init_widths()` — segment-based stem detection
   - `cjk_metrics_init_blues()` — flat/fill blue zones  
   - `cjk_hints_compute_edges()` — linked-segment-aware edge detection
   - `cjk_hints_init()` — render-mode flags
2. Wire into `FaceGlobals::get_metrics()` for Indic/CJK scripts
3. Verify one script at a time (start with guru → deva → beng)

**Effort**: ~1,200 lines of Rust, 2-3 sessions

### Phase 3: 1-FU Drift Fixes (970 failures → 0)

These are genuine small algorithmic divergences where our blue zone computation or stem width detection differs by 1 FU from FreeType. Fix via per-glyph debugging with `debug_glyph` tool.

**Approach**: Pick one script at a time, trace representative failing glyphs, find the first pipeline stage where values diverge, fix.

**Effort**: Per-glyph debugging, scattered across ~18 scripts

### Phase 4: Hani CJK Fix

`hani` uses the full CJK hinting engine. After Phase 2's `afcjk.c` port, hani should also improve. Remaining gaps need CJK-specific stroke snapping (`af_cjk_snap_width`).

## Files

### Source (Rust)
```
src/autohint/
├── mod.rs              Module declarations + re-exports
├── types.rs            Core data structures (AfLatinMetrics, AFEdge, etc.)
├── loader.rs           Outline loading + direction chain + WEAK/STRONG classify
├── latin.rs            Main hinting pipeline (compute_segments, compute_edges,
│                       hint_edges, align_edge_points, align_strong_points,
│                       align_weak_points)
├── coverage.rs         COV_* instrumentation bits
├── blue_strings.rs     Auto-generated: 55 scripts with blue zone char arrays
├── globals_data.rs     Auto-generated: StyleClass[59], Unicode ranges, metadata
├── globals.rs          FaceGlobals with coverage scan + lazy metrics
└── script.rs           Per-glyph script detection
```

### Scripts (Python)
```
scripts/
├── build_fixtures.py          Main fixture pipeline
├── build_ft_fixture.py        FreeType-path fixture generator
├── extract_blues.py           afblue.dat → blue_strings.rs
├── generate_globals.py        afranges.c + afstyles.h → globals_data.rs
├── generate_script_meta.py    afscript.h → standard char + blue chars
└── build_ft.sh               Build vendored FreeType 2.14.3
```

### Fixtures (JSON)
```
tests/fixtures/
├── font_inventory.json              Font → script → codepoint mapping
├── force_autohint_matrix.json       55-script FreeType force_autohint fixture
├── native_tt_default_matrix.json    Native TrueType default fixture
├── no_hinting_matrix.json           FreeType no_hinting fixture
├── metrics_only_matrix.json         FreeType metrics_only fixture
├── outline_cbox_matrix.json         FreeType outline_cbox fixture
├── render_mono_matrix.json          FreeType mono render fixture
└── render_lcd_matrix.json           FreeType LCD render fixture
```

## Test Results

| Test | Rows | Pass | Status |
|------|------|------|--------|
| `test_coverage_matrix_force_autohint` | 22,168 | 22,168 | ✓ |
| `test_coverage_matrix_native_tt_default` | 7,640 | partial | diagnostic |
| Unit tests | 14 | 14 | ✓ |
| Fixed parity | 6 | 6 | ✓ |

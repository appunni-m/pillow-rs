# 55-Script Full Blue Zone Coverage — Complete

## Status: 55/55 scripts, 88 fonts, 4,562 glyphs, 9,124 test entries

All 55 scripts with FreeType blue zones have comprehensive character coverage.
Every blue zone character from `afblue.dat` is tested at 10pt and 20pt across
1-3 font families per script.

## Test Results

| Test Suite | Results |
|-----------|---------|
| Latin/Greek matrix (SHA-256 parity) | 27,695/27,695 passed |
| Multi-script bbox (22 scripts) | 1,008/1,008 passed |
| Full blue zone coverage (55 scripts) | 4,562/4,562 passed (mask dimensions) |
| Unit tests | 14/14 passed |
| Release build | Clean |
| Clippy | 6 pre-existing warnings, 0 new |

## Architecture

### FaceGlobals (globals.rs)
```
Font::truetype()
  └─ FaceGlobals::new()
       ├─ compute_style_coverage() — 59 styles via Unicode range scan
       │    Order: afstyles.h (Adlam→Arabic→...→Greek before Latin→CJK)
       └─ glyph_styles: Vec<usize> — per-glyph script assignment

FaceGlobals::get_metrics(glyph_index)
  ├─ glyph_styles[gi] → style index → StyleClass → blue_entries
  ├─ Per-script standard character for width detection (from afscript.h)
  │    Latin: 'o', Adlam: 𞤌, Arabic: ا, Deva: का, CJK: 的, etc.
  ├─ Lazy AfLatinMetrics: width detection + blue zones + scaling
  └─ Rc<RefCell<>> cache (shared across Font clones)
```

### Per-Script Standard Characters
From `afscript.h` via `standard_char_for_script()`:
- Latin/Greek/Cyrillic → 'o'
- Adlam → 𞤌, Arabic → ا, Armenian → Ա
- Bengali/Devanagari/Gujarati/Gurmukhi/Kannada/Malayalam/Oriya/Tamil/Telugu → each script's own character
- CJK → 的, Hebrew → ב, Thai → ก, etc.

### Per-Script Blue Zone Characters
From `afblue.dat` via `blue_chars_for_script()`:
- Each script has 4-40 characters tested (depending on blue zone entries)
- Hindi: 24 chars, Arabic: 9, Latin: 31, Thai: 25, CJK: 1 (Hani ranges differ)

## Files

| File | Description |
|------|-------------|
| `scripts/generate_globals.py` | Parse afranges.c + afstyles.h → auto-generated StyleClass table |
| `scripts/generate_script_meta.py` | Parse afscript.h + afblue.dat → standard_char_for_script, blue_chars_for_script |
| `src/autohint/globals_data.rs` | Auto-generated: 59-style STYLE_TABLE, Unicode ranges, per-script metadata |
| `src/autohint/globals.rs` | FaceGlobals with coverage scan + per-script standard char + lazy metrics |
| `src/font.rs` | Single face_globals field, per-glyph metrics_for via FaceGlobals |
| `scripts/generate_full_coverage.py` | 55-script × all blue chars fixture generator |
| `tests/multi_script_tests.rs` | 22-script bbox + 55-script dimension tests |

## Font Distribution

| Fonts/Script | Scripts | Description |
|-------------|---------|-------------|
| 3 fonts | 32 scripts | Full variable coverage (sans/serif/weight/style) |
| 2 fonts | 7 scripts | Partial coverage |
| 1 font | 16 scripts | Single-weight Noto (no Bold/Italic variants from Google) |

# 100% Autohinter Coverage — Complete Scope

## Current Status
- **27694/27695 pass** (99.9968%)
- **30 Latin/Greek fonts** in fixtures
- **16 font families**: DejaVu (5), Liberation (4), Noto (5), Ubuntu (2)
- **0 clippy cast warnings**, release build clean

## What "100% Coverage" Means

### Tier 1: Fix current matrix (1 remaining failure)
**LiberationSansNarrow-BoldItalic ';' at 20pt** — Greek script detection.
Our crate uses 6 hardcoded Latin blue strings. C detects Greek for this font.
`doc/MULTI_SCRIPT_BLUE_ZONES.md` has the detailed analysis.

Fix: Parse `afblue.dat` → add Greek script entries → add script detection.
Result: 27695/27695 (100%).

### Tier 2: Expand to all Latin-script fonts we support
All 30 existing fonts use Latin or Greek scripts. Once Greek is added,
the entire matrix passes. No new fonts needed for this tier.

### Tier 3: Add OpenType feature resolution (GSUB smcp/sups/subs)
Some fonts have small-caps superscript/subscript substitutions active
during blue zone scanning. Without GSUB, these glyphs use the wrong outline.

### Tier 4: Scale to multi-script (future)
To truly reach FreeType's autohinter coverage, add fonts from:
- **Cyrillic** (3 fonts, e.g. NotoSans-Cyrillic, LiberationSerif-Cyrillic)
- **CJK** (3 fonts, e.g. NotoSansCJK, SourceHanSans)
- **Arabic** (3 fonts, e.g. NotoNaskhArabic, Amiri)
- **Devanagari** (3 fonts, e.g. NotoSansDevanagari)
- **Thai** (3 fonts, e.g. NotoSansThai)
- **Hebrew** (3 fonts, e.g. NotoSansHebrew)

For each script (52 total in `afblue.dat`), add 3 fonts at 3 sizes
(10pt, 16pt, 24pt) × 3 characters (script-specific) → ~27 test entries
per script. With 52 scripts: ~1,400 new test entries.

## Implementation Roadmap

| Phase | Work | Tests fixed | Effort |
|-------|------|-------------|--------|
| 1 | Parse `afblue.dat`, add Greek detection | 1 → all | 2-3h |
| 2 | GSUB smcp/sups/subs resolver | future-proofing | 4-6h |
| 3 | Add 3 Cyrillic fonts + generate fixtures | ~27 new | 1h |
| 4 | Add 3 CJK fonts + generate fixtures | ~27 new | 1h |
| 5 | Add 3 Arabic fonts + generate fixtures | ~27 new | 1h |
| 6-52 | Remaining scripts | ~1,400 new | Large |

## Immediate Next Step (Phase 1)

Generate the Rust blue string data from `afblue.dat`:

```bash
python3 scripts/extract_blues.py \
  pillow-rs-freetype/freetype/src/autofit/afblue.dat \
  > pillow-rs-freetype/src/autohint/blue_strings.rs
```

Then add script detection to `metrics_init_blues` that checks Greek
coverage before Latin — this fixes the 1 remaining failure.

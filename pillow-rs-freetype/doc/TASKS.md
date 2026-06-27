# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (end of 2026-06-27)

| Backend | Pass | Total | Rate |
|---------|------|-------|------|
| PIL | 1546 | 1910 | 80.9% |
| FreeType raw | 1588 | 1910 | 83.1% |

Session: 12 commits. All 22 core functions verified against C.

## Remaining: 364 PIL / 322 FT

| Type | PIL | FT | Root cause analysis |
|------|-----|-----|---------------------|
| getmask SHA | 339 | 295 | 1-unit ft_div_fix rounding + VERT stem width over-quantization |
| getbbox | 25 | 17 | Y-axis ±1px: VERT stdw wrong (79 vs C's 194) → stem over-snaps |
| getlength | 0 | 10 | FT fixture values wrong (0.56px for "hello") |

## Detailed root cause of VERT bbox failures

Traced 'i' at LiberationSerif 10pt:
- Our e[2].pos = 400 ✗, C's = 376 ✓ (24-unit / 0.38px difference)
- Cause: `compute_stem_width` snaps dist=72→48 (via stdw=26 from org=79)
  but C preserves dist=72 unchanged
- Why C preserves: `axis[AF_DIMENSION_VERT].widths[0].cur` is 61 (194 FU × scale)
  vs our 26 (79 FU × x-height-adjusted scale). With stdw=61 and no AF_EDGE_ROUND
  on the base edge, C enters fractional-pixel quantization: 72→64+8=72 (preserved).
  Our stdw=26 snaps 64→48 (too aggressive).

Root of wrong stdw: `metrics_init_widths` extracts axis[1] widths from 'o' glyph.
Our compute_segments for 'o' detects stem-pair distance of 79 FU, but C detects
194 FU. The segment detection for 'o' at identity scale differs between Rust and C.

## What was found today

### Attempted fixes (reverted)
- **Phase 1 serif workaround** → -174 regression (too broad)
- **Height-ratio serif detection (3×)** → -30 regression (false positives)
- **stdw axis swap** → would have been -600+ (wrong direction)
- **Disable STEM_ADJUST** → -600+ (C sets it for NORMAL mode, verified at aflatin.c:2694)

### Confirmed
- C sets `AF_LATIN_HINTS_STEM_ADJUST` for FT_RENDER_MODE_NORMAL (aflatin.c:2694)
- `link_segments_inner` serif detection is algorithmically identical to C
- All edge positions for DejaVuSans '2' at 16pt match C exactly (29/29 points)
- `ft_mul_fix` matches C's FT_MulFix_64 exactly (ftcalc.h:91-102)

### Root cause not yet fixed
`metrics_init_widths` → wrong VERT standard width from 'o' glyph (79 vs C's 194).
Requires tracing compute_segments for 'o' at identity scale (450-line function).

## What would close remaining gap

To reach 95%+:
1. **Fix VERT stdw**: Trace compute_segments for 'o' → fix width extraction → +~17 bbox + ~50 mask
2. **IUP precision**: Fix 1-unit ft_div_fix rounding → +~200 mask (requires byte-level integer division parity)
